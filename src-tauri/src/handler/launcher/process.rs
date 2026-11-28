use serde::Serialize;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
    thread,
};
use tauri::Emitter;

/// 全局游戏进程跟踪。
/// 等待线程持有 Child，避免终止命令提前清除状态并漏掉进程回收。
struct GameProcess {
    pid: u32,
    fully_started: bool,
    termination_requested: bool,
}

/// 全局启动状态：已拉起的游戏进程，以及仍在准备阶段（检测/下载 Java、
/// 构建参数）、尚未拉起进程的启动请求。
///
/// `launch_game` 改为 async 后，准备阶段不再占住主线程，终止命令可能在
/// 进程拉起之前到达；准备中的启动登记在这里，终止命令才能取消它。
#[derive(Default)]
struct LaunchState {
    process: Option<GameProcess>,
    /// 处于准备阶段的启动请求数量
    preparing: usize,
    /// 每次终止命令取消准备中的启动时递增。启动请求记录登记时的值，
    /// 值发生变化即表示它已被取消。
    cancel_generation: u64,
}

fn game_process_store() -> &'static Mutex<LaunchState> {
    static STORE: OnceLock<Mutex<LaunchState>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(LaunchState::default()))
}

/// 登记/注销准备阶段只做计数，状态被毒化时仍继续，避免注销失败导致计数泄漏。
fn lock_launch_state() -> MutexGuard<'static, LaunchState> {
    game_process_store()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub(super) const LAUNCH_CANCELLED_MESSAGE: &str = "启动已取消";

fn register_preparation(state: &mut LaunchState) -> u64 {
    state.preparing += 1;
    state.cancel_generation
}

fn finish_preparation(state: &mut LaunchState) {
    state.preparing = state.preparing.saturating_sub(1);
}

fn is_launch_cancelled(state: &LaunchState, generation: u64) -> bool {
    state.cancel_generation != generation
}

/// 一次处于准备阶段的启动请求。存活期间终止命令可以取消它，drop 时自动注销。
pub(super) struct LaunchTicket {
    generation: u64,
}

impl LaunchTicket {
    pub(super) fn register() -> Self {
        let generation = register_preparation(&mut lock_launch_state());
        Self { generation }
    }

    /// 准备阶段的检查点：已收到终止命令时返回错误，不再继续启动。
    pub(super) fn ensure_not_cancelled(&self) -> Result<(), String> {
        if is_launch_cancelled(&lock_launch_state(), self.generation) {
            Err(LAUNCH_CANCELLED_MESSAGE.to_string())
        } else {
            Ok(())
        }
    }

    /// 交给 `run_command`，在拉起进程的同一把锁内做最后一次取消检查。
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
}

impl Drop for LaunchTicket {
    fn drop(&mut self) {
        finish_preparation(&mut lock_launch_state());
    }
}

fn insert_process_if_empty(
    store: &mut Option<GameProcess>,
    process: GameProcess,
) -> Result<(), String> {
    if let Some(current) = store.as_ref() {
        return Err(format!("已有游戏进程正在运行 (PID {})", current.pid));
    }
    *store = Some(process);
    Ok(())
}

fn clear_process_if_pid(store: &mut Option<GameProcess>, pid: u32) -> bool {
    if store.as_ref().is_some_and(|process| process.pid == pid) {
        *store = None;
        true
    } else {
        false
    }
}

fn claim_termination_request(store: &mut Option<GameProcess>) -> Result<(u32, bool), String> {
    let process = store
        .as_mut()
        .ok_or_else(|| "当前没有运行中的游戏进程".to_string())?;
    if process.termination_requested {
        return Err(format!("游戏进程 (PID {}) 正在终止", process.pid));
    }

    process.termination_requested = true;
    Ok((process.pid, process.fully_started))
}

fn reset_termination_request_if_pid(store: &mut Option<GameProcess>, pid: u32) {
    if let Some(process) = store.as_mut().filter(|process| process.pid == pid) {
        process.termination_requested = false;
    }
}

#[derive(Debug, PartialEq, Eq)]
enum KillAction {
    /// 终止已拉起的游戏进程
    Terminate { pid: u32, started: bool },
    /// 游戏进程尚未拉起，只取消准备中的启动
    CancelPreparation,
}

