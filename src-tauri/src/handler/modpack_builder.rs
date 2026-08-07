use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use url::Url;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SideRequirement {
    TriState(String),
    Bool(bool),
}
impl SideRequirement {
    pub fn from_tri(s: impl Into<String>) -> Self {
        let v = s.into();
        let normalized = match v.to_ascii_lowercase().as_str() {
            "required" | "必须" => "required",
            "optional" | "可选" => "optional",
            _ => "unsupported",
        }
        .to_string();
        Self::TriState(normalized)
    }
    pub fn from_bool(b: bool) -> Self {
        Self::Bool(b)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthFileEnv {
    #[serde(default = "default_required")]
    pub client: String,
    #[serde(default = "default_required")]
    pub server: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthFileEntry {
    pub path: String,
    pub hashes: ModrinthHashes,
    #[serde(default = "default_env")]
    pub env: ModrinthFileEnv,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing)]
    pub client: Option<String>,
    #[serde(default, skip_serializing)]
    pub server: Option<String>,
}
fn default_env() -> ModrinthFileEnv {
    ModrinthFileEnv {
        client: "required".to_string(),
        server: "required".to_string(),
    }
}
fn default_required() -> String {
    "required".to_string()
}
fn default_format_version() -> i32 {
    1
}
fn default_game() -> String {
    "minecraft".to_string()
}
fn default_pack_version() -> String {
    "1.0.0".to_string()
}
fn normalize_modrinth_file_entry(entry: &mut ModrinthFileEntry) {
    if entry.client.is_some() || entry.server.is_some() {
        let migrated_client = entry.client.clone().unwrap_or_else(default_required);
        let migrated_server = entry.server.clone().unwrap_or_else(default_required);
        entry.env = ModrinthFileEnv {
            client: migrated_client,
            server: migrated_server,
        };
        entry.client = None;
        entry.server = None;
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthHashes {
    pub sha1: String,
    pub sha512: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModrinthDependencies {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "fabric-loader"
    )]
    pub fabric_loader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "neoforge-loader",
        alias = "neo-forge"
    )]
    pub neoforge: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "quilt-loader"
    )]
    pub quilt_loader: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurseforgeFileEntry {
    #[serde(rename = "projectID")]
    pub project_id: i64,
    #[serde(rename = "fileID")]
    pub file_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

fn default_true() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "format")]
pub enum ModpackInstance {
    #[serde(rename = "modrinth")]
    Modrinth {
        #[serde(rename = "formatVersion")]
        #[serde(default = "default_format_version")]
        format_version: i32,
        #[serde(default = "default_game")]
        game: String,
        #[serde(rename = "versionId")]
        #[serde(default)]
        version_id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
        #[serde(default)]
        files: Vec<ModrinthFileEntry>,
        #[serde(default)]
        dependencies: ModrinthDependencies,
        #[serde(default)]
        created_at: i64,
        #[serde(default)]
        updated_at: i64,
        #[serde(default)]
        loader: String,
        #[serde(default)]
        loader_version: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author: Option<String>,
        #[serde(default)]
        optifine: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        optifine_version: Option<String>,
        #[serde(default)]
        cross_loader: bool,
        #[serde(default, skip_serializing)]
        game_version: Option<String>,
    },
    #[serde(rename = "curseforge")]
    Curseforge {
        name: String,
        #[serde(default = "default_pack_version")]
        version: String,
        #[serde(default)]
        author: String,
        created_at: i64,
        updated_at: i64,
        game_version: String,
        #[serde(default)]
        loader: String,
        #[serde(default)]
        loader_version: String,
        #[serde(default)]
        optifine: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        optifine_version: Option<String>,
        #[serde(default)]
        cross_loader: bool,
        files: Vec<CurseforgeFileEntry>,
    },
}
impl ModpackInstance {
    pub fn name(&self) -> &str {
        match self {
            Self::Modrinth { name, .. } => name,
            Self::Curseforge { name, .. } => name,
        }
    }
    pub fn format_tag(&self) -> &'static str {
        match self {
            Self::Modrinth { .. } => "modrinth",
            Self::Curseforge { .. } => "curseforge",
        }
    }
    pub fn file_count(&self) -> usize {
        match self {
            Self::Modrinth { files, .. } => files.len(),
            Self::Curseforge { files, .. } => files.len(),
        }
    }
    pub fn game_version(&self) -> String {
        match self {
            Self::Modrinth {
                version_id,
                dependencies,
                game_version,
                ..
            } => {
                dependencies
                    .minecraft
                    .clone()
                    .or_else(|| game_version.clone())
                    // 兼容旧工程：旧版曾把 MC 版本写入 versionId。
                    .unwrap_or_else(|| version_id.clone())
            }
            Self::Curseforge { game_version, .. } => game_version.clone(),
        }
    }
    pub fn loader(&self) -> &str {
        match self {
            Self::Modrinth { loader, .. } => loader,
            Self::Curseforge { loader, .. } => loader,
        }
    }
    pub fn loader_version(&self) -> &str {
        match self {
            Self::Modrinth { loader_version, .. } => loader_version,
            Self::Curseforge { loader_version, .. } => loader_version,
        }
    }
    pub fn pack_version(&self) -> &str {
        match self {
            Self::Modrinth { version_id, .. } => version_id,
            Self::Curseforge { version, .. } => version,
        }
    }
    pub fn optifine(&self) -> (bool, Option<&str>) {
        match self {
            Self::Modrinth {
                optifine,
                optifine_version,
                ..
            } => (*optifine, optifine_version.as_deref()),
            Self::Curseforge {
                optifine,
                optifine_version,
                ..
            } => (*optifine, optifine_version.as_deref()),
        }
    }
    pub fn cross_loader(&self) -> bool {
        match self {
            Self::Modrinth { cross_loader, .. } => *cross_loader,
            Self::Curseforge { cross_loader, .. } => *cross_loader,
        }
    }
    pub fn updated_at(&self) -> i64 {
        match self {
            Self::Modrinth { updated_at, .. } => *updated_at,
            Self::Curseforge { updated_at, .. } => *updated_at,
        }
    }
    pub fn normalize(&mut self) {
        if let Self::Modrinth {
            version_id,
            dependencies,
            game_version,
            files,
            loader,
            loader_version,
            ..
        } = self
        {
            if version_id.is_empty() {
                *version_id = default_pack_version();
            }
            if dependencies.minecraft.is_none() {
                dependencies.minecraft = game_version.clone();
            }
            for f in files.iter_mut() {
                normalize_modrinth_file_entry(f);
            }
            if loader_version.is_empty() {
                *loader_version = match loader.as_str() {
                    "forge" => dependencies.forge.clone(),
                    "neoforge" => dependencies.neoforge.clone(),
                    "fabric" => dependencies.fabric_loader.clone(),
                    "quilt" => dependencies.quilt_loader.clone(),
                    _ => None,
                }
                .unwrap_or_default();
            }
        }
    }
    fn created_at(&self) -> i64 {
        match self {
            Self::Modrinth { created_at, .. } => *created_at,
            Self::Curseforge { created_at, .. } => *created_at,
        }
    }
    fn set_created_at(&mut self, value: i64) {
        match self {
            Self::Modrinth { created_at, .. } => *created_at = value,
            Self::Curseforge { created_at, .. } => *created_at = value,
        }
    }
    fn touch(&mut self) {
        let now = now_secs();
        match self {
            Self::Modrinth {
                updated_at,
                created_at,
                ..
            } => {
                if *created_at == 0 {
                    *created_at = now;
                }
                *updated_at = now;
            }
            Self::Curseforge {
                updated_at,
                created_at,
                ..
            } => {
                if *created_at == 0 {
                    *created_at = now;
                }
                *updated_at = now;
            }
        }
    }
}
fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
fn modpack_root_dir_from_config(minecraft_path: &str) -> PathBuf {
    let base = if minecraft_path.is_empty() {
        default_minecraft_path()
    } else {
        minecraft_path.to_string()
    };
    PathBuf::from(&base).join("modpack")
}
fn default_minecraft_path() -> String {
    #[cfg(target_os = "windows")]
    {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        exe_dir.join("minecraft").to_string_lossy().to_string()
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/Library/Application Support/RTLauncher/version", home)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "./minecraft".to_string()
    }
}
fn instance_file_path(root: &Path, name: &str) -> PathBuf {
    let safe = sanitize_filename::basic(name);
    root.join(format!("{}.json", safe))
}
mod sanitize_filename {
    pub fn basic(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for ch in name.chars() {
            if matches!(
                ch,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' | '\n' | '\r' | '\t'
            ) {
                out.push('_');
            } else {
                out.push(ch);
            }
        }
        let trimmed = out.trim().trim_matches('.');
        if trimmed.is_empty() {
            "unnamed".to_string()
        } else {
            trimmed.to_string()
        }
    }
}
fn ensure_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {}", e))
}
fn replace_file(tmp: &Path, target: &Path) -> Result<(), String> {
    if !target.exists() {
        return fs::rename(tmp, target).map_err(|e| format!("重命名文件失败: {}", e));
    }
    let backup = target.with_extension(format!(
        "{}.bak",
        target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
    ));
    if backup.exists() {
        fs::remove_file(&backup).map_err(|e| format!("清理旧备份失败: {}", e))?;
    }
    fs::rename(target, &backup).map_err(|e| format!("备份旧文件失败: {}", e))?;
    if let Err(error) = fs::rename(tmp, target) {
        let _ = fs::rename(&backup, target);
        return Err(format!("替换文件失败: {}", error));
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path
            .split('/')
            .next()
            .is_some_and(|part| part.contains(':'))
    {
        return Err(format!("文件目标路径不安全: {}", path));
    }
    Ok(())
}

fn validate_hash(value: &str, expected_len: usize, name: &str) -> Result<(), String> {
    if value.len() != expected_len || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{} 哈希格式错误", name));
    }
    Ok(())
}

