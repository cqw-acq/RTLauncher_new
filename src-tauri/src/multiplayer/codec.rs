use base64::{engine::general_purpose, Engine as _};

const OPENP2P_TOKEN: &str = "11661058147873189554";

pub(super) fn encode_room_info(room_name: &str, port: &str) -> String {
    general_purpose::STANDARD.encode(format!("{},{}", room_name, port))
}

pub(super) fn host_arguments(room_name: &str) -> Vec<String> {
    ["-d", "-node", room_name, "-token", OPENP2P_TOKEN]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

pub(super) fn join_arguments(
    encoded_value: &str,
    player_name: &str,
) -> Result<Vec<String>, String> {
    let decoded = general_purpose::STANDARD
        .decode(encoded_value)
        .map_err(|error| format!("Base64 解码失败: {}", error))?;
    let decoded_text = String::from_utf8(decoded)
        .map_err(|error| format!("解码后的字节不是有效的 UTF-8 字符串: {}", error))?;
    let parts: Vec<&str> = decoded_text.split(',').collect();
    if parts.len() != 2 {
        return Err("解码后的字符串格式不正确，应为: 房间名,端口号".to_string());
    }

    Ok([
        "-d",
        "-node",
        player_name,
        "-token",
        OPENP2P_TOKEN,
        "-appname",
        "RTlauncher",
        "-peernode",
        parts[0],
        "-dstip",
        "127.0.0.1",
        "-dstport",
        parts[1],
        "-srcport",
        parts[1],
        "-protocol",
        "tcp",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect())
}
