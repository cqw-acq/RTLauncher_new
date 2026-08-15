#[cfg(target_os = "windows")]
use std::io::Error;
use std::io::{BufRead, BufReader};
#[cfg(target_os = "windows")]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
#[cfg(target_os = "windows")]
use std::{env, fs};
use tauri::command;

use super::codec;
use super::logs::{self, append_text as append_log_str, clear_files as clear_openp2p_log_files};
use super::paths::{
    bridge_dir as get_bridge_dir, executable_path as get_executable_path,
    openp2p_dir as get_openp2p_dir, openp2p_path as get_openp2p_path,
};
use super::OPENP2P_BIN;
static OPENP2P_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
static OPENP2P_START_LOCK: Mutex<()> = Mutex::new(());

fn is_openp2p_process_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("openp2p") || name.eq_ignore_ascii_case("openp2p.exe")
}

/// 窗口关闭时的快速兜底清理：直接用系统进程 API 结束 openp2p，
/// 不依赖 wmic/taskkill 等外部命令，避免触发杀毒软件拦截。
#[cfg(target_os = "windows")]
pub fn quick_kill_openp2p() {
    let system = sysinfo::System::new_all();
    for process in system.processes().values() {
        if is_openp2p_process_name(&process.name().to_string_lossy()) {
            let _ = process.kill();
        }
    }
}

fn has_openp2p_system_process() -> bool {
    let system = sysinfo::System::new_all();
    system
        .processes()
        .values()
        .any(|process| is_openp2p_process_name(&process.name().to_string_lossy()))
}