fn validate_modrinth_download(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("下载地址格式错误: {}", e))?;
    if parsed.scheme() != "https" {
        return Err("Modrinth 文件下载地址必须使用 HTTPS".to_string());
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    const ALLOWED: &[&str] = &[
        "cdn.modrinth.com",
        "github.com",
        "raw.githubusercontent.com",
        "gitlab.com",
    ];
    if !ALLOWED.contains(&host.as_str()) {
        return Err(format!("Modrinth 文件下载域名不在允许列表中: {}", host));
    }
    Ok(())
}

fn validate_loader_version(loader: &str, version: &str) -> Result<(), String> {
    if loader.trim().is_empty() {
        return Err("请选择模组加载器".to_string());
    }
    if version.trim().is_empty() || version.eq_ignore_ascii_case("latest") {
        return Err(format!("{} 必须使用具体加载器版本", loader));
    }
    Ok(())
}

fn validate_project(instance: &ModpackInstance, for_export: bool) -> Result<(), String> {
    if instance.name().trim().is_empty() {
        return Err("整合包名称不能为空".to_string());
    }
    if instance.pack_version().trim().is_empty() {
        return Err("整合包版本不能为空".to_string());
    }
    validate_loader_version(instance.loader(), instance.loader_version())?;

    if for_export && instance.cross_loader() {
        return Err("标准整合包只能声明一个加载器".to_string());
    }
    if for_export && instance.optifine().0 {
        return Err("OptiFine 文件尚未加入标准清单".to_string());
    }

    match instance {
        ModpackInstance::Modrinth {
            format_version,
            game,
            version_id,
            files,
            dependencies,
            loader,
            ..
        } => {
            if *format_version != 1 || game != "minecraft" {
                return Err("Modrinth 工程的 formatVersion/game 不符合 mrpack v1".to_string());
            }
            if version_id.trim().is_empty() {
                return Err("Modrinth versionId 不能为空".to_string());
            }
            if dependencies
                .minecraft
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Err("Modrinth dependencies.minecraft 不能为空".to_string());
            }
            let declared_loader_version = match loader.as_str() {
                "forge" => dependencies.forge.as_deref(),
                "neoforge" => dependencies.neoforge.as_deref(),
                "fabric" => dependencies.fabric_loader.as_deref(),
                "quilt" => dependencies.quilt_loader.as_deref(),
                _ => None,
            }
            .unwrap_or_default();
            if declared_loader_version != instance.loader_version() {
                return Err("加载器版本与 Modrinth dependencies 不一致".to_string());
            }
            if for_export && files.is_empty() {
                return Err("整合包至少需要一个文件".to_string());
            }
            let mut paths = HashSet::new();
            for file in files {
                validate_relative_path(&file.path)?;
                if !paths.insert(file.path.to_ascii_lowercase()) {
                    return Err(format!("存在重复目标路径: {}", file.path));
                }
                validate_hash(&file.hashes.sha1, 40, "SHA-1")?;
                validate_hash(&file.hashes.sha512, 128, "SHA-512")?;
                if file.file_size == 0 {
                    return Err(format!("文件大小必须大于零: {}", file.path));
                }
                if file.downloads.is_empty() {
                    return Err(format!("文件缺少下载地址: {}", file.path));
                }
                for download in &file.downloads {
                    validate_modrinth_download(download)?;
                }
                for side in [&file.env.client, &file.env.server] {
                    if !matches!(side.as_str(), "required" | "optional" | "unsupported") {
                        return Err(format!("文件环境标记错误: {}", file.path));
                    }
                }
            }
        }
        ModpackInstance::Curseforge { author, files, .. } => {
            if author.trim().is_empty() {
                return Err("CurseForge 整合包作者不能为空".to_string());
            }
            if for_export && files.is_empty() {
                return Err("整合包至少需要一个文件".to_string());
            }
            let mut projects = HashSet::new();
            for file in files {
                if file.project_id <= 0 || file.file_id <= 0 {
                    return Err("CurseForge projectID/fileID 必须大于零".to_string());
                }
                if !projects.insert(file.project_id) {
                    return Err(format!("CurseForge 项目重复: {}", file.project_id));
                }
            }
        }
    }
    Ok(())
}