/// 决定终止命令要做什么。准备中的启动（含排队等待的）一律取消，
/// 避免用户点了终止后游戏仍在准备结束时被拉起。
fn claim_kill_action(state: &mut LaunchState) -> Result<KillAction, String> {
    let has_preparing_launch = state.preparing > 0;
    if state.process.is_some() {
        let (pid, started) = claim_termination_request(&mut state.process)?;
        if has_preparing_launch {
            state.cancel_generation += 1;
        }
        return Ok(KillAction::Terminate { pid, started });
    }
    if has_preparing_launch {
        state.cancel_generation += 1;
        return Ok(KillAction::CancelPreparation);
    }
    Err("当前没有运行中的游戏进程".to_string())
}

/// 游戏客户端已经打开窗口、进入首次资源加载（随后显示主菜单）时才会输出的日志。
/// 均来自本地 E2E 实测日志（1.20.1 Forge/Quilt、1.21.1 原版/Fabric/NeoForge），
/// 大小写不敏感匹配。
const CLIENT_READY_MARKERS: &[&str] = &[
    // 1.12.2 起 SoundManager/SoundEngine 初始化完成
    "sound engine started",
    // 新版 LWJGL3 音频库：OpenAL initialized on device ...
    "openal initialized",
];

/// 旧有标记：原版/加载器的启动完成文案，以及进入世界、连接服务器等更晚的阶段。
const LEGACY_READY_MARKERS: &[&str] = &[
    "minecraft client started",
    "minecraft is ready to start",
    "preparing spawn area",
    "minecraft initialized",
    "launching game",
    "minecraft client has started",
    "title screen",
    "connecting to ",
    "joined the game",
];

/// 检测游戏是否完全启动（JVM 启动、主窗口就绪、开始加载资源/显示主菜单）
/// 通过 Minecraft 日志中的标志性字符串判断，大小写不敏感。
fn is_game_fully_started(line: &str) -> bool {
    let lower = line.to_lowercase();
    CLIENT_READY_MARKERS
        .iter()
        .chain(LEGACY_READY_MARKERS)
        .any(|marker| lower.contains(marker))
        // 纹理图集创建：1.12.2 为 "Created: 1024x512 textures-atlas"，
        // 1.13+ 为 "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas"
        || (lower.contains("created:") && lower.contains("atlas"))
        || (lower.contains("loading complete") && lower.contains("mod"))
        || (lower.contains("joined ") && lower.contains("server"))
}

/// 将进程结束状态转换为可展示的退出码。
/// Unix 上 `ExitStatus::code()` 在进程被信号终止时为 None；以前这类情况统一
/// 显示为 -1，无法区分 SIGABRT/SIGKILL 等真实原因。约定用负信号号表示它们。
pub(super) fn display_exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return -signal;
        }
    }

    -1
}

/// 游戏日志事件，发送给前端的结构体
#[derive(Debug, Clone, Serialize)]
pub(super) struct GameLogEvent {
    pub(super) level: String,
    pub(super) message: String,
}

/// 解析 Minecraft log4j 日志行，提取日志级别
/// 支持格式: [HH:MM:SS] [Thread/LEVEL]: message
fn parse_log_level(line: &str) -> &'static str {
    // 跳过时间戳等标签，查找 [线程/LEVEL] 模式。
    let mut remaining = line;
    while let Some(start) = remaining.find('[') {
        let after_start = &remaining[start + 1..];
        let Some(end) = after_start.find(']') else {
            break;
        };
        let tag = &after_start[..end];
        if let Some(slash) = tag.rfind('/') {
            let level = &tag[slash + 1..];
            match level.to_uppercase().as_str() {
                "ERROR" | "FATAL" => return "error",
                "WARN" | "WARNING" => return "warn",
                _ => {}
            }
        }
        remaining = &after_start[end + 1..];
    }
    // fallback: 全文扫描关键词
    let u = line.to_uppercase();
    if u.contains("[ERROR]") || u.contains("[FATAL]") || u.contains("STDERR:") {
        "error"
    } else if u.contains("[WARN]") || u.contains("[WARNING]") {
        "warn"
    } else {
        "info"
    }
}

fn claim_startup_transition(started: &std::sync::atomic::AtomicBool) -> bool {
    started
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
}

#[cfg(unix)]
fn ensure_owner_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::metadata(path).map_err(|e| format!("无法读取 Java 文件信息: {}", e))?;
    let permissions = metadata.permissions();
    if permissions.mode() & 0o100 != 0 {
        return Ok(());
    }

    let current_uid = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != current_uid {
        return Err(format!(
            "Java 文件不属于当前用户，不能修改执行权限: {}",
            path.display()
        ));
    }

    info!("Java 缺少所有者执行权限，正在修复: {}", path.display());
    let mut new_permissions = permissions;
    new_permissions.set_mode(new_permissions.mode() | 0o100);
    std::fs::set_permissions(path, new_permissions)
        .map_err(|e| format!("无法设置 Java 执行权限: {}", e))
}

