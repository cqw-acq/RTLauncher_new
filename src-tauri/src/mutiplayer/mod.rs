pub mod hoster;

use tauri::command;

/// 房主模式：创建房间并启动 openp2p，返回联机码
///
/// - `room_name`: 房间名（即本机节点名）
/// - `port`: Minecraft 服务器端口（用于编码进联机码）
#[command]
pub fn mp_host_room(room_name: String, port: String) -> Result<String, String> {
    // 先启动 openp2p
    hoster::run_openp2p(&room_name)?;

    // 返回联机码供玩家分享
    let join_code = hoster::encode_info(&room_name, &port);
    Ok(join_code)
}

/// 客机模式：使用联机码加入房间
///
/// - `join_code`: 联机码（Base64 编码的 "房间名,端口" 字符串）
/// - `player_name`: 本机节点名（玩家名）
#[command]
pub fn mp_join_room(join_code: String, player_name: String) -> Result<String, String> {
    hoster::start_openp2p_with_encoded_info(&join_code, &player_name)
}

/// 仅编码房间信息，生成联机码（不启动 openp2p）
#[command]
pub fn mp_encode_info(room_name: String, port: String) -> String {
    hoster::encode_info(&room_name, &port)
}

/// 断开联机：终止 openp2p 进程
#[command]
pub fn mp_disconnect() -> Result<(), String> {
    hoster::kill_openp2p()
}

/// 安装 openp2p：将指定文件复制到 bridge 目录
///
/// - `src_path`: 源文件路径（用户拖入的文件）
#[command]
pub fn mp_install_openp2p(src_path: String) -> Result<String, String> {
    hoster::install_openp2p(&src_path)
}

/// 检查 openp2p 是否已安装
#[command]
pub fn mp_check_openp2p() -> bool {
    hoster::check_openp2p()
}
