/// 为离线玩家生成 Minecraft 标准 UUID v3。
pub(super) fn offline_uuid(player_name: &str) -> String {
    use md5::{Digest, Md5};

    let mut digest = Md5::digest(format!("OfflinePlayer:{player_name}").as_bytes());
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3],
        digest[4], digest[5],
        digest[6], digest[7],
        digest[8], digest[9],
        digest[10], digest[11], digest[12], digest[13], digest[14], digest[15]
    )
}

/// 检查是否是合法 UUID 格式
pub(super) fn is_valid_uuid(s: &str) -> bool {
    match s.len() {
        32 => s.bytes().all(|byte| byte.is_ascii_hexdigit()),
        36 => s.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        }),
        _ => false,
    }
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
    use super::{is_valid_uuid, offline_uuid};

    #[test]
    fn generates_minecraft_offline_uuid_vectors() {
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(
            offline_uuid("Steve"),
            "5627dd98-e6be-3c21-b8a8-e92344183641"
        );
    }

    #[test]
    fn accepts_only_compact_or_canonical_uuid_layouts() {
        assert!(is_valid_uuid("b50ad385829d3141a2167e7d7539ba7f"));
        assert!(is_valid_uuid("b50ad385-829d-3141-a216-7e7d7539ba7f"));
        assert!(!is_valid_uuid("b50ad385829d3141-a2167e7d7539ba7f"));
        assert!(!is_valid_uuid("b50ad385-829d3141-a216-7e7d7539ba7f"));
    }
}
