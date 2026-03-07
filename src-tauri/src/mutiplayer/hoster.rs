use std::path::PathBuf;
use std::process::{Command, Child};
use std::sync::Mutex;
use std::env;
use std::io;

/// openp2p 可执行文件名（Windows 用 .exe，其他平台不加后缀）
const OPENP2P_BIN: &str = if cfg!(target_os = "windows") {
    "openp2p.exe"
} else {
    "openp2p"
};

/// 全局 openp2p 进程句柄
static OPENP2P_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// 获取 bridge 目录路径
///
/// - macOS:   ~/Library/Application Support/RTLauncher/bridge
/// - Linux:   <exe_dir>/RTL/bridge
/// - Windows: <exe_dir>/RTL/bridge
fn get_bridge_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME")
            .map_err(|_| "无法获取 HOME 环境变量".to_string())?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("RTLauncher")
            .join("bridge"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let exe_dir = env::current_exe()
            .map_err(|e| format!("无法获取当前可执行文件路径: {}", e))?
            .parent()
            .ok_or_else(|| "无法获取可执行文件父目录".to_string())?
            .to_path_buf();
        Ok(exe_dir.join("RTL").join("bridge"))
    }
}

/// 检查当前进程是否具有管理员权限
#[cfg(target_os = "windows")]
fn is_admin() -> io::Result<bool> {
    use winapi::um::winnt::HANDLE;
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::processthreadsapi::{OpenProcessToken, GetCurrentProcess};
    use winapi::um::winnt::{TokenElevation, TOKEN_QUERY};
    use std::mem;

    unsafe {
        let mut token: HANDLE = mem::zeroed();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut elevation = winapi::um::winnt::TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION>() as u32;

        if GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut size,
        ) == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(elevation.TokenIsElevated != 0)
    }
}

#[cfg(not(target_os = "windows"))]
fn is_admin() -> io::Result<bool> {
    // 在Unix系统上，检查是否为root用户
    Ok(nix::unistd::Uid::effective().is_root())
}

/// 以管理员权限运行命令（Windows）
#[cfg(target_os = "windows")]
fn run_as_admin(path: &str, args: &[&str]) -> io::Result<()> {
    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_SHOW;
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    let path_wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let args_str = args.join(" ");
    let args_wide: Vec<u16> = OsStr::new(&args_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let runas: Vec<u16> = OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            runas.as_ptr(),
            path_wide.as_ptr(),
            args_wide.as_ptr(),
            std::ptr::null(),
            SW_SHOW,
        );

        if result as i32 <= 32 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

/// 以管理员权限运行命令（macOS）
#[cfg(target_os = "macos")]
fn run_as_admin(path: &str, args: &[&str]) -> io::Result<()> {
    let args_str = args.join(" ");
    Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "do shell script \"{} {}\" with administrator privileges",
            path, args_str
        ))
        .spawn()?;
    Ok(())
}

/// 以管理员权限运行命令（Linux）
#[cfg(target_os = "linux")]
fn run_as_admin(path: &str, args: &[&str]) -> io::Result<()> {
    let mut cmd = Command::new("pkexec");
    cmd.arg(path);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.spawn()?;
    Ok(())
}

/// 查找并运行openp2p程序

pub fn run_openp2p(room_name: &str) -> Result<String, String> {
    // 构建 bridge 文件夹路径（平台相关）
    let bridge_dir = get_bridge_dir()?;

    // 构建 openp2p 可执行文件的完整路径
    let openp2p_path = bridge_dir.join(OPENP2P_BIN);

    // 转换为绝对路径
    let absolute_path = openp2p_path.canonicalize()
        .map_err(|e| format!("无法获取 {} 的绝对路径: {}", OPENP2P_BIN, e))?;;

    // 转换为字符串并将反斜杠替换为正斜杠
    let original_path = absolute_path.to_str()
        .ok_or_else(|| "路径包含无效的UTF-8字符".to_string())?
        .to_string();

    // 转换为字符串并将反斜杠替换为正斜杠用于普通运行
    let path_str = original_path.replace("\\", "/");

    // 构建命令参数
    let args = [
        "-d",
        "-node",
        room_name,
        "-token",
        "11661058147873189554"
    ];

    // 尝试直接运行
    let mut cmd = Command::new(&path_str);
    for arg in &args {
        cmd.arg(arg);
    }
    
    match cmd.spawn() {
        Ok(child) => {
            let mut guard = OPENP2P_PROCESS.lock()
                .map_err(|_| "无法锁定进程句柄".to_string())?;
            *guard = Some(child);
            Ok(path_str)
        }
        Err(e) => {
            // 如果直接运行失败，尝试以管理员权限运行
            if !is_admin().unwrap_or(false) {
                match run_as_admin(&original_path, &args) {
                    Ok(_) => Ok(path_str),
                    Err(admin_err) => Err(format!("启动openp2p失败: {}，以管理员权限启动也失败: {}", e, admin_err))
                }
            } else {
                Err(format!("启动openp2p失败: {}", e))
            }
        }
    }
}

