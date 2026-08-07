use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheResourceKind {
    Mod,
    ResourcePack,
    DataPack,
    World,
    ShaderPack,
    Modpack,
}
impl CacheResourceKind {
    pub fn dir_name(&self) -> &'static str {
        match self {
            CacheResourceKind::Mod => "mods",
            CacheResourceKind::ResourcePack => "resourcepacks",
            CacheResourceKind::DataPack => "datapacks",
            CacheResourceKind::World => "worlds",
            CacheResourceKind::ShaderPack => "shaderpacks",
            CacheResourceKind::Modpack => "modpacks",
        }
    }
    pub fn all() -> &'static [CacheResourceKind] {
        &[
            CacheResourceKind::Mod,
            CacheResourceKind::ResourcePack,
            CacheResourceKind::DataPack,
            CacheResourceKind::World,
            CacheResourceKind::ShaderPack,
            CacheResourceKind::Modpack,
        ]
    }
}
/// Resolve the launcher cache base without creating it.
///
/// Priority:
///   1. User-configured `selected_minecraft_path` from launcher settings
///      (this is the game directory chosen on the Launch page).
///   2. Fall back to the per-platform default directory.
///
/// Read-only commands use this path so merely opening a resource page never
/// mutates the filesystem (and therefore cannot trigger Tauri's dev watcher).
fn cache_base_dir() -> Result<PathBuf, String> {
    // 1) Try to use the user-configured Minecraft directory. This makes
    //    downloads and cache live inside whatever game folder the user
    //    selected on the Launch page (e.g. %APPDATA%\.minecraft or a
    //    custom portable location).
    let user_cfg = super::config::get_launcher_paths_config();
    let selected = user_cfg.selected_minecraft_path.trim();
    if !selected.is_empty() {
        let p = PathBuf::from(selected);
        // Consider it valid if the path exists OR its parent is writable.
        if p.exists() || p.parent().map(|par| par.exists()).unwrap_or(false) {
            return Ok(p);
        }
    }

    // 2) Fall back to the platform default location.
    #[cfg(target_os = "windows")]
    {
        let exe_dir = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(exe_dir.join("minecraft"));
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        return Ok(PathBuf::from(home).join("Library/Application Support/RTLauncher"));
    }
    // Runtime cache must not live below `src-tauri` during development: Tauri's
    // file watcher treats cache writes as source changes and restarts the app.
    #[cfg(target_os = "linux")]
    {
        return Ok(crate::app_paths::linux_cache_dir());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Ok(PathBuf::from("./minecraft"))
    }
}

fn cache_root_path() -> Result<PathBuf, String> {
    Ok(cache_base_dir()?.join("cache"))
}

fn cache_dir_path_for_kind(kind: CacheResourceKind) -> Result<PathBuf, String> {
    Ok(cache_root_path()?.join(kind.dir_name()))
}

fn cache_dir_path_for_version(
    kind: CacheResourceKind,
    mc_version: &str,
) -> Result<PathBuf, String> {
    Ok(cache_dir_path_for_kind(kind)?.join(sanitize_version(mc_version)))
}

