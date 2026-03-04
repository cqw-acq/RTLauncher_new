pub mod littleskinLoader;
pub mod yissadrail;
pub mod official;

use serde::{Deserialize, Serialize};

/// 获取平台配置目录：
/// - macOS: ~/Library/Application Support/RTLauncher/config
/// - Linux/Windows: ./RTL/config
pub fn config_dir() -> String {
    #[cfg(target_os = "macos")]
    let dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/Library/Application Support/RTLauncher/config", home)
    };

    #[cfg(not(target_os = "macos"))]
    let dir = "./RTL/config".to_string();

    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// 获取数据库路径
pub fn db_path() -> String {
    format!("{}/LaunchAccount.db", config_dir())
}

/// 统一的账户信息，前端可直接使用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// 玩家名
    pub name: String,
    /// 玩家 UUID
    pub uuid: String,
    /// 登录类型: "littleskin" | "third_party" | "offline"
    pub auth_type: String,
    /// access_token (可能为空)
    pub access_token: String,
    /// 皮肤 URL (可选)
    pub skin_url: Option<String>,
}

/// 第三方登录后获取到的角色列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyAccountList {
    pub access_token: String,
    pub profiles: Vec<ThirdPartyProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyProfile {
    pub id: String,
    pub name: String,
}