#[cfg(target_os = "windows")]
fn quote_windows_argument(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return argument.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
        }
        backslashes = 0;
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn start_openp2p_with_args(args: &[String]) -> Result<String, String> {
    // 序列化启动请求，并在真正创建进程前检查系统中已有的 OpenP2P。
    // OpenP2P 默认占用同一个 PublicIPPort；重复启动会导致新实例持续报端口冲突。
    let _start_guard = OPENP2P_START_LOCK
        .lock()
        .map_err(|_| "OpenP2P 启动流程锁异常".to_string())?;
    if mp_is_openp2p_running() {
        return Err("OpenP2P 已在后台运行，请先停止当前联机后再启动".to_string());
    }

    let openp2p_path = get_openp2p_path()?;
    if !openp2p_path.exists() {
        return Err(format!(
            "{} 不存在于: {}，请确保已将 openp2p 可执行文件拖入安装",
            OPENP2P_BIN,
            openp2p_path.display()
        ));
    }
    let working_dir = get_openp2p_dir()?;
    let path_str = get_executable_path(&openp2p_path)?;
    logs::clear_buffer();
    clear_openp2p_log_files(&working_dir);
    append_log_str(&format!(
        "[RTLauncher] 正在启动 openp2p...\n\
         [RTLauncher]   可执行文件: {}\n\
         [RTLauncher]   工作目录: {}\n\
         [RTLauncher]   参数: {:?}\n\
         [RTLauncher]   日志目录: {}/log/\n\n",
        path_str,
        working_dir.display(),
        args,
        working_dir.display()
    ));
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new(&openp2p_path);
        c.creation_flags(0x08000000);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new(&openp2p_path);
    cmd.current_dir(&working_dir);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());
    let spawn_result = cmd.spawn();
    match spawn_result {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => {
                                let with_newline = text + "\n";
                                append_log_str(&with_newline);
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
            if let Some(stderr) = child.stderr.take() {
                thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => {
                                let with_newline = "[stderr] ".to_string() + &text + "\n";
                                append_log_str(&with_newline);
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
            {
                let mut guard = OPENP2P_PROCESS
                    .lock()
                    .map_err(|_| "无法锁定进程句柄".to_string())?;
                *guard = Some(child);
            }
            append_log_str("[RTLauncher] ✅ openp2p 进程已启动，正在捕获输出...\n");
            append_log_str("[RTLauncher] (右侧日志窗口会持续显示 openp2p 的所有输出)\n\n");
            Ok(path_str)
        }
        Err(e) => {
            let is_elevation_error = cfg!(target_os = "windows") && e.raw_os_error() == Some(740);
            if is_elevation_error {
                append_log_str("[RTLauncher] ⚠ openp2p.exe 需要管理员权限才能运行\n");
                append_log_str("[RTLauncher]   正在以管理员身份重新启动（会弹出 UAC 提示）...\n");
                append_log_str(
                    "[RTLauncher]   注意：以管理员权限启动后，无法通过管道捕获 stdout\n",
                );
                append_log_str(
                    "[RTLauncher]   将改为读取 openp2p 自己生成的日志文件来获取反馈\n\n",
                );
                #[cfg(target_os = "windows")]
                {
                    use std::ffi::OsStr;
                    use std::os::windows::ffi::OsStrExt;
                    use std::ptr;
                    use winapi::um::shellapi::ShellExecuteW;
                    use winapi::um::winuser::SW_HIDE;
                    let exe_path = std::path::PathBuf::from(&path_str);
                    let exe_wide: Vec<u16> = OsStr::new(&exe_path)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let args_str = args
                        .iter()
                        .map(|argument| quote_windows_argument(argument))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let args_wide: Vec<u16> = OsStr::new(&args_str)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let runas_wide: Vec<u16> = OsStr::new("runas")
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let work_dir_wide: Vec<u16> = working_dir
                        .as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let result = unsafe {
                        ShellExecuteW(
                            ptr::null_mut(),
                            runas_wide.as_ptr(),
                            exe_wide.as_ptr(),
                            args_wide.as_ptr(),
                            work_dir_wide.as_ptr(),
                            SW_HIDE,
                        )
                    };
                    if (result as i32) <= 32 {
                        let err = Error::last_os_error();
                        let err_msg = format!(
                            "[RTLauncher] ❌ 以管理员身份启动也失败: {}\n\
                             [RTLauncher]   请尝试手动以管理员身份运行此程序（右键 → 以管理员身份运行）\n",
                            err
                        );
                        append_log_str(&err_msg);
                        return Err(err_msg);
                    }
                    append_log_str("[RTLauncher] ✅ openp2p 已以管理员权限启动\n");
                    append_log_str("[RTLauncher]   正在等待 openp2p 生成日志文件...\n");
                    append_log_str("[RTLauncher]   (如果长时间无输出，请检查 openp2p.exe 是否被杀毒软件拦截)\n\n");
                    {
                        let mut guard = OPENP2P_PROCESS
                            .lock()
                            .map_err(|_| "无法锁定进程句柄".to_string())?;
                        *guard = None;
                    }
                    Ok(path_str)
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let err_msg = format!(
                        "[RTLauncher] ❌ 启动失败: {}\n\
                         [RTLauncher]   请尝试以 sudo/管理员权限运行此程序\n",
                        e
                    );
                    append_log_str(&err_msg);
                    Err(err_msg)
                }
            } else {
                let err_msg = format!(
                    "[RTLauncher] ❌ 启动失败 (系统错误: {})\n\
                     [RTLauncher]   可执行文件路径: {}\n\
                     [RTLauncher]   工作目录: {}\n\
                     [RTLauncher]   请确认:\n\
                     [RTLauncher]   1. {} 存在且与当前操作系统及 CPU 架构匹配\n\
                     [RTLauncher]   2. 当前用户具备该文件的执行权限\n\
                     [RTLauncher]   3. 文件未被系统安全工具拦截\n",
                    e,
                    path_str,
                    working_dir.display(),
                    OPENP2P_BIN
                );
                append_log_str(&err_msg);
                Err(err_msg)
            }
        }
    }
}
#[command]
pub fn mp_check_openp2p() -> bool {
    get_openp2p_path().map(|p| p.exists()).unwrap_or(false)
}
#[command]
pub fn mp_install_openp2p(src_path: String) -> Result<String, String> {
    let bridge_dir = get_bridge_dir()?;
    std::fs::create_dir_all(&bridge_dir).map_err(|e| format!("创建 bridge 目录失败: {}", e))?;
    let dest = bridge_dir.join(OPENP2P_BIN);
    std::fs::copy(&src_path, &dest).map_err(|e| format!("复制文件失败: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)
            .map_err(|e| format!("获取文件属性失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms).map_err(|e| format!("设置执行权限失败: {}", e))?;
    }
    Ok(dest.to_string_lossy().to_string())
}
#[command]
pub fn mp_start_openp2p_host(room_name: String) -> Result<String, String> {
    let args = codec::host_arguments(&room_name);
    start_openp2p_with_args(&args)
}
#[command]
pub fn mp_encode_room_info(room_name: String, port_count: String) -> String {
    codec::encode_room_info(&room_name, &port_count)
}
#[command]
pub fn mp_start_openp2p_join(encoded_value: String, player_name: String) -> Result<String, String> {
    let args = codec::join_arguments(&encoded_value, &player_name)?;
    start_openp2p_with_args(&args)
}
fn kill_all_openp2p_processes() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;
        use winapi::um::shellapi::ShellExecuteW;

        if !has_openp2p_system_process() {
            return Ok(());
        }

        // 这台机器上的 taskkill/tasklist 会返回“分页文件太小”，因此绕开这两个命令，
        // 使用管理员 PowerShell 的 Process API 重复结束守护/工作进程。
        let working_dir = get_openp2p_dir()?;
        let stop_script = working_dir.join(".rtlauncher-stop-openp2p.ps1");
        let script = r#"$ErrorActionPreference = 'SilentlyContinue'
for ($i = 0; $i -lt 20; $i++) {
  Get-Process -Name openp2p -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 250
}
"#;
        fs::write(&stop_script, script).map_err(|e| format!("写入 OpenP2P 停止脚本失败: {}", e))?;

        let windows_dir =
            env::var_os("WINDIR").unwrap_or_else(|| OsStr::new("C:\\Windows").to_os_string());
        let powershell_path = PathBuf::from(windows_dir)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        let powershell_wide: Vec<u16> = powershell_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let runas_wide: Vec<u16> = OsStr::new("runas")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let powershell_args = format!(
            "-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"",
            stop_script.display()
        );
        let args_wide: Vec<u16> = OsStr::new(&powershell_args)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        append_log_str("[RTLauncher] 请求管理员权限，通过进程 API 结束 OpenP2P...\n");
        let work_dir_wide: Vec<u16> = working_dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                runas_wide.as_ptr(),
                powershell_wide.as_ptr(),
                args_wide.as_ptr(),
                work_dir_wide.as_ptr(),
                winapi::um::winuser::SW_HIDE,
            )
        };
        if (result as isize) <= 32 {
            let _ = fs::remove_file(&stop_script);
            return Err(format!(
                "管理员终止请求未执行（ShellExecute 返回 {}）",
                result as isize
            ));
        }

        // 停止逻辑运行在 blocking 线程中，等待期间不占用 Tauri 界面线程。
        let mut consecutive_missing = 0;
        for _ in 0..150 {
            thread::sleep(std::time::Duration::from_millis(200));
            if !has_openp2p_system_process() {
                consecutive_missing += 1;
                if consecutive_missing >= 3 {
                    let _ = fs::remove_file(&stop_script);
                    return Ok(());
                }
            } else {
                consecutive_missing = 0;
            }
        }
        let _ = fs::remove_file(&stop_script);
        return Err("OpenP2P 进程仍在运行，请确认管理员提示后重试".to_string());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use sysinfo::Signal;

        // Linux 与 macOS 共用 sysinfo 进程 API，避免依赖 killall/pkill/grep 等外部命令。
        // 前几轮先发送 TERM，仍存活时再发送 KILL。
        for attempt in 0..12 {
            let system = sysinfo::System::new_all();
            let mut found = false;
            for process in system
                .processes()
                .values()
                .filter(|process| is_openp2p_process_name(&process.name().to_string_lossy()))
            {
                found = true;
                if attempt < 4 {
                    let _ = process.kill_with(Signal::Term);
                } else {
                    let _ = process.kill();
                }
            }
            if !found {
                return Ok(());
            }
            thread::sleep(std::time::Duration::from_millis(250));
        }

        Err("OpenP2P 进程仍在运行，请检查当前用户的进程权限".to_string())
    }
}
#[command]
pub async fn mp_stop_openp2p() -> Result<(), String> {
    let working_dir = get_openp2p_dir().ok();
    append_log_str("[RTLauncher] 正在停止 openp2p 进程（含保护线程）...\n");
    {
        if let Ok(mut guard) = OPENP2P_PROCESS.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
            *guard = None;
        }
    }
    tauri::async_runtime::spawn_blocking(kill_all_openp2p_processes)
        .await
        .map_err(|e| format!("OpenP2P 停止任务异常: {}", e))??;
    if let Some(dir) = &working_dir {
        clear_openp2p_log_files(dir);
    }
    append_log_str("[RTLauncher] ✅ openp2p 进程（含所有保护线程）已终止\n");
    Ok(())
}
#[command]
pub fn mp_is_openp2p_running() -> bool {
    if let Ok(mut guard) = OPENP2P_PROCESS.lock() {
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(None) => return true,
                Ok(Some(_)) => {
                    // `openp2p -d` 的父进程会在拉起后台工作进程后正常退出。
                    // 清除旧句柄，继续通过系统进程列表检查真正的工作进程。
                    *guard = None;
                }
                Err(e) => {
                    append_log_str(&format!("[RTLauncher] 检查进程句柄失败: {}\n", e));
                }
            }
        }
    }

    has_openp2p_system_process()
}
#[command]
pub fn mp_poll_log() -> String {
    let working_dir = get_openp2p_dir().ok();
    logs::poll(working_dir.as_deref())
}
#[command]
pub fn mp_get_openp2p_dir() -> String {
    get_openp2p_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| String::new())
}
#[command]
pub fn mp_get_openp2p_path() -> String {
    get_openp2p_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| String::new())
}
pub fn ensure_openp2p_stopped() {
    {
        if let Ok(mut guard) = OPENP2P_PROCESS.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
            *guard = None;
        }
    }
    let _ = kill_all_openp2p_processes();
}