fn curseforge_loader_id(loader: &str, version: &str) -> Result<String, String> {
    validate_loader_version(loader, version)?;
    let prefix = format!("{}-", loader.to_ascii_lowercase());
    if version.to_ascii_lowercase().starts_with(&prefix) {
        Ok(version.to_string())
    } else {
        Ok(format!("{}-{}", loader.to_ascii_lowercase(), version))
    }
}

fn normalized_export_path(output_path: &str, extension: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::from(output_path.trim());
    if path.as_os_str().is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map_or(true, |value| !value.eq_ignore_ascii_case(extension))
    {
        path.set_extension(extension);
    }
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    Ok(path)
}

fn write_json_zip(
    output: &Path,
    metadata_name: &str,
    metadata: &serde_json::Value,
    add_overrides_dir: bool,
) -> Result<(), String> {
    let tmp = output.with_extension(format!(
        "{}.tmp",
        output
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("zip")
    ));
    let file = fs::File::create(&tmp).map_err(|e| format!("创建导出文件失败: {}", e))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    zip.start_file(metadata_name, options)
        .map_err(|e| format!("创建 ZIP 元数据失败: {}", e))?;
    let json = serde_json::to_vec_pretty(metadata).map_err(|e| e.to_string())?;
    zip.write_all(&json)
        .map_err(|e| format!("写入 ZIP 元数据失败: {}", e))?;
    if add_overrides_dir {
        zip.add_directory(
            "overrides/",
            FileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .unix_permissions(0o755),
        )
        .map_err(|e| format!("创建 overrides 目录失败: {}", e))?;
    }
    zip.finish().map_err(|e| format!("完成 ZIP 失败: {}", e))?;
    replace_file(&tmp, output)?;
    Ok(())
}

