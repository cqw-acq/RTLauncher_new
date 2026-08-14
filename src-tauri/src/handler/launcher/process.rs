use serde::Serialize;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
};
use tauri::Emitter;

/// 全局游戏进程跟踪。
/// 等待线程持有 Child，避免终止命令提前清除状态并漏掉进程回收。
struct GameProcess {
    pid: u32,
    fully_started: bool,
}

fn game_process_store() -> &'static Mutex<Option<GameProcess>> {
    static STORE: OnceLock<Mutex<Option<GameProcess>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
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

/// 检测游戏是否完全启动（JVM 启动、加载完资源、主窗口就绪）
/// 通过 Minecraft 日志中的标志性字符串判断
fn is_game_fully_started(line: &str) -> bool {
    // 原版 Minecraft: "Minecraft client started" / "Preparing spawn area" / "Minecraft initialized"
    // 常见 Mod 加载器: "mod loading complete" / "Minecraft is ready to start" / "Launching game"
    let lower = line.to_lowercase();
    lower.contains("minecraft client started")
        || lower.contains("minecraft is ready to start")
        || lower.contains("preparing spawn area")
        || lower.contains("minecraft initialized")
        || lower.contains("launching game")
        || lower.contains("loading complete") && lower.contains("mod")
        || lower.contains("minecraft client has started")
        // 对于新版 Fabric/原版，未必输出上述固定文案；一旦已经进入世界、
        // 显示主菜单或开始连接服务器，就说明客户端已经成功运行。
        || lower.contains("title screen")
        || lower.contains("connecting to ")
        || lower.contains("joined the game")
        || lower.contains("joined ") && lower.contains("server")
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

    // 持有同一个锁完成“检查 + 启动 + 登记”。这样两个并发请求不能覆盖彼此。
    let mut store = game_process_store()
        .lock()
        .map_err(|e| format!("无法读取游戏进程状态: {}", e))?;
    if let Some(current) = store.as_ref() {
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
                &mut store,
                GameProcess {
                    pid,
                    fully_started: false,
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
                                    if let Some(gp) = store.as_mut().filter(|gp| gp.pid == pid) {
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
                                    if let Some(gp) = store.as_mut().filter(|gp| gp.pid == pid) {
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
                    .map(|mut store| clear_process_if_pid(&mut store, pid))
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

/// 终止当前游戏进程（在游戏未完全启动前可调用）
#[tauri::command]
pub fn kill_game_process() -> Result<String, String> {
    // 终止期间保留进程槽。等待线程会回收 Child，然后清除相同 PID 的槽。
    let store = game_process_store().lock().map_err(|e| e.to_string())?;
    let process = store
        .as_ref()
        .ok_or_else(|| "当前没有运行中的游戏进程".to_string())?;
    let pid = process.pid;
    let started = process.fully_started;
    terminate_process(pid)?;

    if started {
        Ok(format!("游戏进程 (PID {}) 已终止", pid))
    } else {
        Ok(format!("启动中的游戏进程 (PID {}) 已取消", pid))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        claim_startup_transition, clear_process_if_pid, insert_process_if_empty, parse_log_level,
        GameProcess,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

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
            },
        )
        .expect("insert the first process");

        let second = insert_process_if_empty(
            &mut store,
            GameProcess {
                pid: 200,
                fully_started: false,
            },
        );
        assert!(second.is_err());
        assert_eq!(store.as_ref().map(|process| process.pid), Some(100));

        assert!(!clear_process_if_pid(&mut store, 200));
        assert_eq!(store.as_ref().map(|process| process.pid), Some(100));

        assert!(clear_process_if_pid(&mut store, 100));
        assert!(store.is_none());
    }
}