#[cfg(test)]
mod tests {
    use super::super::codec::{encode_room_info, host_arguments, join_arguments};
    use super::super::logs::read_increment as read_log_increment;
    use super::is_openp2p_process_name;
    #[cfg(target_os = "windows")]
    use super::quote_windows_argument;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn room_codec_keeps_the_legacy_base64_format() {
        assert_eq!(encode_room_info("room", "25565"), "cm9vbSwyNTU2NQ==");
    }

    #[test]
    fn host_arguments_keep_the_existing_openp2p_contract() {
        assert_eq!(
            host_arguments("room"),
            vec!["-d", "-node", "room", "-token", "11661058147873189554"]
        );
    }

    #[test]
    fn join_arguments_decode_legacy_room_codes() {
        assert_eq!(
            join_arguments("cm9vbSwyNTU2NQ==", "player").expect("decode room code"),
            vec![
                "-d",
                "-node",
                "player",
                "-token",
                "11661058147873189554",
                "-appname",
                "RTlauncher",
                "-peernode",
                "room",
                "-dstip",
                "127.0.0.1",
                "-dstport",
                "25565",
                "-srcport",
                "25565",
                "-protocol",
                "tcp",
            ]
        );
    }

    #[test]
    fn openp2p_process_name_matches_exact_names_case_insensitively() {
        assert!(is_openp2p_process_name("openp2p"));
        assert!(is_openp2p_process_name("OpenP2P"));
        assert!(is_openp2p_process_name("openp2p.exe"));
        assert!(is_openp2p_process_name("OPENP2P.EXE"));
    }

