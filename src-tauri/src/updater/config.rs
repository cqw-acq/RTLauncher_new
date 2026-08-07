use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_FILE_NAME: &str = "launcher.json";
const MIN_CHECK_INTERVAL_SECONDS: i64 = 24 * 3600;

const UPDATE_ENDPOINT: &str = "https://api.gitcode.com/api/v5/repos/bubulaladdi/RTLauncher/releases";

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
            download_path: None,
            download_progress: None,
            status: UpdateStatus::Idle,
        }
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

pub fn get_update_endpoint() -> String {
    UPDATE_ENDPOINT.to_string()
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
            Ok(text) => serde_json::from_str::<Value>(&text).unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
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
            let diff = now - last_time;
            diff >= MIN_CHECK_INTERVAL_SECONDS
        }
        None => true,
    }
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

    "unknown".to_string()
}

pub fn matches_asset_name(asset_name: &str) -> bool {
    let os = get_current_os();

    if asset_name.contains(&os) {
        return true;
    }

    match os.as_str() {
        "windows-x86_64" | "windows-aarch64" => {
            asset_name.ends_with(".exe")
                && !asset_name.contains("macos")
                && !asset_name.contains("linux")
        }
        "macos-aarch64" | "macos-x86_64" => {
            asset_name.contains("macos")
                || asset_name.contains("darwin")
                || asset_name.ends_with(".dmg")
        }
        "linux-x86_64" | "linux-aarch64" => {
            asset_name.contains("linux")
        }
        _ => false,
    }
}