use serde::Serialize;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
};
use tauri::Emitter;

/// 全局游戏进程跟踪（存储 Child 所有权 + PID，便于 kill）
struct GameProcess {
    child: Option<Child>,
    pid: u32,
    fully_started: bool,
}

fn game_process_store() -> &'static Mutex<Option<GameProcess>> {
    static STORE: OnceLock<Mutex<Option<GameProcess>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
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
    // 查找 [XXX/LEVEL] 模式（log4j2 标准格式）
    if let Some(start) = line.find('[') {
        if let Some(end) = line[start..].find(']') {
            let tag = &line[start + 1..start + end];
            if let Some(slash) = tag.rfind('/') {
                let level = &tag[slash + 1..];
                match level.to_uppercase().as_str() {
                    "ERROR" | "FATAL" => return "error",
                    "WARN" | "WARNING" => return "warn",
                    _ => {}
                }
            }
        }
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
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            std::fs::metadata(&javaPath).map_err(|e| format!("无法读取 Java 文件信息: {}", e))?;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            info!("Java 缺少执行权限，正在修复: {}", javaPath.display());
            let mut new_perms = permissions.clone();
            new_perms.set_mode(permissions.mode() | 0o755);
            std::fs::set_permissions(&javaPath, new_perms)
                .map_err(|e| format!("无法设置 Java 执行权限: {}", e))?;
        }
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

    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            info!("游戏启动成功，进程ID: {}", pid);

            // 从子进程取出 stdout/stderr 管道
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // 存储到全局进程表（启动中，尚未完成初始化）
            {
                let mut store = game_process_store().lock().unwrap();
                *store = Some(GameProcess {
                    child: Some(child),
                    pid,
                    fully_started: false,
                });
            }

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
                            if !flag.load(std::sync::atomic::Ordering::SeqCst)
                                && is_game_fully_started(&line)
                            {
                                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                                {
                                    let mut store = game_process_store().lock().unwrap();
                                    if let Some(gp) = store.as_mut() {
                                        gp.fully_started = true;
                                    }
                                }
                                let _ = handle.emit("game-fully-started", pid);
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
                            if !flag.load(std::sync::atomic::Ordering::SeqCst)
                                && is_game_fully_started(&line)
                            {
                                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                                {
                                    let mut store = game_process_store().lock().unwrap();
                                    if let Some(gp) = store.as_mut() {
                                        gp.fully_started = true;
                                    }
                                }
                                let _ = handle.emit("game-fully-started", pid);
                            }
                        }
                    }
                });
            }

            // 在后台线程中等待进程结束，结束时向前端发送事件
            thread::spawn(move || {
                // 从全局 store 中取出 child 所有权以 wait
                let child_to_wait = {
                    let mut store = game_process_store().lock().unwrap();
                    store.as_mut().and_then(|gp| gp.child.take())
                };

                let exit_code = if let Some(mut c) = child_to_wait {
                    match c.wait() {
                        Ok(status) => display_exit_code(status),
                        Err(e) => {
                            error!("等待游戏进程 {} 时出错: {}", pid, e);
                            -1
                        }
                    }
                } else {
                    -1
                };

                info!("游戏进程 {} 已结束，退出码: {}", pid, exit_code);

                // 清空全局进程表
                {
                    let mut store = game_process_store().lock().unwrap();
                    *store = None;
                }

                let _ = app_handle.emit("game-exited", exit_code);
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

/// 终止当前游戏进程（在游戏未完全启动前可调用）
#[tauri::command]
pub fn kill_game_process() -> Result<String, String> {
    let mut store = game_process_store().lock().map_err(|e| e.to_string())?;

    let process_info = match store.as_mut() {
        Some(gp) => {
            let pid = gp.pid;
            let started = gp.fully_started;
            // 尝试直接 kill
            let result = if let Some(c) = gp.child.as_mut() {
                c.kill().map(|_| ()).map_err(|e| e.to_string())
            } else {
                Err("没有进程句柄".to_string())
            };

            // 跨平台兜底：如果 child.kill() 失败，用系统命令 kill
            if result.is_err() {
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/PID", &pid.to_string()])
                        .output();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }
            }

            Some((pid, started))
        }
        None => None,
    };

    // 清空 store
    *store = None;

    match process_info {
        Some((pid, started)) => {
            let msg = if started {
                format!("游戏进程 (PID {}) 已终止", pid)
            } else {
                format!("启动中的游戏进程 (PID {}) 已取消", pid)
            };
            // 注意：这里无法发送事件，因为没有 app_handle
            // 前端会根据返回的成功结果自行更新状态
            Ok(msg)
        }
        None => Err("当前没有运行中的游戏进程".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_level_detects_error_and_warn_from_log4j_thread_tag() {
        assert_eq!(
            parse_log_level("[12:34:56] [Server thread/ERROR]: boom"),
            "error"
        );
        assert_eq!(
            parse_log_level("[12:34:56] [Client thread/FATAL]: crash"),
            "error"
        );
        assert_eq!(parse_log_level("[12:34:56] [main/WARN]: careful"), "warn");
        assert_eq!(
            parse_log_level("[12:34:56] [main/WARNING]: careful"),
            "warn"
        );
        assert_eq!(parse_log_level("[12:34:56] [main/INFO]: all good"), "info");
    }

    #[test]
    fn parse_log_level_is_case_insensitive_for_the_level_tag() {
        assert_eq!(parse_log_level("[main/error]: lowercase"), "error");
        assert_eq!(parse_log_level("[main/warn]: lowercase"), "warn");
    }

    #[test]
    fn parse_log_level_falls_back_to_keyword_scan_without_a_thread_tag() {
        assert_eq!(parse_log_level("Something failed STDERR: oops"), "error");
        assert_eq!(parse_log_level("plain [WARN] no thread tag"), "warn");
        assert_eq!(parse_log_level("just a regular line"), "info");
    }

    #[test]
    fn parse_log_level_defaults_to_info_when_the_bracket_has_no_slash() {
        assert_eq!(parse_log_level("[main]: no level here"), "info");
    }

    #[test]
    fn detects_various_fully_started_markers_case_insensitively() {
        for line in [
            "[12:00:00] [main/INFO]: Minecraft client started",
            "Minecraft is ready to start",
            "PREPARING SPAWN AREA: 50%",
            "Minecraft initialized",
            "Launching game",
            "Minecraft client has started",
            "Title Screen",
            "Connecting to server.example.com, 25565",
            "[main/INFO]: Joined the game",
        ] {
            assert!(is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn requires_both_keywords_for_compound_markers() {
        assert!(is_game_fully_started("Mod loading complete"));
        assert!(!is_game_fully_started("Loading complete")); // 缺少 "mod"
        assert!(is_game_fully_started("Player joined the server"));
        assert!(!is_game_fully_started("joined")); // 缺少 "server"
    }

    #[test]
    fn does_not_flag_unrelated_log_lines_as_fully_started() {
        for line in [
            "[12:00:00] [main/INFO]: Setting user: Steve",
            "Loading Minecraft 1.20.1 with Fabric Loader",
            "Downloading assets...",
        ] {
            assert!(!is_game_fully_started(line), "{line}");
        }
    }

    #[test]
    fn game_log_event_serializes_with_expected_field_names() {
        let event = GameLogEvent {
            level: "warn".to_string(),
            message: "low memory".to_string(),
        };
        let json = serde_json::to_value(&event).expect("serialize GameLogEvent");
        assert_eq!(json["level"], "warn");
        assert_eq!(json["message"], "low memory");
    }

    #[cfg(unix)]
    #[test]
    fn display_exit_code_returns_the_normal_exit_code_when_present() {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            display_exit_code(std::process::ExitStatus::from_raw(3 << 8)),
            3
        );
        assert_eq!(
            display_exit_code(std::process::ExitStatus::from_raw(0)),
            0
        );
    }

    fn spawn_long_running_child() -> Child {
        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/C", "ping", "-n", "30", "127.0.0.1"])
                .spawn()
                .expect("failed to spawn placeholder child process")
        }
        #[cfg(not(windows))]
        {
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("failed to spawn placeholder child process")
        }
    }

    #[test]
    fn kill_game_process_lifecycle_reports_no_process_then_terminates_tracked_ones() {
        // 确保测试开始时没有残留的全局状态。
        *game_process_store().lock().unwrap() = None;

        assert_eq!(
            kill_game_process(),
            Err("当前没有运行中的游戏进程".to_string())
        );

        // 未完全启动的进程被终止时，提示语应为"已取消"。
        let child = spawn_long_running_child();
        let pid = child.id();
        *game_process_store().lock().unwrap() = Some(GameProcess {
            child: Some(child),
            pid,
            fully_started: false,
        });

        let result = kill_game_process().expect("expected the tracked process to be killed");
        assert!(result.contains(&pid.to_string()));
        assert!(result.contains("已取消"));
        assert!(game_process_store().lock().unwrap().is_none());

        // 完全启动的进程被终止时，提示语应为"已终止"。
        let child2 = spawn_long_running_child();
        let pid2 = child2.id();
        *game_process_store().lock().unwrap() = Some(GameProcess {
            child: Some(child2),
            pid: pid2,
            fully_started: true,
        });

        let result2 = kill_game_process().expect("expected the tracked process to be killed");
        assert!(result2.contains(&pid2.to_string()));
        assert!(result2.contains("已终止"));
        assert!(game_process_store().lock().unwrap().is_none());
    }
}