fn modrinth_export_json(instance: &ModpackInstance) -> Result<serde_json::Value, String> {
    let ModpackInstance::Modrinth {
        format_version,
        game,
        version_id,
        name,
        summary,
        files,
        dependencies,
        ..
    } = instance
    else {
        return Err("工程类型不是 Modrinth".to_string());
    };
    let clean_files: Vec<serde_json::Value> = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "hashes": file.hashes,
                "env": file.env,
                "downloads": file.downloads,
                "fileSize": file.file_size,
            })
        })
        .collect();
    let mut index = serde_json::json!({
        "formatVersion": format_version,
        "game": game,
        "versionId": version_id,
        "name": name,
        "files": clean_files,
        "dependencies": dependencies,
    });
    if let Some(summary) = summary.as_ref().filter(|value| !value.trim().is_empty()) {
        index["summary"] = serde_json::Value::String(summary.clone());
    }
    Ok(index)
}

fn curseforge_export_json(instance: &ModpackInstance) -> Result<serde_json::Value, String> {
    let ModpackInstance::Curseforge {
        name,
        version,
        author,
        game_version,
        loader,
        loader_version,
        files,
        ..
    } = instance
    else {
        return Err("工程类型不是 CurseForge".to_string());
    };
    let loader_id = curseforge_loader_id(loader, loader_version)?;
    let manifest_files: Vec<serde_json::Value> = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "projectID": file.project_id,
                "fileID": file.file_id,
                "required": file.required,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "minecraft": {
            "version": game_version,
            "modLoaders": [{
                "id": loader_id,
                "primary": true,
            }],
        },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": name,
        "version": version,
        "author": author,
        "files": manifest_files,
        "overrides": "overrides",
    }))
}
#[tauri::command]
pub fn get_modpack_dir(minecraft_path: Option<String>) -> Result<String, String> {
    let dir = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    ensure_dir(&dir)?;
    Ok(dir.to_string_lossy().to_string())
}
#[tauri::command]
pub fn save_modpack_instance(
    app: tauri::AppHandle,
    mut instance: ModpackInstance,
    minecraft_path: Option<String>,
) -> Result<(), String> {
    let name = instance.name().trim().to_string();
    if name.is_empty() {
        return Err("整合包名称不能为空".to_string());
    }
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    ensure_dir(&root)?;
    let file = instance_file_path(&root, &name);
    instance.normalize();
    if instance.created_at() == 0 && file.exists() {
        if let Ok(text) = fs::read_to_string(&file) {
            if let Ok(existing) = serde_json::from_str::<ModpackInstance>(&text) {
                if existing.created_at() > 0 {
                    instance.set_created_at(existing.created_at());
                }
            }
        }
    }
    validate_project(&instance, false)?;
    instance.touch();
    let tmp = file.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&instance).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| format!("写入文件失败: {}", e))?;
    replace_file(&tmp, &file)?;
    let _ = app.emit("modpack-instance-updated", &name);
    Ok(())
}

