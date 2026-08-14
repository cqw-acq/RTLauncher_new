/// 为离线玩家生成稳定的 UUID v3（基于玩家名称）
pub(super) fn offline_uuid(player_name: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let input = format!("OfflinePlayer:{}", player_name);
    let mut h1 = DefaultHasher::new();
    input.hash(&mut h1);
    let hi = h1.finish();
    let mut h2 = DefaultHasher::new();
    format!("{}:salt", input).hash(&mut h2);
    let lo = h2.finish();
    let hi = (hi & 0xFFFFFFFF_FFFF0FFF) | 0x00000000_00003000;
    let lo = (lo & 0x3FFFFFFF_FFFFFFFF) | 0x80000000_00000000;

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (hi >> 32) as u32,
        (hi >> 16) as u16 & 0xFFFF,
        hi as u16 & 0xFFFF,
        (lo >> 48) as u16 & 0xFFFF,
        lo & 0x0000FFFFFFFFFFFF
    )
}

/// 检查是否是合法 UUID 格式
pub(super) fn is_valid_uuid(s: &str) -> bool {
    // 支持 xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx 或无连字符 32位 hex
    let trimmed = s.replace('-', "");
    trimmed.len() == 32 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}

/// 根据账户认证方式生成 Minecraft 的身份参数。
///
/// authlib-injector 使用 Yggdrasil 认证流程，而不是微软账号流程。把第三方
/// access token 伪装成 `msa` 会让新版客户端访问 Minecraft Services 的
/// `/player/attributes`，该服务会以 401 拒绝第三方 token。
pub(super) fn launch_auth_identity(
    auth_token: &str,
    yggdrasil_api: &str,
) -> (&'static str, &'static str) {
    if !yggdrasil_api.trim().is_empty() {
        ("mojang", "{}")
    } else if auth_token.trim().is_empty() || auth_token.trim() == "0" {
        ("legacy", "{}")
    } else {
        // 正版账户仍使用 Microsoft 登录流程；不要伪造 user properties，
        // 客户端会在需要时自行从官方服务取得它们。
        ("msa", "{}")
    }
}
