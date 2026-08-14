/// 为离线玩家生成稳定的 UUID（基于玩家名称；算法不保证与 RFC 4122 v3 完全一致）
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_stable_and_shaped_like_a_uuid() {
        let first = offline_uuid("Steve");
        let second = offline_uuid("Steve");
        assert_eq!(first, second, "same player name should hash to the same UUID");
        assert!(is_valid_uuid(&first));

        let parts: Vec<&str> = first.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn offline_uuid_differs_for_different_player_names() {
        assert_ne!(offline_uuid("Steve"), offline_uuid("Alex"));
    }

    #[test]
    fn is_valid_uuid_accepts_hyphenated_and_bare_hex_strings() {
        assert!(is_valid_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf5"));
        assert!(is_valid_uuid("069a79f444e94726a5befca90e38aaf5"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn is_valid_uuid_rejects_wrong_length_or_non_hex_input() {
        assert!(!is_valid_uuid(""));
        assert!(!is_valid_uuid("not-a-uuid"));
        // 31 个十六进制字符，长度不足
        assert!(!is_valid_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf"));
        // 含有非十六进制字符 g
        assert!(!is_valid_uuid("069a79f4-44e9-4726-a5be-fca90e38aafg"));
    }

    #[test]
    fn chooses_mojang_identity_whenever_a_yggdrasil_api_is_configured() {
        assert_eq!(
            launch_auth_identity("", "https://example.invalid/api/yggdrasil"),
            ("mojang", "{}")
        );
        // yggdrasil_api 优先于 auth_token 的状态，即便 token 看起来像正版 token
        assert_eq!(
            launch_auth_identity("microsoft-token", "  https://example.invalid  "),
            ("mojang", "{}")
        );
    }

    #[test]
    fn treats_whitespace_only_yggdrasil_api_as_absent() {
        assert_eq!(launch_auth_identity("0", "   "), ("legacy", "{}"));
        assert_eq!(launch_auth_identity("microsoft-token", "   "), ("msa", "{}"));
    }

    #[test]
    fn treats_empty_or_placeholder_token_as_legacy() {
        assert_eq!(launch_auth_identity("", ""), ("legacy", "{}"));
        assert_eq!(launch_auth_identity("  0  ", ""), ("legacy", "{}"));
    }
}
