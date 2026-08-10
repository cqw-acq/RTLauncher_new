use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Java安装信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaInstallationInfo {
    pub path: String,
    pub version: String,
    pub major_version: i32,
    pub vendor: String,
    pub architecture: String,
    #[serde(default)]
    pub java_type: String,
}

/// 获取平台配置目录（与 auth::config_dir 一致）
fn config_dir() -> String {
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
    PathBuf::from(config_dir()).join("launcher.json")
}

/// 获取平台默认游戏目录
fn default_minecraft_path() -> String {
    #[cfg(target_os = "windows")]
    {
        // Windows: %APPDATA%/.minecraft
        if let Ok(appdata) = std::env::var("APPDATA") {
            return std::path::PathBuf::from(appdata)
                .join(".minecraft")
                .to_string_lossy()
                .to_string();
        }
        // 回退到启动器所在目录
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        return exe_dir.join(".minecraft").to_string_lossy().to_string();
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: ~/Library/Application Support/minecraft
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/Library/Application Support/minecraft", home);
        }
        "./.minecraft".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        crate::app_paths::linux_minecraft_dir()
            .to_string_lossy()
            .to_string()
    }
}

/// HMCL 配置文件结构（部分字段）
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct HMCLConfig {
    #[serde(default)]
    #[allow(dead_code)]
    commonDirType: Option<String>,
    #[serde(default)]
    commonDirectory: Option<String>,
    #[serde(default)]
    configurations: Option<serde_json::Value>,
}

