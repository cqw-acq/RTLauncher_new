use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_FILE_NAME: &str = "launcher.json";
const MIN_CHECK_INTERVAL_SECONDS: i64 = 60;

const LIGHTING_TEAM_UPDATE_ENDPOINT: &str =
    "http://update-service.lighting-team.com/api/v1/versions";
const GITHUB_UPDATE_ENDPOINT: &str = "https://api.github.com/repos/cqw-acq/RTLauncher_new/releases";
const LIGHTING_TEAM_ASSET_HOST: &str =
    "7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la";

/// 允许的更新下载域名白名单。release URL 或重定向目标必须落在这些域名上。
pub const TRUSTED_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "githubusercontent.com",
    LIGHTING_TEAM_ASSET_HOST,
];

/// 发布 Release 时，可选地在 Release body 里放一个 SHA-256 清单，匹配格式：
///   `SHA256 (filename) = hexhash`  或  `hexhash  filename`  或  `hexhash *filename`
/// 如果找到与下载附件同名的条目，就必须在安装前通过 SHA-256 校验（fail closed）。
pub const HASH_TAG: &str = "SHA256SUMS";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default)]
    pub last_check_time: Option<i64>,
    #[serde(default)]
    pub current_version: String,
    #[serde(default)]
    pub target_version: Option<String>,
    #[serde(default)]
    pub target_os: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
    /// 期望的 SHA-256 校验和（小写 16 进制，64 字符）。从 Release body 解析得到后持久化。
    /// 如果为 Some，则下载后 **必须** 校验匹配（fail closed）。
    #[serde(default)]
    pub expected_sha256: Option<String>,
    #[serde(default)]
    pub download_path: Option<String>,
    #[serde(default)]
    pub download_progress: Option<f64>,
    #[serde(default)]
    pub status: UpdateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateStatus {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "checking")]
    Checking,
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "downloaded")]
    Downloaded,
    #[serde(rename = "error")]
    Error(String),
}

impl Default for UpdateStatus {
    fn default() -> Self {
        UpdateStatus::Idle
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            last_check_time: None,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            target_version: None,
            target_os: None,
            download_url: None,
            file_size: None,
            expected_sha256: None,
            download_path: None,
            download_progress: None,
            status: UpdateStatus::Idle,
        }
    }
}

/// 严格校验 64 字符小写十六进制 SHA-256 字符串。
pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// 校验 URL 主机名是否落在受信任白名单中。
pub fn is_trusted_download_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    if parsed.scheme() != "https" {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    TRUSTED_DOWNLOAD_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed)))
}

/// 校验 Release 元数据给出的初始附件 URL。初始 URL 必须来自指定仓库或指定存储桶；
/// 下载过程中的 GitHub CDN 重定向由 `is_trusted_download_url` 单独校验。
pub fn is_trusted_release_asset_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(url) if url.scheme() == "https" => url,
        _ => return false,
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };

    match host {
        "github.com" => parsed
            .path()
            .starts_with("/cqw-acq/RTLauncher_new/releases/download/"),
        LIGHTING_TEAM_ASSET_HOST => parsed.path().starts_with("/RTL/releases/"),
        _ => false,
    }
}

pub fn config_dir() -> String {
    #[cfg(target_os = "macos")]
    let dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/Library/Application Support/RTLauncher/config", home)
    };

    #[cfg(target_os = "linux")]
    let dir = crate::app_paths::linux_config_dir()
        .to_string_lossy()
        .to_string();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let dir = "./RTL/config".to_string();

    let _ = fs::create_dir_all(&dir);
    dir
}

fn launcher_config_path() -> PathBuf {
    PathBuf::from(config_dir()).join(CONFIG_FILE_NAME)
}

fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs() as i64
}

pub fn get_update_endpoints() -> [&'static str; 2] {
    [LIGHTING_TEAM_UPDATE_ENDPOINT, GITHUB_UPDATE_ENDPOINT]
}

pub fn get_update_config() -> UpdateConfig {
    let path = launcher_config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(text) => {
                if let Ok(mut value) = serde_json::from_str::<Value>(&text) {
                    if let Some(update) = value.get_mut("update") {
                        if let Ok(cfg) = serde_json::from_value::<UpdateConfig>(update.clone()) {
                            return cfg;
                        }
                    }
                }
                UpdateConfig::default()
            }
            Err(_) => UpdateConfig::default(),
        }
    } else {
        UpdateConfig::default()
    }
}

pub fn save_update_config(update_cfg: UpdateConfig) -> Result<(), String> {
    let path = launcher_config_path();
    let mut value: Value = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Value>(&text)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
            Err(_) => Value::Object(serde_json::Map::new()),
        }
    } else {
        Value::Object(serde_json::Map::new())
    };

    value["update"] = serde_json::to_value(&update_cfg).map_err(|e| e.to_string())?;

    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

pub fn should_check_update() -> bool {
    let cfg = get_update_config();
    match cfg.last_check_time {
        Some(last_time) => {
            let now = get_current_timestamp();
            has_check_interval_elapsed(last_time, now)
        }
        None => true,
    }
}

fn has_check_interval_elapsed(last_check: i64, now: i64) -> bool {
    now.saturating_sub(last_check) >= MIN_CHECK_INTERVAL_SECONDS
}

pub fn update_last_check_time() -> Result<(), String> {
    let mut cfg = get_update_config();
    cfg.last_check_time = Some(get_current_timestamp());
    save_update_config(cfg)
}