    #[test]
    fn openp2p_process_name_rejects_other_processes() {
        assert!(!is_openp2p_process_name("openp2pd"));
        assert!(!is_openp2p_process_name("myopenp2p"));
        assert!(!is_openp2p_process_name("openp2p-helper"));
        assert!(!is_openp2p_process_name("notepad.exe"));
        assert!(!is_openp2p_process_name(""));
    }

    #[test]
    fn log_reader_returns_only_new_bytes_and_handles_truncation() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rtlauncher-openp2p-log-{}-{}.txt",
            std::process::id(),
            unique
        ));
        let offset = Mutex::new(0u64);

        fs::write(&path, b"first\n").expect("write initial log");
        assert_eq!(read_log_increment(&path, &offset), b"first\n");
        assert!(read_log_increment(&path, &offset).is_empty());

        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open log for append")
            .write_all(b"second\n")
            .expect("append log");
        assert_eq!(read_log_increment(&path, &offset), b"second\n");

        fs::write(&path, b"new\n").expect("truncate and rewrite log");
        assert_eq!(read_log_increment(&path, &offset), b"new\n");

        let _ = fs::remove_file(path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_arguments_preserve_spaces_quotes_and_trailing_backslashes() {
        assert_eq!(quote_windows_argument("plain"), "plain");
        assert_eq!(quote_windows_argument("two words"), "\"two words\"");
        assert_eq!(quote_windows_argument("a\"b"), "\"a\\\"b\"");
        assert_eq!(
            quote_windows_argument("C:\\room path\\"),
            "\"C:\\room path\\\\\""
        );
    }
}
