use std::env;
use std::fs;
#[cfg(target_os = "windows")]
use std::io::Error;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use tauri::command;

const OPENP2P_BIN: &str = if cfg!(target_os = "windows") {
    "openp2p.exe"
} else {
    "openp2p"
};
static OPENP2P_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
static OPENP2P_START_LOCK: Mutex<()> = Mutex::new(());
static LOG_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OPENP2P_TXT_OFFSET: Mutex<u64> = Mutex::new(0);
static OPENP2P_LOG_OFFSET: Mutex<u64> = Mutex::new(0);
fn legacy_bridge_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .parent()
        .map(|parent| parent.join("RTL").join("bridge"))
}

fn preferred_bridge_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(PathBuf::from)
            .map(|base| base.join("RTLauncher").join("bridge"))
            .unwrap_or_else(|| env::temp_dir().join("RTLauncher").join("bridge"));
    }

    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|home| {
                home.join("Library")
                    .join("Application Support")
                    .join("RTLauncher")
                    .join("bridge")
            })
            .unwrap_or_else(|| env::temp_dir().join("RTLauncher").join("bridge"));
    }

    #[cfg(target_os = "linux")]
    {
        return crate::app_paths::linux_data_dir().join("bridge");
    }
}

fn get_bridge_dir() -> Result<PathBuf, String> {
    let preferred = preferred_bridge_dir();
    let preferred_binary = preferred.join(OPENP2P_BIN);
    if preferred_binary.is_file() {
        return Ok(preferred);
    }

    // 兼容旧版放在可执行文件旁边的目录；新安装使用各系统的用户可写数据目录。
    if let Some(legacy) = legacy_bridge_dir() {
        if legacy.join(OPENP2P_BIN).is_file() {
            return Ok(legacy);
        }
    }
    Ok(preferred)
}
fn get_openp2p_path() -> Result<PathBuf, String> {
    Ok(get_bridge_dir()?.join(OPENP2P_BIN))
}
fn get_openp2p_dir() -> Result<PathBuf, String> {
    let path = get_openp2p_path()?;
    Ok(path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(PathBuf::from(".")))
}
fn get_executable_path(path: &Path) -> Result<String, String> {
    if path.is_absolute() {
        Ok(path.display().to_string())
    } else {
        let abs = env::current_dir()
            .map_err(|e| format!("无法获取当前目录: {}", e))?
            .join(path);
        Ok(abs.display().to_string())
    }
}
fn append_log(text: &[u8]) {
    if let Ok(mut guard) = LOG_BUFFER.lock() {
        guard.extend_from_slice(text);
    }
}
fn append_log_str(text: &str) {
    append_log(text.as_bytes());
}
fn openp2p_log_file(working_dir: &Path) -> PathBuf {
    working_dir.join("log").join("openp2p.txt")
}
fn openp2p_legacy_log_file(working_dir: &Path) -> PathBuf {
    working_dir.join("log").join("openp2p.log")
}
fn clear_openp2p_log_files(working_dir: &Path) {
    let log_dir = working_dir.join("log");
    if let Err(e) = fs::create_dir_all(&log_dir) {
        append_log_str(&format!("[RTLauncher] ⚠ 创建日志目录失败: {}\n", e));
        return;
    }

    for log_file in [
        openp2p_log_file(working_dir),
        openp2p_legacy_log_file(working_dir),
    ] {
        let _ = std::fs::File::create(log_file);
    }

    if let Ok(mut offset) = OPENP2P_TXT_OFFSET.lock() {
        *offset = 0;
    }
    if let Ok(mut offset) = OPENP2P_LOG_OFFSET.lock() {
        *offset = 0;
    }
}

fn read_log_increment(path: &Path, offset: &Mutex<u64>) -> Vec<u8> {
    let file_size = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(_) => return Vec::new(),
    };
    let mut offset_guard = match offset.lock() {
        Ok(guard) => guard,
        Err(_) => return Vec::new(),
    };

    if file_size < *offset_guard {
        *offset_guard = 0;
    }
    if file_size == *offset_guard {
        return Vec::new();
    }

    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    if file.seek(std::io::SeekFrom::Start(*offset_guard)).is_err() {
        return Vec::new();
    }

    let mut buffer = Vec::new();
    if file
        .take(file_size - *offset_guard)
        .read_to_end(&mut buffer)
        .is_err()
    {
        return Vec::new();
    }

    *offset_guard = file_size;
    buffer
}

fn has_openp2p_system_process() -> bool {
    let system = sysinfo::System::new_all();
    system.processes().values().any(|process| {
        let name = process.name().to_string_lossy();
        name.eq_ignore_ascii_case("openp2p") || name.eq_ignore_ascii_case("openp2p.exe")
    })
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

fn start_openp2p_with_args(args: &[&str]) -> Result<String, String> {
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
    if let Ok(mut guard) = LOG_BUFFER.lock() {
        guard.clear();
    }
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
    let args = ["-d", "-node", &room_name, "-token", "11661058147873189554"];
    start_openp2p_with_args(&args)
}
#[command]
pub fn mp_encode_room_info(room_name: String, port_count: String) -> String {
    use base64::{engine::general_purpose, Engine as _};
    let combined = format!("{},{}", room_name, port_count);
    general_purpose::STANDARD.encode(combined)
}
#[command]
pub fn mp_start_openp2p_join(encoded_value: String, player_name: String) -> Result<String, String> {
    use base64::{engine::general_purpose, Engine as _};
    let decoded = general_purpose::STANDARD
        .decode(&encoded_value)
        .map_err(|e| format!("Base64 解码失败: {}", e))?;
    let decoded_str = String::from_utf8(decoded)
        .map_err(|e| format!("解码后的字节不是有效的 UTF-8 字符串: {}", e))?;
    let parts: Vec<&str> = decoded_str.split(',').collect();
    if parts.len() != 2 {
        return Err("解码后的字符串格式不正确，应为: 房间名,端口号".to_string());
    }
    let room_name = parts[0];
    let port = parts[1];
    let args = [
        "-d",
        "-node",
        &player_name,
        "-token",
        "11661058147873189554",
        "-appname",
        "RTlauncher",
        "-peernode",
        room_name,
        "-dstip",
        "127.0.0.1",
        "-dstport",
        port,
        "-srcport",
        port,
        "-protocol",
        "tcp",
    ];
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
            for process in system.processes().values().filter(|process| {
                let name = process.name().to_string_lossy();
                name.eq_ignore_ascii_case("openp2p") || name.eq_ignore_ascii_case("openp2p.exe")
            }) {
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
    let mut content = if let Ok(mut guard) = LOG_BUFFER.lock() {
        std::mem::take(&mut *guard)
    } else {
        Vec::new()
    };

    if let Ok(working_dir) = get_openp2p_dir() {
        for chunk in [
            read_log_increment(&openp2p_log_file(&working_dir), &OPENP2P_TXT_OFFSET),
            read_log_increment(&openp2p_legacy_log_file(&working_dir), &OPENP2P_LOG_OFFSET),
        ] {
            if !chunk.is_empty() {
                if !content.is_empty() && !content.ends_with(b"\n") {
                    content.push(b'\n');
                }
                content.extend_from_slice(&chunk);
            }
        }
    }

    String::from_utf8_lossy(&content).to_string()
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
    #[cfg(target_os = "windows")]
    use super::quote_windows_argument;
    use super::read_log_increment;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

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