pub fn cache_root_dir() -> Result<PathBuf, String> {
    let cache_root = cache_root_path()?;
    std::fs::create_dir_all(&cache_root).map_err(|e| e.to_string())?;
    Ok(cache_root)
}
pub fn get_cache_dir_for_kind(kind: CacheResourceKind) -> Result<PathBuf, String> {
    let dir = cache_dir_path_for_kind(kind)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
pub fn get_cache_dir_for_version(
    kind: CacheResourceKind,
    mc_version: &str,
) -> Result<PathBuf, String> {
    let version_dir = cache_dir_path_for_version(kind, mc_version)?;
    std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
    Ok(version_dir)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModLoaderKind {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    LiteLoader,
    Ornithe,
    Vanilla,
    Custom(String),
}
impl ModLoaderKind {
    pub fn dir_name(&self) -> String {
        match self {
            ModLoaderKind::Forge => "forge".to_string(),
            ModLoaderKind::NeoForge => "neoforge".to_string(),
            ModLoaderKind::Fabric => "fabric".to_string(),
            ModLoaderKind::Quilt => "quilt".to_string(),
            ModLoaderKind::LiteLoader => "liteloader".to_string(),
            ModLoaderKind::Ornithe => "ornithe".to_string(),
            ModLoaderKind::Vanilla => "vanilla".to_string(),
            ModLoaderKind::Custom(name) => name.to_lowercase(),
        }
    }
}

fn mod_cache_dir_path(mc_version: &str, loader: &ModLoaderKind) -> Result<PathBuf, String> {
    Ok(cache_dir_path_for_version(CacheResourceKind::Mod, mc_version)?.join(loader.dir_name()))
}

pub fn get_mod_cache_dir(mc_version: &str, loader: ModLoaderKind) -> Result<PathBuf, String> {
    let loader_dir = mod_cache_dir_path(mc_version, &loader)?;
    std::fs::create_dir_all(&loader_dir).map_err(|e| e.to_string())?;
    Ok(loader_dir)
}
fn sanitize_version(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
pub fn ensure_all_cache_dirs() -> Result<(), String> {
    for kind in CacheResourceKind::all() {
        get_cache_dir_for_kind(*kind)?;
    }
    Ok(())
}
pub fn parse_resource_kind(kind: &str) -> Result<CacheResourceKind, String> {
    match kind.to_ascii_lowercase().as_str() {
        "mod" | "mods" => Ok(CacheResourceKind::Mod),
        "resourcepack" | "resourcepacks" => Ok(CacheResourceKind::ResourcePack),
        "datapack" | "datapacks" => Ok(CacheResourceKind::DataPack),
        "world" | "worlds" => Ok(CacheResourceKind::World),
        "shaderpack" | "shaderpacks" | "shader" => Ok(CacheResourceKind::ShaderPack),
        "modpack" | "modpacks" => Ok(CacheResourceKind::Modpack),
        other => Err(format!("未知的资源类型: {}", other)),
    }
}
pub fn parse_mod_loader(loader: &str) -> Result<ModLoaderKind, String> {
    let trimmed = loader.trim();
    if trimmed.is_empty() {
        return Ok(ModLoaderKind::Vanilla);
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "forge" => Ok(ModLoaderKind::Forge),
        "neoforge" | "neo_forge" | "neoforged" | "neoforge_21_1_99" => Ok(ModLoaderKind::NeoForge),
        "fabric" => Ok(ModLoaderKind::Fabric),
        "quilt" => Ok(ModLoaderKind::Quilt),
        "liteloader" | "lite_loader" | "litemod" => Ok(ModLoaderKind::LiteLoader),
        "ornithe" => Ok(ModLoaderKind::Ornithe),
        "vanilla" | "通用" | "common" => Ok(ModLoaderKind::Vanilla),
        _ => {
            let sanitized: String = lower
                .chars()
                .map(|c| match c {
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                    _ => c,
                })
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
                .collect();
            let sanitized = sanitized.trim();
            if sanitized.is_empty() {
                Ok(ModLoaderKind::Vanilla)
            } else {
                Ok(ModLoaderKind::Custom(sanitized.replace(" ", "_")))
            }
        }
    }
}
#[tauri::command]
pub fn get_cache_root() -> Result<String, String> {
    Ok(cache_root_path()?.to_string_lossy().to_string())
}
#[tauri::command]
pub fn get_cache_dir(kind: String) -> Result<String, String> {
    let resource_kind = parse_resource_kind(&kind)?;
    Ok(cache_dir_path_for_kind(resource_kind)?
        .to_string_lossy()
        .to_string())
}
#[tauri::command]
pub fn get_cache_dir_by_version(kind: String, mc_version: String) -> Result<String, String> {
    let resource_kind = parse_resource_kind(&kind)?;
    Ok(cache_dir_path_for_version(resource_kind, &mc_version)?
        .to_string_lossy()
        .to_string())
}
#[tauri::command]
pub fn init_cache_dirs() -> Result<(), String> {
    ensure_all_cache_dirs()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheDirInfo {
    pub kind: String,
    pub dir_name: String,
    pub path: String,
}
#[tauri::command]
pub fn list_cache_dirs() -> Result<Vec<CacheDirInfo>, String> {
    let mut result = Vec::new();
    for kind in CacheResourceKind::all() {
        let path = cache_dir_path_for_kind(*kind)?;
        result.push(CacheDirInfo {
            kind: kind.dir_name().to_string(),
            dir_name: kind.dir_name().to_string(),
            path: path.to_string_lossy().to_string(),
        });
    }
    Ok(result)
}
#[tauri::command]
pub fn list_cached_files(kind: String, mc_version: Option<String>) -> Result<Vec<String>, String> {
    let resource_kind = parse_resource_kind(&kind)?;
    let dir = match &mc_version {
        Some(v) => cache_dir_path_for_version(resource_kind, v)?,
        None => cache_dir_path_for_kind(resource_kind)?,
    };
    let is_world_type = matches!(resource_kind, CacheResourceKind::World);
    let mut files = Vec::new();
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_type = entry.file_type().map_err(|e| e.to_string())?;
                    if file_type.is_file() || (is_world_type && file_type.is_dir()) {
                        if let Some(name) = entry.file_name().to_str() {
                            files.push(name.to_string());
                        }
                    }
                }
            }
            files.sort();
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(Vec::new());
            }
            return Err(e.to_string());
        }
    }
    Ok(files)
}
#[tauri::command]
pub fn get_mod_cache_dir_cmd(mc_version: String, mod_loader: String) -> Result<String, String> {
    let loader = parse_mod_loader(&mod_loader)?;
    Ok(mod_cache_dir_path(&mc_version, &loader)?
        .to_string_lossy()
        .to_string())
}
#[tauri::command]
pub fn list_cached_mods(mc_version: String, mod_loader: String) -> Result<Vec<String>, String> {
    let loader = parse_mod_loader(&mod_loader)?;
    let dir = mod_cache_dir_path(&mc_version, &loader)?;
    let mut files = Vec::new();
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_type = entry.file_type().map_err(|e| e.to_string())?;
                    if file_type.is_file() {
                        if let Some(name) = entry.file_name().to_str() {
                            files.push(name.to_string());
                        }
                    }
                }
            }
            files.sort();
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(Vec::new());
            }
            return Err(e.to_string());
        }
    }
    Ok(files)
}
#[tauri::command]
pub fn cache_to_instance(
    kind: String,
    mc_version: String,
    mod_loader: Option<String>,
    file_name: String,
    instance_dir: String,
    instance_subdir: String,
) -> Result<(), String> {
    let resource_kind = parse_resource_kind(&kind)?;
    let src_dir = if resource_kind == CacheResourceKind::Mod {
        let loader_str = mod_loader.as_ref().map(|s| s.as_str()).unwrap_or("forge");
        let loader = parse_mod_loader(loader_str)?;
        get_mod_cache_dir(&mc_version, loader)?
    } else {
        get_cache_dir_for_version(resource_kind, &mc_version)?
    };
    let src_path = src_dir.join(&file_name);
    if !src_path.exists() {
        return Err(format!("源文件不存在: {}", src_path.display()));
    }
    let dest_base = std::path::PathBuf::from(&instance_dir);
    let dest_dir = dest_base.join(&instance_subdir);
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest_path = dest_dir.join(&file_name);
    if dest_path.exists() {
        return Err(format!("目标文件已存在: {}", dest_path.display()));
    }
    let is_dir = src_path.is_dir();
    if is_dir {
        match std::fs::rename(&src_path, &dest_path) {
            Ok(_) => Ok(()),
            Err(_) => {
                copy_dir_recursive(&src_path, &dest_path).map_err(|e| {
                    format!(
                        "从 {} 复制目录到 {} 失败: {}",
                        src_path.display(),
                        dest_path.display(),
                        e
                    )
                })?;
                std::fs::remove_dir_all(&src_path)
                    .map_err(|e| format!("删除源目录 {} 失败: {}", src_path.display(), e))?;
                Ok(())
            }
        }
    } else {
        match std::fs::rename(&src_path, &dest_path) {
            Ok(_) => Ok(()),
            Err(_) => {
                std::fs::copy(&src_path, &dest_path).map_err(|e2| {
                    format!(
                        "从 {} 复制到 {} 失败: {}",
                        src_path.display(),
                        dest_path.display(),
                        e2
                    )
                })?;
                std::fs::remove_file(&src_path)
                    .map_err(|e2| format!("删除源文件 {} 失败: {}", src_path.display(), e2))?;
                Ok(())
            }
        }
    }
}
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            std::fs::copy(&entry_path, &dest_path)?;
        }
    }
    Ok(())
}
#[tauri::command]
pub fn instance_to_cache(
    kind: String,
    mc_version: String,
    mod_loader: Option<String>,
    file_name: String,
    instance_dir: String,
    instance_subdir: String,
) -> Result<(), String> {
    let resource_kind = parse_resource_kind(&kind)?;
    let src_dir = std::path::PathBuf::from(&instance_dir).join(&instance_subdir);
    let src_path = src_dir.join(&file_name);
    if !src_path.exists() {
        return Err(format!("源文件不存在: {}", src_path.display()));
    }
    let dest_dir = if resource_kind == CacheResourceKind::Mod {
        let loader_str = mod_loader.as_ref().map(|s| s.as_str()).unwrap_or("forge");
        let loader = parse_mod_loader(loader_str)?;
        let dir = get_mod_cache_dir(&mc_version, loader)?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir
    } else {
        let dir = get_cache_dir_for_version(resource_kind, &mc_version)?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir
    };
    let dest_path = dest_dir.join(&file_name);
    if dest_path.exists() {
        return Err(format!("目标文件已存在: {}", dest_path.display()));
    }
    let is_dir = src_path.is_dir();
    if is_dir {
        match std::fs::rename(&src_path, &dest_path) {
            Ok(_) => Ok(()),
            Err(_) => {
                copy_dir_recursive(&src_path, &dest_path).map_err(|e| {
                    format!(
                        "从 {} 复制目录到 {} 失败: {}",
                        src_path.display(),
                        dest_path.display(),
                        e
                    )
                })?;
                std::fs::remove_dir_all(&src_path)
                    .map_err(|e| format!("删除源目录 {} 失败: {}", src_path.display(), e))?;
                Ok(())
            }
        }
    } else {
        match std::fs::rename(&src_path, &dest_path) {
            Ok(_) => Ok(()),
            Err(_) => {
                std::fs::copy(&src_path, &dest_path).map_err(|e2| {
                    format!(
                        "从 {} 复制到 {} 失败: {}",
                        src_path.display(),
                        dest_path.display(),
                        e2
                    )
                })?;
                std::fs::remove_file(&src_path)
                    .map_err(|e2| format!("删除源文件 {} 失败: {}", src_path.display(), e2))?;
                Ok(())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_kind_dir_names() {
        assert_eq!(CacheResourceKind::Mod.dir_name(), "mods");
        assert_eq!(CacheResourceKind::ResourcePack.dir_name(), "resourcepacks");
        assert_eq!(CacheResourceKind::DataPack.dir_name(), "datapacks");
        assert_eq!(CacheResourceKind::World.dir_name(), "worlds");
        assert_eq!(CacheResourceKind::ShaderPack.dir_name(), "shaderpacks");
    }
    #[test]
    fn test_sanitize_version() {
        assert_eq!(sanitize_version("1.12.2"), "1.12.2");
        assert_eq!(sanitize_version("  1.20.4  "), "1.20.4");
        assert_eq!(sanitize_version(""), "unknown");
        assert_eq!(sanitize_version("1.12/forge"), "1.12_forge");
    }

    #[test]
    fn test_listing_missing_cache_does_not_create_directories() {
        let unique_version = format!(
            "read-only-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after the Unix epoch")
                .as_nanos(),
        );
        let resource_dir =
            cache_dir_path_for_version(CacheResourceKind::ResourcePack, &unique_version)
                .expect("resource cache path should resolve");
        let mod_dir = mod_cache_dir_path(&unique_version, &ModLoaderKind::Fabric)
            .expect("mod cache path should resolve");

        assert!(!resource_dir.exists());
        assert!(!mod_dir.exists());
        assert_eq!(
            list_cached_files("resourcepack".to_string(), Some(unique_version.clone()))
                .expect("listing a missing resource cache should succeed"),
            Vec::<String>::new(),
        );
        assert_eq!(
            list_cached_mods(unique_version, "fabric".to_string())
                .expect("listing a missing mod cache should succeed"),
            Vec::<String>::new(),
        );
        assert!(!resource_dir.exists());
        assert!(!mod_dir.exists());
    }

    #[test]
    fn test_cache_root_dir_can_be_created() {
        let dir = cache_root_dir();
        assert!(dir.is_ok());
        assert!(dir.unwrap().exists());
    }
    #[test]
    fn test_mod_loader_dir_names() {
        assert_eq!(ModLoaderKind::Forge.dir_name(), "forge");
        assert_eq!(ModLoaderKind::NeoForge.dir_name(), "neoforge");
        assert_eq!(ModLoaderKind::Fabric.dir_name(), "fabric");
        assert_eq!(ModLoaderKind::Quilt.dir_name(), "quilt");
        assert_eq!(ModLoaderKind::LiteLoader.dir_name(), "liteloader");
        assert_eq!(ModLoaderKind::Ornithe.dir_name(), "ornithe");
        assert_eq!(ModLoaderKind::Vanilla.dir_name(), "vanilla");
        assert_eq!(
            ModLoaderKind::Custom("my_loader".to_string()).dir_name(),
            "my_loader"
        );
    }
    #[test]
    fn test_parse_mod_loader_variants() {
        assert!(matches!(
            parse_mod_loader("forge"),
            Ok(ModLoaderKind::Forge)
        ));
        assert!(matches!(
            parse_mod_loader("NEOFORGE"),
            Ok(ModLoaderKind::NeoForge)
        ));
        assert!(matches!(
            parse_mod_loader("Fabric"),
            Ok(ModLoaderKind::Fabric)
        ));
        assert!(matches!(
            parse_mod_loader("quilt"),
            Ok(ModLoaderKind::Quilt)
        ));
        assert!(matches!(
            parse_mod_loader("liteloader"),
            Ok(ModLoaderKind::LiteLoader)
        ));
        assert!(matches!(
            parse_mod_loader("ornithe"),
            Ok(ModLoaderKind::Ornithe)
        ));
        assert!(matches!(parse_mod_loader(""), Ok(ModLoaderKind::Vanilla)));
        assert!(matches!(
            parse_mod_loader("通用"),
            Ok(ModLoaderKind::Vanilla)
        ));
        assert!(matches!(
            parse_mod_loader("my_custom_loader"),
            Ok(ModLoaderKind::Custom(_))
        ));
        assert!(matches!(
            parse_mod_loader("Rift"),
            Ok(ModLoaderKind::Custom(_))
        ));
    }
    #[test]
    fn test_get_mod_cache_dir_creates_nested_structure() {
        let dir = get_mod_cache_dir("1.12.2", ModLoaderKind::Forge);
        assert!(dir.is_ok());
        let path = dir.unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("mods"));
        assert!(path_str.contains("1.12.2"));
        assert!(path_str.contains("forge"));
        assert!(path.exists());
    }
}