/// 将房间名和端口数用逗号分隔后进行Base64编码
pub fn encode_info(room_name: &str, port_count: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    
    // 将房间名和端口数用逗号分隔
    let combined = format!("{},{}", room_name, port_count);
    
    // 进行Base64编码
    let encoded = general_purpose::STANDARD.encode(combined);
    
    // 输出编码后的内容
    println!("编码后的内容: {}", encoded);
    
    encoded
}

/// 解码base64编码的值并启动openp2p
///
/// 参数:
/// - encoded_value: base64编码后的值
/// - player_name: 玩家名
///
/// 返回:
/// - Result<String, String>: 成功返回 openp2p 程序的路径，失败返回错误信息
pub fn start_openp2p_with_encoded_info(encoded_value: &str, player_name: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose};
    
    // 解码base64编码的值
    let decoded = general_purpose::STANDARD.decode(encoded_value)
        .map_err(|e| format!("Base64解码失败: {}", e))?;
    
    // 将解码后的字节转换为字符串
    let decoded_str = String::from_utf8(decoded)
        .map_err(|e| format!("解码后的字节不是有效的UTF-8字符串: {}", e))?;
    
    // 按逗号分割字符串，获取房间名和端口号
    let parts: Vec<&str> = decoded_str.split(',')
        .collect();
    
    if parts.len() != 2 {
        return Err("解码后的字符串格式不正确，应为:房间名,端口号".to_string());
    }
    
    let room_name = parts[0];
    let port = parts[1];
    
    println!("解码后的房间名: {}", room_name);
    println!("解码后的端口号: {}", port);
    
    // 构建命令参数
    let args = [
        "-d",
        "-node",
        player_name,
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
        "tcp"
    ];
    
    // 调用通用的启动函数
    start_openp2p_with_args(&args)
}

/// 使用指定参数启动openp2p
///
/// 参数:
/// - args: 命令行参数数组
///
/// 返回:
/// - Result<String, String>: 成功返回 openp2p 程序的路径，失败返回错误信息
fn start_openp2p_with_args(args: &[&str]) -> Result<String, String> {
    // 构建 bridge 文件夹路径（平台相关）
    let bridge_dir = get_bridge_dir()?;
    let openp2p_path = bridge_dir.join(OPENP2P_BIN);

    // 检查 openp2p 可执行文件是否存在
    if !openp2p_path.exists() {
        return Err(format!("{} 不存在于: {}，请确保 bridge 文件夹中有 {} 文件", OPENP2P_BIN, openp2p_path.display(), OPENP2P_BIN));
    }

    // 转换为绝对路径
    let absolute_path = openp2p_path.canonicalize()
        .map_err(|e| format!("无法获取 {} 的绝对路径: {}", OPENP2P_BIN, e))?;

    // 转换为字符串并将反斜杠替换为正斜杠
    let original_path = absolute_path.to_str()
        .ok_or_else(|| "路径包含无效的UTF-8字符".to_string())?
        .to_string();

    // 转换为字符串并将反斜杠替换为正斜杠用于普通运行
    let path_str = original_path.replace("\\", "/");

    // 尝试直接运行
    let mut cmd = Command::new(&path_str);
    for arg in args {
        cmd.arg(arg);
    }

    match cmd.spawn() {
        Ok(child) => {
            let mut guard = OPENP2P_PROCESS.lock()
                .map_err(|_| "无法锁定进程句柄".to_string())?;
            *guard = Some(child);
            Ok(path_str)
        }
        Err(e) => {
            // 如果直接运行失败，尝试以管理员权限运行
            if !is_admin().unwrap_or(false) {
                match run_as_admin(&original_path, args) {
                    Ok(_) => Ok(path_str),
                    Err(admin_err) => Err(format!("启动openp2p失败: {}，以管理员权限启动也失败: {}", e, admin_err))
                }
            } else {
                Err(format!("启动openp2p失败: {}", e))
            }
        }
    }
}

/// 终止 openp2p 进程
pub fn kill_openp2p() -> Result<(), String> {
    let mut guard = OPENP2P_PROCESS.lock()
        .map_err(|_| "无法锁定进程句柄".to_string())?;
    if let Some(child) = guard.as_mut() {
        child.kill().map_err(|e| format!("终止 openp2p 失败: {}", e))?;
        let _ = child.wait();
    }
    *guard = None;
    Ok(())
}

/// 将拖入的 openp2p 二进制文件安装到 bridge 目录
///
/// 返回安装后的完整路径
pub fn install_openp2p(src_path: &str) -> Result<String, String> {
    let bridge_dir = get_bridge_dir()?;
    std::fs::create_dir_all(&bridge_dir)
        .map_err(|e| format!("创建 bridge 目录失败: {}", e))?;

    let dest = bridge_dir.join(OPENP2P_BIN);
    std::fs::copy(src_path, &dest)
        .map_err(|e| format!("复制文件失败: {}", e))?;

    // Unix 下设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)
            .map_err(|e| format!("获取文件属性失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)
            .map_err(|e| format!("设置执行权限失败: {}", e))?;
    }

    Ok(dest.to_string_lossy().to_string())
}

/// 检查 openp2p 是否已安装
pub fn check_openp2p() -> bool {
    get_bridge_dir()
        .map(|d| d.join(OPENP2P_BIN).exists())
        .unwrap_or(false)
}