#[tauri::command]
pub fn export_modpack_instance(
    name: String,
    output_path: String,
    minecraft_path: Option<String>,
) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    let project_path = instance_file_path(&root, trimmed);
    let text =
        fs::read_to_string(&project_path).map_err(|e| format!("读取整合包工程失败: {}", e))?;
    let mut instance: ModpackInstance =
        serde_json::from_str(&text).map_err(|e| format!("解析整合包工程失败: {}", e))?;
    instance.normalize();
    validate_project(&instance, true)?;

    let output = match &instance {
        ModpackInstance::Modrinth { .. } => {
            let output = normalized_export_path(&output_path, "mrpack")?;
            let index = modrinth_export_json(&instance)?;
            write_json_zip(&output, "modrinth.index.json", &index, false)?;
            output
        }
        ModpackInstance::Curseforge { .. } => {
            let output = normalized_export_path(&output_path, "zip")?;
            let manifest = curseforge_export_json(&instance)?;
            write_json_zip(&output, "manifest.json", &manifest, true)?;
            output
        }
    };

    Ok(output.to_string_lossy().to_string())
}
#[tauri::command]
pub fn list_modpack_instances(minecraft_path: Option<String>) -> Result<Vec<ListEntry>, String> {
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&root).map_err(|e| e.to_string())?;
    for item in read_dir.flatten() {
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let parsed: serde_json::Result<ModpackInstance> = serde_json::from_str(&text);
        match parsed {
            Ok(mut inst) => {
                inst.normalize();
                entries.push(ListEntry {
                    name: inst.name().to_string(),
                    format: inst.format_tag().to_string(),
                    file_count: inst.file_count(),
                    updated_at: inst.updated_at(),
                    game_version: inst.game_version(),
                    loader: inst.loader().to_string(),
                    optifine: inst.optifine().0,
                    cross_loader: inst.cross_loader(),
                })
            }
            Err(_) => continue,
        }
    }
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(entries)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEntry {
    pub name: String,
    pub format: String,
    pub file_count: usize,
    pub updated_at: i64,
    pub game_version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub optifine: bool,
    #[serde(default)]
    pub cross_loader: bool,
}
#[tauri::command]
pub fn load_modpack_instance(
    name: String,
    minecraft_path: Option<String>,
) -> Result<ModpackInstance, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    let file = instance_file_path(&root, trimmed);
    if !file.exists() {
        return Err(format!("整合包不存在: {}", trimmed));
    }
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let mut inst: ModpackInstance = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    inst.normalize();
    Ok(inst)
}
#[tauri::command]
pub fn delete_modpack_instance(name: String, minecraft_path: Option<String>) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    let file = instance_file_path(&root, trimmed);
    if !file.exists() {
        return Err(format!("整合包不存在: {}", trimmed));
    }
    fs::remove_file(&file).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn rename_modpack_instance(
    old_name: String,
    new_name: String,
    minecraft_path: Option<String>,
) -> Result<(), String> {
    let old_trim = old_name.trim();
    let new_trim = new_name.trim();
    if old_trim.is_empty() || new_trim.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let root = modpack_root_dir_from_config(&minecraft_path.unwrap_or_default());
    let old_path = instance_file_path(&root, old_trim);
    let new_path = instance_file_path(&root, new_trim);
    if !old_path.exists() {
        return Err(format!("原整合包不存在: {}", old_trim));
    }
    if new_path.exists() && new_path != old_path {
        return Err(format!("同名整合包已存在: {}", new_trim));
    }
    let text = fs::read_to_string(&old_path).map_err(|e| e.to_string())?;
    let mut inst: ModpackInstance = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    match &mut inst {
        ModpackInstance::Modrinth { name, .. } => *name = new_trim.to_string(),
        ModpackInstance::Curseforge { name, .. } => *name = new_trim.to_string(),
    }
    inst.touch();
    let text = serde_json::to_string_pretty(&inst).map_err(|e| e.to_string())?;
    let tmp = new_path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    replace_file(&tmp, &new_path)?;
    if new_path != old_path {
        let _ = fs::remove_file(&old_path);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn modrinth_project() -> ModpackInstance {
        ModpackInstance::Modrinth {
            format_version: 1,
            game: "minecraft".to_string(),
            version_id: "1.0.0".to_string(),
            name: "Test Pack".to_string(),
            summary: Some("Summary".to_string()),
            files: vec![ModrinthFileEntry {
                path: "mods/example.jar".to_string(),
                hashes: ModrinthHashes {
                    sha1: "a".repeat(40),
                    sha512: "b".repeat(128),
                    sha256: None,
                },
                env: default_env(),
                downloads: vec!["https://cdn.modrinth.com/data/test/example.jar".to_string()],
                file_size: 42,
                display_name: Some("UI only".to_string()),
                client: None,
                server: None,
            }],
            dependencies: ModrinthDependencies {
                minecraft: Some("1.21.1".to_string()),
                neoforge: Some("21.1.200".to_string()),
                ..Default::default()
            },
            created_at: 1,
            updated_at: 1,
            loader: "neoforge".to_string(),
            loader_version: "21.1.200".to_string(),
            author: Some("Author".to_string()),
            optifine: false,
            optifine_version: None,
            cross_loader: false,
            game_version: None,
        }
    }

    fn curseforge_project() -> ModpackInstance {
        ModpackInstance::Curseforge {
            name: "Test Pack".to_string(),
            version: "2.0.0".to_string(),
            author: "Author".to_string(),
            created_at: 1,
            updated_at: 1,
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            loader_version: "0.16.10".to_string(),
            optifine: false,
            optifine_version: None,
            cross_loader: false,
            files: vec![CurseforgeFileEntry {
                project_id: 123,
                file_id: 456,
                display_name: Some("UI only".to_string()),
                required: true,
                category: Some("mod".to_string()),
            }],
        }
    }

    #[test]
    fn modrinth_export_contains_only_standard_fields() {
        let project = modrinth_project();
        validate_project(&project, true).unwrap();
        let json = modrinth_export_json(&project).unwrap();
        assert_eq!(json["versionId"], "1.0.0");
        assert_eq!(json["dependencies"]["minecraft"], "1.21.1");
        assert_eq!(json["dependencies"]["neoforge"], "21.1.200");
        assert!(json["dependencies"].get("neoforge-loader").is_none());
        assert!(json.get("format").is_none());
        assert!(json.get("loader").is_none());
        assert!(json["files"][0].get("display_name").is_none());
    }

    #[test]
    fn curseforge_export_matches_manifest_shape() {
        let project = curseforge_project();
        validate_project(&project, true).unwrap();
        let json = curseforge_export_json(&project).unwrap();
        assert_eq!(json["manifestType"], "minecraftModpack");
        assert_eq!(json["manifestVersion"], 1);
        assert_eq!(json["minecraft"]["version"], "1.20.1");
        assert_eq!(json["minecraft"]["modLoaders"][0]["id"], "fabric-0.16.10");
        assert_eq!(json["files"][0]["projectID"], 123);
        assert!(json.get("format").is_none());
        assert!(json["files"][0].get("display_name").is_none());
    }

    #[test]
    fn validation_rejects_latest_and_unsafe_paths() {
        let mut project = modrinth_project();
        if let ModpackInstance::Modrinth {
            loader_version,
            dependencies,
            ..
        } = &mut project
        {
            *loader_version = "latest".to_string();
            dependencies.neoforge = Some("latest".to_string());
        }
        assert!(validate_project(&project, true).is_err());

        let mut project = modrinth_project();
        if let ModpackInstance::Modrinth { files, .. } = &mut project {
            files[0].path = "../outside.jar".to_string();
        }
        assert!(validate_project(&project, true).is_err());
    }

    #[test]
    fn datapacks_are_valid_pack_resources() {
        let mut modrinth = modrinth_project();
        if let ModpackInstance::Modrinth { files, .. } = &mut modrinth {
            files[0].path = "datapacks/example.zip".to_string();
        }
        validate_project(&modrinth, true).unwrap();

        let mut curseforge = curseforge_project();
        if let ModpackInstance::Curseforge { files, .. } = &mut curseforge {
            files[0].category = Some("datapack".to_string());
        }
        validate_project(&curseforge, true).unwrap();
    }

    #[test]
    fn modpacks_keep_the_original_file_entry_logic() {
        let mut modrinth = modrinth_project();
        if let ModpackInstance::Modrinth { files, .. } = &mut modrinth {
            files[0].path = "example.mrpack".to_string();
            files[0].downloads =
                vec!["https://cdn.modrinth.com/data/test/example.mrpack".to_string()];
        }
        validate_project(&modrinth, true).unwrap();
        let modrinth_json = modrinth_export_json(&modrinth).unwrap();
        assert_eq!(modrinth_json["files"][0]["path"], "example.mrpack");

        let mut curseforge = curseforge_project();
        if let ModpackInstance::Curseforge { files, .. } = &mut curseforge {
            files[0].category = Some("modpack".to_string());
        }
        validate_project(&curseforge, true).unwrap();
        let curseforge_json = curseforge_export_json(&curseforge).unwrap();
        assert_eq!(curseforge_json["files"][0]["projectID"], 123);
        assert_eq!(curseforge_json["files"][0]["fileID"], 456);
    }

    #[test]
    fn zip_writer_places_metadata_at_archive_root() {
        let unique = format!(
            "rtlauncher-modpack-test-{}-{}.mrpack",
            std::process::id(),
            now_secs()
        );
        let output = std::env::temp_dir().join(unique);
        let metadata = modrinth_export_json(&modrinth_project()).unwrap();
        write_json_zip(&output, "modrinth.index.json", &metadata, false).unwrap();

        {
            let file = fs::File::open(&output).unwrap();
            let mut archive = zip::ZipArchive::new(file).unwrap();
            assert!(archive.by_name("modrinth.index.json").is_ok());
        }
        let _ = fs::remove_file(output);
    }
}