pub fn get_current_os() -> String {
    #[cfg(target_os = "windows")]
    {
        if cfg!(target_arch = "x86_64") {
            return "windows-x86_64".to_string();
        } else if cfg!(target_arch = "aarch64") {
            return "windows-aarch64".to_string();
        }
        return "windows".to_string();
    }

    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            return "macos-aarch64".to_string();
        }
        return "macos-x86_64".to_string();
    }

    #[cfg(target_os = "linux")]
    {
        if cfg!(target_arch = "x86_64") {
            return "linux-x86_64".to_string();
        } else if cfg!(target_arch = "aarch64") {
            return "linux-aarch64".to_string();
        }
        return "linux".to_string();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return "unknown".to_string();
    }
}

pub fn matches_asset_name_for(asset_name: &str, os: &str) -> bool {
    let name_lower = asset_name.to_lowercase();

    if name_lower.contains(&os.replace('-', "_")) || name_lower.contains(&os) {
        return true;
    }

    let (os_family, arch) = match os {
        "windows-x86_64" => ("windows", "x86_64"),
        "windows-aarch64" => ("windows", "aarch64"),
        "macos-x86_64" => ("macos", "x86_64"),
        "macos-aarch64" => ("macos", "aarch64"),
        "linux-x86_64" => ("linux", "x86_64"),
        "linux-aarch64" => ("linux", "aarch64"),
        _ => return false,
    };

    let arch_alt = match arch {
        "x86_64" => vec!["x86_64", "x64", "amd64"],
        "aarch64" => vec!["aarch64", "arm64", "arm_64"],
        _ => vec![arch],
    };

    let other_arch_keywords: &[&str] = match arch {
        "x86_64" => &["aarch64", "arm64", "arm_64"],
        "aarch64" => &["x86_64", "x64", "amd64"],
        _ => &[],
    };
    if other_arch_keywords
        .iter()
        .any(|keyword| name_lower.contains(keyword))
    {
        return false;
    }

    let family_alt = match os_family {
        "macos" => vec!["macos", "darwin", "mac"],
        "windows" => vec!["windows", "win", "msi", "nsis"],
        f => vec![f],
    };

    for f in &family_alt {
        for a in &arch_alt {
            let pattern1 = format!("{}_{}", f, a);
            let pattern2 = format!("{}-{}", f, a);
            let pattern3 = format!("{}_{}", a, f);
            let pattern4 = format!("{}-{}", a, f);
            if name_lower.contains(&pattern1)
                || name_lower.contains(&pattern2)
                || name_lower.contains(&pattern3)
                || name_lower.contains(&pattern4)
            {
                return true;
            }
        }
    }

    match os_family {
        "windows" => {
            (name_lower.ends_with(".exe")
                || name_lower.ends_with(".msi")
                || name_lower.contains(".nsis"))
                && !name_lower.contains("macos")
                && !name_lower.contains("darwin")
                && !name_lower.contains("linux")
        }
        "macos" => {
            name_lower.contains("macos")
                || name_lower.contains("darwin")
                || name_lower.ends_with(".dmg")
                || name_lower.ends_with(".app")
                || name_lower.ends_with(".app.tar.gz")
        }
        "linux" => {
            name_lower.contains("linux")
                || name_lower.ends_with(".appimage")
                || name_lower.ends_with(".deb")
        }
        _ => false,
    }
}

pub fn get_current_arch_keywords() -> Vec<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        vec!["x86_64", "x64", "amd64"]
    }
    #[cfg(target_arch = "aarch64")]
    {
        vec!["aarch64", "arm64", "arm_64"]
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        vec![]
    }
}

pub fn get_current_os_keywords() -> Vec<&'static str> {
    #[cfg(target_os = "windows")]
    {
        vec!["windows", "win", "msi", "nsis"]
    }
    #[cfg(target_os = "macos")]
    {
        vec!["macos", "darwin", "mac", "dmg", "app"]
    }
    #[cfg(target_os = "linux")]
    {
        vec!["linux", "appimage", "deb"]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_check_cooldown_opens_at_sixty_seconds() {
        assert!(!has_check_interval_elapsed(1_000, 1_059));
        assert!(has_check_interval_elapsed(1_000, 1_060));
    }

    #[test]
    fn updater_accepts_only_configured_release_hosts() {
        assert!(is_trusted_download_url(
            "https://github.com/cqw-acq/RTLauncher_new/releases/download/1.2.0/app.exe"
        ));
        assert!(is_trusted_download_url(
            "https://release-assets.githubusercontent.com/github-production-release-asset/app.exe"
        ));
        assert!(is_trusted_download_url(
            "https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/app.exe"
        ));
        assert!(!is_trusted_download_url(
            "http://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/app.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://gitcode.com/bubulaladdi/RTLauncher/releases/download/1.2.0/app.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://example.com/RTLauncher/app.exe"
        ));
    }

    #[test]
    fn initial_asset_url_is_limited_to_the_configured_repository_or_bucket() {
        assert!(is_trusted_release_asset_url(
            "https://github.com/cqw-acq/RTLauncher_new/releases/download/1.2.0/app.exe"
        ));
        assert!(is_trusted_release_asset_url(
            "https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/app.exe"
        ));
        assert!(!is_trusted_release_asset_url(
            "https://github.com/another-owner/another-repo/releases/download/1.2.0/app.exe"
        ));
        assert!(!is_trusted_release_asset_url(
            "https://another-bucket.tcb.qcloud.la/RTL/releases/1.2.0/app.exe"
        ));
        assert!(!is_trusted_release_asset_url(
            "https://release-assets.githubusercontent.com/github-production-release-asset/app.exe"
        ));
    }
}