pub fn run_command(
    args: Vec<String>,
    javaPath: PathBuf,
    MCPath: PathBuf,
    app_handle: tauri::AppHandle,
    launch_generation: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // 检查 Java 路径是否存在
    if !javaPath.exists() {
        return Err(format!("Java 路径不存在: {}", javaPath.display()).into());
    }

    // 校验 java_path 不是一个 .jar 文件
    if let Some(ext) = javaPath.extension() {
        if ext.eq_ignore_ascii_case("jar") {
            return Err(format!(
                "Java 路径指向了一个 .jar 文件而非 Java 可执行文件: {}\n请设置为 java 或 javaw 可执行文件的路径，例如 /usr/bin/java",
                javaPath.display()
            ).into());
        }
    }

    // 在 Unix 系统上检查并修复执行权限
    #[cfg(unix)]
    {
        ensure_owner_executable(&javaPath)?;
    }

    // 确保工作目录存在
    if !MCPath.exists() {
        std::fs::create_dir_all(&MCPath)
            .map_err(|e| format!("无法创建游戏目录 {}: {}", MCPath.display(), e))?;
    }

    let mut command = if cfg!(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos"
    )) {
        Command::new(&javaPath)
    } else {
        return Err("不支持的操作系统".to_string().into());
    };

    // 设置工作目录为 minecraft_path（共享资源目录）
    // 游戏的实际运行目录通过 --game-dir 参数传递
    command.current_dir(&MCPath);
    command.args(&args);
    // 捕获标准输出和错误输出以便转发日志到前端
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    // 持有同一个锁完成“检查 + 启动 + 登记”。这样两个并发请求不能覆盖彼此，
    // 终止命令也只能落在“取消启动”或“终止已登记进程”之一，不会漏杀。
    let mut store = game_process_store()
        .lock()
        .map_err(|e| format!("无法读取游戏进程状态: {}", e))?;
    if is_launch_cancelled(&store, launch_generation) {
        return Err(LAUNCH_CANCELLED_MESSAGE.into());
    }
    if let Some(current) = store.process.as_ref() {
        return Err(format!("已有游戏进程正在运行 (PID {})", current.pid).into());
    }

    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            info!("游戏启动成功，进程ID: {}", pid);

            // 从子进程取出 stdout/stderr 管道
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // 存储到全局进程表（启动中，尚未完成初始化）。
            insert_process_if_empty(
                &mut store.process,
                GameProcess {
                    pid,
                    fully_started: false,
                    termination_requested: false,
                },
            )?;
            drop(store);

            // 用于检测"完全启动"的共享 flag
            let fully_started_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            // 读取 stdout 并逐行转发给前端
            if let Some(out) = stdout {
                let handle = app_handle.clone();
                let flag = fully_started_flag.clone();
                thread::spawn(move || {
                    let reader = BufReader::new(out);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            let level = parse_log_level(&line).to_string();
                            println!("[{}] {}", level, line);
                            let _ = handle.emit(
                                "game-log",
                                GameLogEvent {
                                    level: level.clone(),
                                    message: line.clone(),
                                },
                            );
                            // 检测游戏是否已完全启动
                            if is_game_fully_started(&line) && claim_startup_transition(&flag) {
                                let is_current_process = {
                                    let mut store = game_process_store().lock().unwrap();
                                    if let Some(gp) =
                                        store.process.as_mut().filter(|gp| gp.pid == pid)
                                    {
                                        gp.fully_started = true;
                                        true
                                    } else {
                                        false
                                    }
                                };
                                if is_current_process {
                                    let _ = handle.emit("game-fully-started", pid);
                                }
                            }
                        }
                    }
                });
            }

            // 读取 stderr 并逐行转发给前端（通常为错误/警告信息）
            if let Some(err) = stderr {
                let handle = app_handle.clone();
                let flag = fully_started_flag.clone();
                thread::spawn(move || {
                    let reader = BufReader::new(err);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            let level = parse_log_level(&line).to_string();
                            println!("[{}] {}", level, line);
                            let _ = handle.emit(
                                "game-log",
                                GameLogEvent {
                                    level: level.clone(),
                                    message: line.clone(),
                                },
                            );
                            // 同样在 stderr 中检测启动完成（有些启动日志走 stderr）
                            if is_game_fully_started(&line) && claim_startup_transition(&flag) {
                                let is_current_process = {
                                    let mut store = game_process_store().lock().unwrap();
                                    if let Some(gp) =
                                        store.process.as_mut().filter(|gp| gp.pid == pid)
                                    {
                                        gp.fully_started = true;
                                        true
                                    } else {
                                        false
                                    }
                                };
                                if is_current_process {
                                    let _ = handle.emit("game-fully-started", pid);
                                }
                            }
                        }
                    }
                });
            }

            // 在后台线程中等待进程结束，结束时向前端发送事件
            thread::spawn(move || {
                let exit_code = match child.wait() {
                    Ok(status) => display_exit_code(status),
                    Err(e) => {
                        error!("等待游戏进程 {} 时出错: {}", pid, e);
                        -1
                    }
                };

                info!("游戏进程 {} 已结束，退出码: {}", pid, exit_code);

                // 仅清除此等待线程对应的进程。旧线程不能清除或上报新进程。
                let should_emit = game_process_store()
                    .lock()
                    .map(|mut store| clear_process_if_pid(&mut store.process, pid))
                    .unwrap_or(false);
                if should_emit {
                    let _ = app_handle.emit("game-exited", exit_code);
                }
            });
            Ok(())
        }
        Err(e) => {
            let msg = format!("游戏启动失败 (Java: {}): {}", javaPath.display(), e);
            println!("{}", msg);
            Err(msg.into())
        }
    }
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .map_err(|e| format!("无法运行 taskkill: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "taskkill 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(windows))]
fn terminate_process(pid: u32) -> Result<(), String> {
    let output = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| format!("无法运行 kill: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "kill 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// 终止当前游戏进程（在游戏未完全启动前可调用）。
/// 游戏进程尚未拉起（仍在检测/下载 Java、构建参数）时，取消这次启动。
#[tauri::command]
pub fn kill_game_process() -> Result<String, String> {
    // 先在锁内登记终止请求，再释放锁运行外部命令。
    let action = {
        let mut store = game_process_store().lock().map_err(|e| e.to_string())?;
        claim_kill_action(&mut store)?
    };
    let (pid, started) = match action {
        KillAction::CancelPreparation => {
            info!("游戏进程尚未拉起，已取消准备中的启动");
            return Ok("已取消正在准备的游戏启动".to_string());
        }
        KillAction::Terminate { pid, started } => (pid, started),
    };

    if let Err(error) = terminate_process(pid) {
        if let Ok(mut store) = game_process_store().lock() {
            reset_termination_request_if_pid(&mut store.process, pid);
        }
        return Err(error);
    }

    if started {
        Ok(format!("游戏进程 (PID {}) 已终止", pid))
    } else {
        Ok(format!("启动中的游戏进程 (PID {}) 已取消", pid))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        claim_kill_action, claim_startup_transition, claim_termination_request,
        clear_process_if_pid, finish_preparation, insert_process_if_empty, is_game_fully_started,
        is_launch_cancelled, parse_log_level, register_preparation,
        reset_termination_request_if_pid, GameProcess, KillAction, LaunchState,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn detects_startup_from_real_1_21_1_logs() {
        // 摘自本地 E2E：1.21.1 原版 / Fabric 0.20.1 / NeoForge 21.1.267 进入主菜单的日志
        for line in [
            "[16:37:28] [Render thread/INFO]: OpenAL initialized on device MacBook Pro扬声器",
            "[16:37:28] [Render thread/INFO]: Sound engine started",
            "[16:37:28] [Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas",
            "[16:38:01] [Render thread/INFO]: Created: 1024x512x0 minecraft:textures/atlas/gui.png-atlas",
            "[16:55:02] [Render thread/INFO] [minecraft/SoundEngine]: Sound engine started",
            "[16:55:02] [Render thread/INFO] [minecraft/TextureAtlas]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas",
        ] {
            assert!(is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn detects_startup_from_real_1_20_1_logs() {
        // 摘自本地 E2E：1.20.1 Forge 47.4.29 / Quilt 0.30.2 进入主菜单的日志
        for line in [
            "[16:53:57] [Render thread/INFO] [mojang/Library]: OpenAL initialized on device CoreAudio Default",
            "[16:53:57] [Render thread/INFO] [minecraft/SoundEngine]: Sound engine started",
            "[16:53:57] [Render thread/INFO] [minecraft/TextureAtlas]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas",
            "[16:54:26] [Render thread/INFO]: OpenAL initialized on device CoreAudio Default",
            "[16:54:26] [Render thread/INFO]: Created: 256x256x4 minecraft:textures/atlas/signs.png-atlas",
        ] {
            assert!(is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn detects_startup_from_1_12_2_log_format() {
        // 本次 E2E 的 1.12.2 在 arm64 上因 LWJGL2 原生库缺失未能进入主菜单，
        // 这里是 1.12.2 原版 / Forge 主菜单前的标准日志格式样本。
        for line in [
            "[12:00:00] [Sound Library Loader/INFO]: Sound engine started",
            "[12:00:00] [Client thread/INFO]: Created: 1024x512 textures-atlas",
            "[12:00:00] [Sound Library Loader/INFO] [minecraft/SoundManager]: Sound engine started",
            "[12:00:00] [Client thread/INFO] [minecraft/TextureMap]: Created: 512x512 textures-atlas",
        ] {
            assert!(is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn matches_startup_markers_case_insensitively() {
        assert!(is_game_fully_started(
            "[Render thread/INFO]: SOUND ENGINE STARTED"
        ));
        assert!(is_game_fully_started(
            "[Render thread/INFO]: openal Initialized."
        ));
    }

    #[test]
    fn does_not_detect_startup_from_early_log_lines() {
        // 摘自同一批 E2E 日志中 JVM/加载器启动、早期窗口、模组加载阶段的行，
        // 以及 1.12.2 启动失败时的行；这些都不能让启动页提前结束。
        for line in [
            "\u{1b}[32m[16:54:52.859] [main/INFO] [Launcher/MODLAUNCHER]: ModLauncher running: args [--fml.neoForgeVersion, 21.1.267, --fml.fmlVersion, 4.0.45, --fml.mcVersion, 1.21.1, --launchTarget, forgeclient]",
            "\u{1b}[m\u{1b}[32m[16:54:53.712] [main/INFO] [EARLYDISPLAY/]: Requested GL version 4.1 got version 4.1 ",
            "[16:53:50] [main/INFO] [cp.mo.mo.LaunchServiceHandler/MODLAUNCHER]: Launching target 'forgeclient' with arguments [--version, e2e-forge-1.20.1]",
            "[16:53:53] [modloading-worker-0/INFO] [ne.mi.co.ForgeMod/FORGEMOD]: Forge mod loading, version 47.4.29, for MC 1.20.1 with MCP 20230612.114412",
            "[16:54:59] [modloading-worker-0/INFO] [ne.ne.ne.co.NeoForgeMod/NEOFORGE-MOD]: NeoForge mod loading, version 21.1.267, for MC 1.21.1",
            "[16:37:49] [main/INFO]: Loading Minecraft 1.21.1 with Fabric Loader 0.20.1",
            "[16:37:49] [main/INFO]: Loading 57 mods:",
            "[16:54:16] [main/INFO]: Loading Minecraft 1.20.1 with Quilt Loader 0.30.2",
            "[16:37:20] [Datafixer Bootstrap/INFO]: 226 Datafixer optimizations took 99 milliseconds",
            "[16:37:23] [Render thread/INFO]: Setting user: RTLTest",
            "[16:37:23] [Render thread/INFO]: Backend library: LWJGL version 3.3.3+5",
            "[16:53:55] [Render thread/INFO] [minecraft/ReloadableResourceManager]: Reloading ResourceManager: vanilla, mod_resources",
            "[16:37:28] [Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play",
            "[16:38:52] [main/INFO] [FML]: Found valid fingerprint for Minecraft. Certificate fingerprint cd99959656f753dc28d863b46769f7f8fbaefcfc",
            "[16:38:52] [main/INFO] [LaunchWrapper]: Launching wrapped minecraft {net.minecraft.client.main.Main}",
            "Exception in thread \"main\" java.lang.UnsatisfiedLinkError: no lwjgl in java.library.path",
        ] {
            assert!(!is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn cancels_a_launch_that_is_still_preparing() {
        let mut state = LaunchState::default();
        let generation = register_preparation(&mut state);
        assert!(!is_launch_cancelled(&state, generation));

        assert_eq!(
            claim_kill_action(&mut state),
            Ok(KillAction::CancelPreparation)
        );
        assert!(is_launch_cancelled(&state, generation));

        // 取消之后登记的新启动不受之前的终止命令影响。
        finish_preparation(&mut state);
        let next = register_preparation(&mut state);
        assert!(!is_launch_cancelled(&state, next));
    }

    #[test]
    fn reports_nothing_to_kill_when_no_launch_is_active() {
        let mut state = LaunchState::default();

        assert!(claim_kill_action(&mut state).is_err());
        assert_eq!(state.cancel_generation, 0);

        // 准备阶段结束后计数归零，不会因重复注销下溢。
        let _ = register_preparation(&mut state);
        finish_preparation(&mut state);
        finish_preparation(&mut state);
        assert_eq!(state.preparing, 0);
        assert!(claim_kill_action(&mut state).is_err());
    }

    #[test]
    fn terminating_a_game_also_cancels_queued_launches() {
        let mut state = LaunchState {
            process: Some(GameProcess {
                pid: 100,
                fully_started: true,
                termination_requested: false,
            }),
            ..LaunchState::default()
        };
        let untouched = state.cancel_generation;
        assert_eq!(
            claim_kill_action(&mut state),
            Ok(KillAction::Terminate {
                pid: 100,
                started: true
            })
        );
        // 没有排队中的启动时不改变取消代数。
        assert_eq!(state.cancel_generation, untouched);

        reset_termination_request_if_pid(&mut state.process, 100);
        let queued = register_preparation(&mut state);
        assert_eq!(
            claim_kill_action(&mut state),
            Ok(KillAction::Terminate {
                pid: 100,
                started: true
            })
        );
        assert!(is_launch_cancelled(&state, queued));
    }

    #[test]
    fn reads_the_level_from_the_thread_tag_after_a_timestamp() {
        assert_eq!(
            parse_log_level("[12:00:00] [Render thread/ERROR]: failed"),
            "error"
        );
        assert_eq!(parse_log_level("[12:00:00] [Worker/WARN]: delayed"), "warn");
        assert_eq!(
            parse_log_level("[12:00:00] [Render thread/INFO]: ready"),
            "info"
        );
    }

    #[test]
    fn claims_the_startup_transition_only_once() {
        let started = AtomicBool::new(false);

        assert!(claim_startup_transition(&started));
        assert!(!claim_startup_transition(&started));
        assert!(started.load(Ordering::SeqCst));
    }

    #[cfg(unix)]
    #[test]
    fn adds_only_the_owner_execute_permission() {
        use super::ensure_owner_executable;
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "rtlauncher-java-permissions-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, []).expect("create temporary Java executable");
        let mut permissions = std::fs::metadata(&path)
            .expect("read temporary file metadata")
            .permissions();
        permissions.set_mode(0o640);
        std::fs::set_permissions(&path, permissions).expect("set initial permissions");

        ensure_owner_executable(&path).expect("add owner execute permission");

        let mode = std::fs::metadata(&path)
            .expect("read updated metadata")
            .permissions()
            .mode()
            & 0o777;
        let _ = std::fs::remove_file(&path);
        assert_eq!(mode, 0o740);
    }

    #[test]
    fn keeps_the_current_process_until_its_waiter_clears_it() {
        let mut store = None;

        insert_process_if_empty(
            &mut store,
            GameProcess {
                pid: 100,
                fully_started: false,
                termination_requested: false,
            },
        )
        .expect("insert the first process");

        let second = insert_process_if_empty(
            &mut store,
            GameProcess {
                pid: 200,
                fully_started: false,
                termination_requested: false,
            },
        );
        assert!(second.is_err());
        assert_eq!(store.as_ref().map(|process| process.pid), Some(100));

        assert!(!clear_process_if_pid(&mut store, 200));
        assert_eq!(store.as_ref().map(|process| process.pid), Some(100));

        assert!(clear_process_if_pid(&mut store, 100));
        assert!(store.is_none());
    }

    #[test]
    fn rejects_a_repeated_termination_request() {
        let mut store = Some(GameProcess {
            pid: 100,
            fully_started: false,
            termination_requested: false,
        });

        assert_eq!(claim_termination_request(&mut store), Ok((100, false)));
        assert!(claim_termination_request(&mut store).is_err());
    }

    #[test]
    fn resets_a_failed_termination_request_only_for_the_same_pid() {
        let mut store = Some(GameProcess {
            pid: 100,
            fully_started: false,
            termination_requested: false,
        });
        claim_termination_request(&mut store).expect("claim the first termination request");

        reset_termination_request_if_pid(&mut store, 200);
        assert!(claim_termination_request(&mut store).is_err());

        reset_termination_request_if_pid(&mut store, 100);
        assert_eq!(claim_termination_request(&mut store), Ok((100, false)));
    }
}