/// 从 HMCL 配置文件中提取游戏目录
fn detect_hmcl_minecraft_paths() -> Vec<String> {
    let mut paths = Vec::new();

    // 收集可能的 HMCL 配置文件位置
    let mut config_files = Vec::new();

    // 便携模式：当前目录下的 .hmcl/hmcl.json
    config_files.push(PathBuf::from(".hmcl").join("hmcl.json"));

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            config_files.push(PathBuf::from(appdata).join(".hmcl").join("hmcl.json"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            config_files.push(PathBuf::from(home.clone()).join(".config").join("hmcl").join("hmcl.json"));
            config_files.push(PathBuf::from(home).join(".hmcl.json"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            config_files.push(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("hmcl")
                .join("hmcl.json"));
        }
    }

    for config_file in config_files {
        if !config_file.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&config_file) {
            if let Ok(hmcl_config) = serde_json::from_str::<HMCLConfig>(&content) {
                // 优先读取 commonDirectory（自定义通用目录）
                if let Some(common_dir) = hmcl_config.commonDirectory {
                    if !common_dir.is_empty() {
                        paths.push(common_dir);
                    }
                }

                // 从 configurations 中读取各个实例的游戏目录
                if let Some(configs) = hmcl_config.configurations {
                    if let Some(configs_obj) = configs.as_object() {
                        for (_instance_id, instance_config) in configs_obj {
                            if let Some(game_dir) = instance_config.get("gameDir").and_then(|v| v.as_str()) {
                                if !game_dir.is_empty() {
                                    paths.push(game_dir.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 去重
    paths.sort();
    paths.dedup();
    paths
}

/// 智能判断目录是否为有效的 Minecraft 游戏目录
/// 通过检查关键文件和目录结构来区分 .minecraft 和 .hmcl
fn is_valid_minecraft_directory(path: &Path) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    // 检查是否包含 HMCL 特有的文件（如果是，则不是 minecraft 目录）
    let hmcl_indicators = [".hmcl", "hmcl.json", "hmclversion.cfg"];
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if hmcl_indicators.iter().any(|&indicator| name == indicator) {
                return false;
            }
        }
    }

    // 重新读取目录以检查 Minecraft 标准目录结构
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    // 检查 Minecraft 标准目录结构
    let mc_indicators = ["versions", "libraries", "assets", "saves", "resourcepacks"];
    
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if mc_indicators.iter().any(|&indicator| name == indicator) {
                return true;
            }
        }
    }

    false
}

/// 存储在 launcher.json 中的路径配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherPathsConfig {
    /// Java 可执行文件路径列表
    pub java_paths: Vec<String>,
    /// 当前选中的 Java 路径
    pub selected_java_path: String,
    /// Java安装信息映射 (major_version -> JavaInstallationInfo)
    #[serde(default)]
    pub java_installations: HashMap<String, JavaInstallationInfo>,
    /// 游戏目录路径列表
    pub minecraft_paths: Vec<String>,
    /// 当前选中的游戏目录
    pub selected_minecraft_path: String,
    /// 平台默认 .minecraft 路径（只读，仅供展示）
    pub default_minecraft_path: String,
}

impl Default for LauncherPathsConfig {
    fn default() -> Self {
        let def_mc = default_minecraft_path();
        Self {
            java_paths: Vec::new(),
            selected_java_path: String::new(),
            java_installations: HashMap::new(),
            minecraft_paths: vec![def_mc.clone()],
            selected_minecraft_path: def_mc.clone(),
            default_minecraft_path: def_mc,
        }
    }
}

/// 读取 launcher.json，不存在时返回默认值
#[tauri::command]
pub fn get_launcher_paths_config() -> LauncherPathsConfig {
    let path = launcher_config_path();
    let def = LauncherPathsConfig::default();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(text) => {
                let mut cfg: LauncherPathsConfig =
                    serde_json::from_str(&text).unwrap_or_else(|_| def.clone());
                // 始终填充最新的平台默认路径
                cfg.default_minecraft_path = default_minecraft_path();
                // 若 minecraft_paths 为空，加入默认路径
                if cfg.minecraft_paths.is_empty() {
                    cfg.minecraft_paths.push(cfg.default_minecraft_path.clone());
                }
                if cfg.selected_minecraft_path.is_empty() {
                    cfg.selected_minecraft_path = cfg.minecraft_paths[0].clone();
                }
                // 去重 java_paths，保持顺序
                {
                    let mut seen = std::collections::HashSet::new();
                    let mut dedup = Vec::new();
                    for p in cfg.java_paths.drain(..) {
                        if seen.insert(p.clone()) {
                            dedup.push(p);
                        }
                    }
                    cfg.java_paths = dedup;
                }
                // 去重 minecraft_paths，保持顺序
                {
                    let mut seen = std::collections::HashSet::new();
                    let mut dedup = Vec::new();
                    for p in cfg.minecraft_paths.drain(..) {
                        if seen.insert(p.clone()) {
                            dedup.push(p);
                        }
                    }
                    cfg.minecraft_paths = dedup;
                }
                
                // 从 HMCL 配置文件检测游戏目录并添加到列表
                let hmcl_paths = detect_hmcl_minecraft_paths();
                for hmcl_path in hmcl_paths {
                    // 只添加通过验证的 Minecraft 目录，避免误判 .hmcl 目录
                    if is_valid_minecraft_directory(Path::new(&hmcl_path)) && !cfg.minecraft_paths.contains(&hmcl_path) {
                        cfg.minecraft_paths.push(hmcl_path);
                    }
                }
                
                cfg
            }
            Err(_) => def,
        }
    } else {
        // 即使配置文件不存在，也尝试从 HMCL 检测路径
        let mut cfg = def.clone();
        let hmcl_paths = detect_hmcl_minecraft_paths();
        for hmcl_path in hmcl_paths {
            if is_valid_minecraft_directory(Path::new(&hmcl_path)) && !cfg.minecraft_paths.contains(&hmcl_path) {
                cfg.minecraft_paths.push(hmcl_path);
            }
        }
        cfg
    }
}

/// 获取 Java 下载目录（应用数据目录下的 java 子目录）
#[tauri::command]
pub fn get_java_download_dir() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/Library/Application Support/RTLauncher/java", home)
    };

    #[cfg(target_os = "windows")]
    let dir = "./RTL/java".to_string();

    #[cfg(target_os = "linux")]
    let dir = crate::app_paths::linux_java_dir()
        .to_string_lossy()
        .to_string();

    let _ = fs::create_dir_all(&dir);
    Ok(dir)
}

/// 将 launcher.json 持久化
#[tauri::command]
pub fn save_launcher_paths_config(config: LauncherPathsConfig) -> Result<(), String> {
    let path = launcher_config_path();
    // 写前去重
    let mut cfg = config;
    {
        let mut seen = std::collections::HashSet::new();
        let mut dedup = Vec::new();
        for p in cfg.java_paths.drain(..) {
            if seen.insert(p.clone()) {
                dedup.push(p);
            }
        }
        cfg.java_paths = dedup;
    }
    {
        let mut seen = std::collections::HashSet::new();
        let mut dedup = Vec::new();
        for p in cfg.minecraft_paths.drain(..) {
            if seen.insert(p.clone()) {
                dedup.push(p);
            }
        }
        cfg.minecraft_paths = dedup;
    }
    
    // 保存时也重新检测 HMCL 路径，确保不会丢失 HMCL 检测到的路径
    let hmcl_paths = detect_hmcl_minecraft_paths();
    for hmcl_path in hmcl_paths {
        if is_valid_minecraft_directory(Path::new(&hmcl_path)) && !cfg.minecraft_paths.contains(&hmcl_path) {
            cfg.minecraft_paths.push(hmcl_path);
        }
    }
    
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}
