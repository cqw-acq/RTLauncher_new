use crate::downloader::concurrent_download::{self, DownloadTask};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

fn detect_file_category(file_path: &Path) -> &'static str {
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension == "jar" {
        return "mods";
    }

    if extension == "zip" {
        if let Ok(file) = std::fs::File::open(file_path) {
            let reader = BufReader::new(file);
            if let Ok(mut archive) = ZipArchive::new(reader) {
                let mut has_data = false;
                let mut has_pack_mcmeta = false;

                for i in 0..archive.len() {
                    if let Ok(entry) = archive.by_index(i) {
                        let name = entry.name();
                        let clean_name = name.trim_end_matches('/');
                        let depth = clean_name.matches('/').count();
                        if depth == 0 {
                            if clean_name == "data" && name.ends_with('/') {
                                has_data = true;
                            }
                            if clean_name == "pack.mcmeta" && !name.ends_with('/') {
                                has_pack_mcmeta = true;
                            }
                        }
                    }
                }

                return match (has_data, has_pack_mcmeta) {
                    (true, true) => "datapacks",
                    (false, true) => "resourcepacks",
                    _ => "shaderpacks",
                };
            }
        }
    }

    "mods"
}

fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        if let Err(e) = fs::create_dir_all(path) {
            eprintln!("[Modpack] 创建目录失败 {}: {}", path.display(), e);
            return Err(anyhow!("创建目录失败: {}", e));
        }
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthIndexEnv {
    #[serde(default = "default_required_side")]
    pub client: String,
    #[serde(default = "default_required_side")]
    pub server: String,
}
fn default_required_side() -> String {
    "required".to_string()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthIndexFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<ModrinthIndexEnv>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub fileSize: Option<u64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModrinthIndexDependencies {
    #[serde(default)]
    pub minecraft: Option<String>,
    #[serde(default, rename = "fabric-loader")]
    pub fabric_loader: Option<String>,
    #[serde(default)]
    pub forge: Option<String>,
    #[serde(default, rename = "neoforge-loader")]
    pub neoforge_loader: Option<String>,
    #[serde(default)]
    pub neoforge: Option<String>,
    #[serde(default, rename = "neo-forge")]
    pub neo_forge: Option<String>,
    #[serde(default, rename = "quilt-loader")]
    pub quilt_loader: Option<String>,
    #[serde(default)]
    pub liteloader: Option<String>,
    #[serde(default)]
    pub optifine: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthIndex {
    #[serde(default)]
    pub formatVersion: i32,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub versionId: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<ModrinthIndexFile>,
    #[serde(default)]
    pub dependencies: ModrinthIndexDependencies,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeManifestFile {
    #[serde(default)]
    pub projectID: i64,
    #[serde(default)]
    pub fileID: i64,
    #[serde(default)]
    pub required: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CurseForgeManifestMinecraft {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub modLoaders: Vec<CurseForgeModLoader>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CurseForgeModLoader {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeManifest {
    #[serde(default)]
    pub minecraft: CurseForgeManifestMinecraft,
    #[serde(default)]
    pub manifestType: String,
    #[serde(default)]
    pub manifestVersion: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub files: Vec<CurseForgeManifestFile>,
    #[serde(default)]
    pub overrides: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackExternalFile {
    pub relative_path: String,
    pub urls: Vec<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub project_id: Option<u64>,
    pub file_id: Option<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModpackLoaderType {
    Vanilla,
    Forge,
    Neoforge,
    Fabric,
    Quilt,
    LiteLoader,
    Optifine,
}
#[derive(Debug, Clone)]
pub struct ParsedModpack {
    pub name: String,
    pub mc_version: String,
    pub loader_type: ModpackLoaderType,
    pub loader_version: Option<String>,
    pub external_files: Vec<ModpackExternalFile>,
    pub extracted_files: Vec<(PathBuf, Vec<u8>)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModpackFormat {
    Modrinth,
    CurseForge,
    Unknown,
}
pub fn detect_modpack_format(path: &Path) -> ModpackFormat {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ModpackFormat::Unknown,
    };
    let mut zip = match ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => return ModpackFormat::Unknown,
    };
    for i in 0..zip.len() {
        if let Ok(entry) = zip.by_index(i) {
            let name = entry.name();
            if name == "modrinth.index.json" {
                return ModpackFormat::Modrinth;
            }
            if name == "manifest.json" {
                return ModpackFormat::CurseForge;
            }
        }
    }
    ModpackFormat::Unknown
}
pub fn parse_modpack_from_zip(path: &Path) -> Result<ParsedModpack> {
    let format = detect_modpack_format(path);
    match format {
        ModpackFormat::Modrinth => parse_modrinth_zip(path),
        ModpackFormat::CurseForge => parse_curseforge_zip(path),
        ModpackFormat::Unknown => bail!("无法识别的整合包格式"),
    }
}
fn parse_modrinth_zip(path: &Path) -> Result<ParsedModpack> {
    let file = fs::File::open(path).with_context(|| format!("打开文件失败: {:?}", path))?;
    let mut zip = ZipArchive::new(file).context("解析 ZIP 失败")?;
    let mut index_content = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).context("读取 ZIP 条目失败")?;
        if entry.name() == "modrinth.index.json" {
            use std::io::Read;
            entry
                .read_to_string(&mut index_content)
                .context("读取 index 失败")?;
            break;
        }
    }
    if index_content.is_empty() {
        bail!("未找到 modrinth.index.json");
    }
    let index: ModrinthIndex =
        serde_json::from_str(&index_content).context("解析 modrinth.index.json 失败")?;
    let mc_version = index
        .dependencies
        .minecraft
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| index.versionId.clone());
    if mc_version.is_empty() {
        bail!("无法确定整合包的 Minecraft 版本");
    }
    let (loader_type, loader_version) = detect_modrinth_loader(&index.dependencies, &mc_version);
    let mut external_files = Vec::new();
    for f in &index.files {
        let client_side = f
            .env
            .as_ref()
            .map(|e| e.client.as_str())
            .unwrap_or("required");
        if client_side == "unsupported" {
            continue;
        }
        if f.downloads.is_empty() {
            continue;
        }
        let path_raw = f.path.trim();
        if path_raw.is_empty()
            || path_raw.starts_with('/')
            || path_raw.starts_with('\\')
            || path_raw.contains("..")
        {
            continue;
        }
        let sha1 = f.hashes.get("sha1").cloned();
        external_files.push(ModpackExternalFile {
            relative_path: f.path.clone(),
            urls: f.downloads.clone(),
            sha1,
            size: f.fileSize,
            project_id: None,
            file_id: None,
        });
    }
    let mut extracted_files = Vec::new();
    for i in 0..zip.len() {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        if name == "modrinth.index.json" {
            continue;
        }
        if name.ends_with('/') {
            continue;
        }
        let relative = if let Some(rest) = name.strip_prefix("overrides/") {
            rest.to_string()
        } else if let Some(rest) = name.strip_prefix("client-overrides/") {
            rest.to_string()
        } else {
            continue;
        };
        if relative.is_empty()
            || relative.contains("..")
            || relative.starts_with('/')
            || relative.starts_with('\\')
        {
            continue;
        }
        let mut buf = Vec::new();
        use std::io::Read;
        if entry.read_to_end(&mut buf).is_err() {
            continue;
        }
        extracted_files.push((PathBuf::from(relative), buf));
    }
    Ok(ParsedModpack {
        name: if !index.name.is_empty() {
            index.name
        } else {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "modpack".to_string())
        },
        mc_version,
        loader_type,
        loader_version,
        external_files,
        extracted_files,
    })
}
fn detect_modrinth_loader(
    deps: &ModrinthIndexDependencies,
    mc_version: &str,
) -> (ModpackLoaderType, Option<String>) {
    if let Some(v) = &deps.forge {
        if !v.is_empty() {
            let normalized = if v.contains('-') {
                v.clone()
            } else {
                format!("{}-{}", mc_version, v)
            };
            return (ModpackLoaderType::Forge, Some(normalized));
        }
    }
    let neoforge_ver = deps
        .neoforge_loader
        .as_ref()
        .or(deps.neoforge.as_ref())
        .or(deps.neo_forge.as_ref())
        .filter(|v| !v.is_empty())
        .cloned();
    if let Some(v) = neoforge_ver {
        let normalized = if v.contains('-') && v.starts_with(mc_version) {
            v
        } else {
            format!("{}-{}", mc_version, v)
        };
        return (ModpackLoaderType::Neoforge, Some(normalized));
    }
    if let Some(v) = &deps.fabric_loader {
        if !v.is_empty() {
            return (ModpackLoaderType::Fabric, Some(v.clone()));
        }
    }
    if let Some(v) = &deps.quilt_loader {
        if !v.is_empty() {
            return (ModpackLoaderType::Quilt, Some(v.clone()));
        }
    }
    if let Some(v) = &deps.liteloader {
        if !v.is_empty() {
            return (ModpackLoaderType::LiteLoader, Some(v.clone()));
        }
    }
    if let Some(v) = &deps.optifine {
        if !v.is_empty() {
            return (ModpackLoaderType::Optifine, Some(v.clone()));
        }
    }
    (ModpackLoaderType::Vanilla, None)
}
fn parse_curseforge_zip(path: &Path) -> Result<ParsedModpack> {
    let file = fs::File::open(path).with_context(|| format!("打开文件失败: {:?}", path))?;
    let mut zip = ZipArchive::new(file).context("解析 ZIP 失败")?;
    let mut manifest_content = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).context("读取 ZIP 条目失败")?;
        if entry.name() == "manifest.json" {
            use std::io::Read;
            entry
                .read_to_string(&mut manifest_content)
                .context("读取 manifest 失败")?;
            break;
        }
    }
    if manifest_content.is_empty() {
        bail!("未找到 manifest.json");
    }
    let manifest: CurseForgeManifest =
        serde_json::from_str(&manifest_content).context("解析 manifest.json 失败")?;
    let mc_version = if !manifest.minecraft.version.is_empty() {
        manifest.minecraft.version.clone()
    } else {
        bail!("无法确定 CurseForge 整合包的 Minecraft 版本");
    };
    let (loader_type, loader_version) =
        parse_curseforge_loaders(&manifest.minecraft.modLoaders, &mc_version);
    let mut external_files = Vec::new();
    for cf in &manifest.files {
        if !cf.required {
            continue;
        }
        if cf.fileID <= 0 || cf.projectID <= 0 {
            continue;
        }
        let first4 = cf.fileID / 1000;
        let last3 = cf.fileID % 1000;
        let pid = cf.projectID as u64;
        let fid = cf.fileID as u64;
        let cdn_url = format!("https://edge.forgecdn.net/files/{}/{:03}/", first4, last3);
        let files_cf_url = format!(
            "https://files-cf.curseforge.com/file/curseforge-files/{}/{:03}/",
            first4, last3
        );
        let www_redirect_url = format!(
            "https://www.curseforge.com/minecraft/mc-mods/{}/download/{}/file",
            pid, fid
        );
        let api_redirect_url = format!(
            "https://api.curseforge.com/v1/mods/{}/files/{}/download",
            pid, fid
        );
        external_files.push(ModpackExternalFile {
            relative_path: String::new(),
            urls: vec![www_redirect_url, files_cf_url, cdn_url, api_redirect_url],
            sha1: None,
            size: None,
            project_id: Some(pid),
            file_id: Some(fid),
        });
    }
    let overrides_prefix = if !manifest.overrides.is_empty() {
        manifest.overrides.clone()
    } else {
        "overrides".to_string()
    };
    let mut extracted_files = Vec::new();
    for i in 0..zip.len() {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        if name == "manifest.json" {
            continue;
        }
        if name.ends_with('/') {
            continue;
        }
        let relative = if let Some(rest) = name.strip_prefix(&format!("{}/", overrides_prefix)) {
            rest.to_string()
        } else {
            continue;
        };
        if relative.is_empty()
            || relative.contains("..")
            || relative.starts_with('/')
            || relative.starts_with('\\')
        {
            continue;
        }
        let mut buf = Vec::new();
        use std::io::Read;
        if entry.read_to_end(&mut buf).is_err() {
            continue;
        }
        extracted_files.push((PathBuf::from(relative), buf));
    }
    Ok(ParsedModpack {
        name: if !manifest.name.is_empty() {
            manifest.name
        } else {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "modpack".to_string())
        },
        mc_version,
        loader_type,
        loader_version,
        external_files,
        extracted_files,
    })
}
fn parse_curseforge_loaders(
    loaders: &[CurseForgeModLoader],
    _mc_version: &str,
) -> (ModpackLoaderType, Option<String>) {
    for loader in loaders {
        let id = &loader.id;
        if id.starts_with("forge") {
            let ver = id.strip_prefix("forge-").unwrap_or(id);
            return (ModpackLoaderType::Forge, Some(ver.to_string()));
        }
        if id.starts_with("neoforge") {
            let ver = id.strip_prefix("neoforge-").unwrap_or(id);
            return (ModpackLoaderType::Neoforge, Some(ver.to_string()));
        }
        if id.starts_with("fabric") {
            let ver = id.strip_prefix("fabric-").unwrap_or(id);
            return (ModpackLoaderType::Fabric, Some(ver.to_string()));
        }
        if id.starts_with("quilt") {
            let ver = id.strip_prefix("quilt-").unwrap_or(id);
            return (ModpackLoaderType::Quilt, Some(ver.to_string()));
        }
        if id.starts_with("liteloader") {
            let ver = id.strip_prefix("liteloader-").unwrap_or(id);
            return (ModpackLoaderType::LiteLoader, Some(ver.to_string()));
        }
    }
    (ModpackLoaderType::Vanilla, None)
}

fn recommend_fabric_loader_version(mc_version: &str, requested_version: &str) -> String {
    let version_map: &[(&str, &str, &str)] = &[
        ("1.18.2", "0.14.9", "0.15.3"),
        ("1.19", "0.14.10", "0.15.3"),
        ("1.19.1", "0.14.10", "0.15.3"),
        ("1.19.2", "0.14.10", "0.15.3"),
        ("1.19.3", "0.14.10", "0.15.3"),
        ("1.19.4", "0.14.17", "0.15.3"),
        ("1.20", "0.15.0", "0.15.3"),
        ("1.20.1", "0.15.0", "0.15.3"),
        ("1.20.2", "0.15.1", "0.15.3"),
        ("1.20.3", "0.15.2", "0.15.3"),
        ("1.20.4", "0.15.2", "0.15.3"),
        ("1.20.5", "0.15.3", "0.15.3"),
        ("1.20.6", "0.15.3", "0.15.3"),
        ("1.21", "0.15.3", "0.15.3"),
        ("1.21.1", "0.15.3", "0.15.3"),
    ];

    for &(mc_ver, old_ver, new_ver) in version_map {
        if mc_version == mc_ver && requested_version == old_ver {
            return new_ver.to_string();
        }
    }

    requested_version.to_string()
}

pub async fn install_modpack_from_zip(
    zip_path: &Path,
    minecraft_path: &Path,
    progress_tx: Option<tokio::sync::mpsc::Sender<(usize, usize, String, String)>>,
    java_path: Option<&str>,
) -> Result<(String, usize)> {
    println!("[Modpack] 解析整合包: {:?}", zip_path);
    let parsed = parse_modpack_from_zip(zip_path)?;
    println!(
        "[Modpack] 名称: {}, MC: {}, Loader: {:?} {:?}, 外部文件: {}, 内置文件: {}",
        parsed.name,
        parsed.mc_version,
        parsed.loader_type,
        parsed.loader_version,
        parsed.external_files.len(),
        parsed.extracted_files.len()
    );
    install_parsed_modpack(&parsed, minecraft_path, progress_tx, java_path).await
}
pub async fn install_parsed_modpack(
    parsed: &ParsedModpack,
    minecraft_path: &Path,
    progress_tx: Option<tokio::sync::mpsc::Sender<(usize, usize, String, String)>>,
    java_path: Option<&str>,
) -> Result<(String, usize)> {
    let instance_name = sanitize_instance_name(&format!("{}-{}", parsed.name, parsed.mc_version));
    let version_dir = minecraft_path.join("versions").join(&instance_name);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("创建版本目录失败: {:?}", version_dir))?;
    println!("[Modpack] 实例目录: {:?}", version_dir);
    let instance_root = version_dir.clone();
    for (relative, content) in &parsed.extracted_files {
        let rel_str = relative.to_string_lossy();
        if rel_str.is_empty()
            || rel_str.contains("..")
            || rel_str.starts_with('/')
            || rel_str.starts_with('\\')
        {
            continue;
        }
        let target = instance_root.join(relative);
        let canonical = target.canonicalize().unwrap_or_else(|_| target.clone());
        if !canonical.starts_with(&instance_root) && !target.starts_with(&instance_root) {
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&target, content);
    }
    println!(
        "[Modpack] 已写入 {} 个内置文件",
        parsed.extracted_files.len()
    );
    let task_count: usize = parsed
        .external_files
        .iter()
        .filter(|f| !f.urls.is_empty())
        .count();
    let total_files: usize = 2 + task_count;
    fn send_progress(
        tx: &Option<tokio::sync::mpsc::Sender<(usize, usize, String, String)>>,
        downloaded: usize,
        total: usize,
        fname: String,
        stage: String,
    ) {
        if let Some(ref t) = tx {
            let _ = t.try_send((downloaded, total, fname, stage));
        }
    }
    let mc_version = parsed.mc_version.clone();
    let mc_dir = minecraft_path.to_path_buf();
    let mc_task = {
        let mc_dir_clone = mc_dir.clone();
        let mc_ver = mc_version.clone();
        let tx_clone = progress_tx.clone();
        tokio::spawn(async move {
            println!("[Modpack] [并行] 下载原版 Minecraft {}...", mc_ver);
            send_progress(
                &tx_clone,
                0,
                total_files,
                String::new(),
                "下载原版 Minecraft".to_string(),
            );
            let version_dir = mc_dir_clone.join("versions").join(&mc_ver);
            let json_path = version_dir.join(format!("{}.json", mc_ver));
            let jar_path = version_dir.join(format!("{}.jar", mc_ver));
            if !json_path.exists() || !jar_path.exists() {
                let (tx_local, mut rx_local) = tokio::sync::mpsc::channel::<f64>(64);
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let tx_forward = tx_clone.clone();
                let forward = tokio::spawn(async move {
                    while let Some(_p) = rx_local.recv().await {
                        send_progress(
                            &tx_forward,
                            0,
                            total_files,
                            String::new(),
                            "下载原版 Minecraft".to_string(),
                        );
                    }
                });
                let _ = crate::downloader::original_dwl::process_version(
                    &mc_ver,
                    &mc_dir_clone,
                    tx_local,
                    cancel,
                )
                .await;
                forward.abort();
            } else {
                println!("[Modpack] 原版 Minecraft {} 已存在，跳过", mc_ver);
            }
            send_progress(
                &tx_clone,
                1,
                total_files,
                String::new(),
                "原版 Minecraft 完成".to_string(),
            );
        })
    };
    let parsed_loader = parsed.loader_type.clone();
    let parsed_loader_ver = parsed.loader_version.clone();
    let parsed_mc = parsed.mc_version.clone();
    let java_path_str = java_path.unwrap_or("").to_string();
    let loader_task: tokio::task::JoinHandle<std::result::Result<(String, String), anyhow::Error>> = {
        let mc_dir_str = mc_dir.to_string_lossy().to_string();
        let tx_clone = progress_tx.clone();
        tokio::spawn(async move {
            send_progress(
                &tx_clone,
                1,
                total_files,
                String::new(),
                "安装 ModLoader".to_string(),
            );
            match parsed_loader {
                ModpackLoaderType::Vanilla => {
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((parsed_mc.clone(), String::new()))
                }
                ModpackLoaderType::Forge => {
                    let forge_ver = parsed_loader_ver.unwrap_or_else(|| "latest".to_string());
                    println!("[Modpack] [并行] 安装 Forge {}...", forge_ver);
                    let version_id = crate::downloader::forge_installer::install_forge(
                        &parsed_mc,
                        &forge_ver,
                        &mc_dir_str,
                        &java_path_str,
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| anyhow!("Forge 安装失败: {}", e))?;
                    println!("[Modpack] Forge 安装完成, 实际版本目录: {}", version_id);
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((version_id, "forge".to_string()))
                }
                ModpackLoaderType::Neoforge => {
                    let neo_ver = parsed_loader_ver.unwrap_or_else(|| "latest".to_string());
                    println!("[Modpack] [并行] 安装 NeoForge {}...", neo_ver);
                    let version_id = crate::downloader::neoforge_installer::install_neoforge(
                        &parsed_mc,
                        &neo_ver,
                        &mc_dir_str,
                        &java_path_str,
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| anyhow!("NeoForge 安装失败: {}", e))?;
                    println!("[Modpack] Neoforge 安装完成, 实际版本目录: {}", version_id);
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((version_id, "neoforge".to_string()))
                }
                ModpackLoaderType::Fabric => {
                    let mut fabric_ver = parsed_loader_ver.unwrap_or_else(|| "latest".to_string());
                    if fabric_ver != "latest" {
                        let recommended_ver =
                            recommend_fabric_loader_version(&parsed_mc, &fabric_ver);
                        if recommended_ver != fabric_ver {
                            println!(
                                "[Modpack] [并行] Fabric Loader {} 版本过旧，升级到 {}...",
                                fabric_ver, recommended_ver
                            );
                            fabric_ver = recommended_ver;
                        }
                    }
                    println!("[Modpack] [并行] 安装 Fabric Loader {}...", fabric_ver);
                    let version_id = crate::downloader::fabric_installer::install_fabric_loader(
                        &parsed_mc,
                        &fabric_ver,
                        &mc_dir_str,
                        true,
                    )
                    .await
                    .map_err(|e| anyhow!("Fabric 安装失败: {}", e))?;
                    println!("[Modpack] Fabric 安装完成, 实际版本目录: {}", version_id);
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((version_id, "fabric".to_string()))
                }
                ModpackLoaderType::Quilt => {
                    let quilt_ver = parsed_loader_ver.unwrap_or_else(|| "latest".to_string());
                    println!("[Modpack] [并行] 安装 Quilt Loader {}...", quilt_ver);
                    let version_id = crate::downloader::quilt_installer::install_quilt_loader(
                        &parsed_mc,
                        &quilt_ver,
                        &mc_dir_str,
                        None,
                    )
                    .await
                    .map_err(|e| anyhow!("Quilt 安装失败: {}", e))?;
                    println!("[Modpack] Quilt 安装完成, 实际版本目录: {}", version_id);
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((version_id, "quilt".to_string()))
                }
                ModpackLoaderType::LiteLoader => {
                    let lite_ver = parsed_loader_ver.unwrap_or_else(|| parsed_mc.clone());
                    println!("[Modpack] [并行] 安装 LiteLoader {}...", lite_ver);
                    let version_id = crate::downloader::liteloader_installer::install_liteloader(
                        &parsed_mc,
                        &lite_ver,
                        &mc_dir_str,
                        &java_path_str,
                        None,
                    )
                    .await
                    .map_err(|e| anyhow!("LiteLoader 安装失败: {}", e))?;
                    println!(
                        "[Modpack] LiteLoader 安装完成, 实际版本目录: {}",
                        version_id
                    );
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((version_id, "liteloader".to_string()))
                }
                ModpackLoaderType::Optifine => {
                    send_progress(
                        &tx_clone,
                        2,
                        total_files,
                        String::new(),
                        "ModLoader 安装完成".to_string(),
                    );
                    Ok((parsed_mc.clone(), String::new()))
                }
            }
        })
    };
    let external_files = parsed.external_files.clone();
    let instance_root_clone = instance_root.clone();
    let external_task = {
        let tx = progress_tx.clone();
        tokio::spawn(async move {
            let mods_dir = instance_root_clone.join("mods");
            let datapacks_dir = instance_root_clone.join("datapacks");
            let resourcepacks_dir = instance_root_clone.join("resourcepacks");
            let shaderpacks_dir = instance_root_clone.join("shaderpacks");
            let _ = ensure_dir(&mods_dir);

            // --- 阶段 1: 预解析 CurseForge 文件信息 ---
            let total_cf = external_files
                .iter()
                .filter(|f| f.project_id.is_some())
                .count();
            println!("[Modpack] 预解析 {} 个 CurseForge 文件...", total_cf);

            let mut cf_futures = Vec::new();
            for f in &external_files {
                if let (Some(pid), Some(fid)) = (f.project_id, f.file_id) {
                    cf_futures.push(async move {
                        (
                            pid,
                            fid,
                            crate::downloader::modular_download::resolve_cf_download_info(pid, fid)
                                .await,
                        )
                    });
                }
            }

            let cf_results = futures::future::join_all(cf_futures).await;
            let mut cf_map: std::collections::HashMap<(u64, u64), String> =
                std::collections::HashMap::new();
            for (pid, fid, result) in cf_results {
                if let Some((name, _sha, cdn)) = result {
                    cf_map.insert((pid, fid), name);
                    drop(cdn);
                }
            }
            println!(
                "[Modpack] CurseForge 预解析完成: 成功 {}/{}",
                cf_map.len(),
                total_cf
            );

            // --- 阶段 2: 构造下载任务，直接下载到目标目录 ---
            let mut download_tasks: Vec<DownloadTask> = Vec::new();
            for f in &external_files {
                if f.urls.is_empty() {
                    continue;
                }
                let rel = f.relative_path.trim();
                if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
                    continue;
                }

                let (file_name, target_dir) = if !rel.is_empty() {
                    // 有明确相对路径（来自 Modrinth/其他）
                    let full = instance_root_clone.join(rel);
                    let dir = full
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| mods_dir.clone());
                    let name = full
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    (name, dir)
                } else if let (Some(pid), Some(fid)) = (f.project_id, f.file_id) {
                    // CurseForge 文件 - 用预解析的文件名
                    let name = cf_map
                        .get(&(pid, fid))
                        .cloned()
                        .unwrap_or_else(|| format!("mod_{}.jar", fid));
                    // 根据扩展名判断目录
                    let lower = name.to_lowercase();
                    let dir = if lower.ends_with(".zip") {
                        // 先假定是 shaderpack，下载后再检测
                        shaderpacks_dir.clone()
                    } else {
                        mods_dir.clone()
                    };
                    (name, dir)
                } else {
                    continue;
                };

                if file_name.is_empty() {
                    continue;
                }
                let _ = ensure_dir(&target_dir);

                download_tasks.push(DownloadTask {
                    file_name,
                    target_dir,
                    urls: f.urls.clone(),
                    sha1: f.sha1.clone(),
                });
            }

            let task_count = download_tasks.len();
            if task_count == 0 {
                println!("[Modpack] 没有可下载的外部资源");
                return;
            }
            println!("[Modpack] 开始并发下载 {} 个文件", task_count);
            for t in &download_tasks {
                println!("[Modpack]   → {}/{}", t.target_dir.display(), t.file_name);
            }

            let (inner_tx, mut inner_rx) = tokio::sync::mpsc::channel::<(usize, usize, String)>(32);
            let tf = total_files;
            let forward = tokio::spawn(async move {
                if let Some(ref ext_tx) = tx {
                    while let Some((done, _total, fname)) = inner_rx.recv().await {
                        println!("[Modpack] 进度: {}/{} | {}", done, tf, fname);
                        let _ = ext_tx.try_send((
                            2 + done,
                            tf,
                            fname.clone(),
                            "下载资源文件".to_string(),
                        ));
                    }
                }
            });
            let result =
                concurrent_download::download_all_with_file_info(download_tasks, Some(inner_tx))
                    .await;
            forward.abort();
            println!(
                "[Modpack] 下载完成: 成功 {}, 失败 {}",
                result.success_count,
                result.failures.len()
            );
            for fail in &result.failures {
                println!("[Modpack] ✗ 失败: {} | {}", fail.file_name, fail.error);
            }

            // --- 阶段 3: 扫描 zip 文件，判断类型并移动 ---
            println!("[Modpack] 检测 zip 文件类型...");
            let scan_dirs = vec![mods_dir.clone(), shaderpacks_dir.clone()];
            let mut to_move: Vec<(PathBuf, PathBuf)> = Vec::new();
            for dir in &scan_dirs {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let src = entry.path();
                        if src.is_dir() {
                            continue;
                        }
                        let ext = src
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if ext != "zip" {
                            continue;
                        }
                        let fname = src
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if fname.is_empty() {
                            continue;
                        }
                        let category = detect_file_category(&src);
                        let target_dir = match category {
                            "datapacks" => Some(&datapacks_dir),
                            "resourcepacks" => Some(&resourcepacks_dir),
                            "shaderpacks" => Some(&shaderpacks_dir),
                            _ => None,
                        };
                        if let Some(td) = target_dir {
                            if td != dir {
                                to_move.push((src.clone(), td.join(&fname)));
                            }
                        }
                    }
                }
            }

            let mut moved = 0usize;
            for (src, dest) in &to_move {
                if let Err(e) = ensure_dir(dest.parent().unwrap()) {
                    eprintln!("[Modpack] 创建目录失败: {}", e);
                    continue;
                }
                // 先尝试删除目标，避免 Windows 上 rename 不能覆盖
                let _ = fs::remove_file(dest);
                match fs::rename(src, dest) {
                    Ok(_) => {
                        moved += 1;
                        println!(
                            "[Modpack] 移动: {} -> {}",
                            src.file_name().unwrap().to_string_lossy(),
                            dest.display()
                        );
                    }
                    Err(_) => {
                        // rename 失败，尝试 copy + remove
                        if let Err(e2) = fs::copy(src, dest).and_then(|_| fs::remove_file(src)) {
                            eprintln!("[Modpack] 移动失败 {}: {}", src.display(), e2);
                        } else {
                            moved += 1;
                        }
                    }
                }
            }
            if moved > 0 {
                println!("[Modpack] 分类移动完成: 处理 {} 个 zip 文件", moved);
            } else if !to_move.is_empty() {
                println!("[Modpack] 分类移动: 没有需要移动的文件");
            }
        })
    };
    println!("[Modpack] 所有下载任务已启动（并行执行）");
    if let Err(e) = mc_task.await {
        println!("[Modpack] 警告: 原版 Minecraft 下载任务失败: {}", e);
    }
    let (final_version_name, loader_type_hint) = match loader_task.await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            println!("[Modpack] 错误: ModLoader 安装失败: {}", e);
            return Err(e.context("整合包安装失败：ModLoader 安装失败"));
        }
        Err(e) => {
            println!("[Modpack] 错误: ModLoader 安装任务崩溃: {}", e);
            return Err(anyhow!("整合包安装失败：ModLoader 安装任务崩溃: {}", e));
        }
    };
    if let Err(e) = external_task.await {
        println!("[Modpack] 警告: 外部资源下载任务失败: {}", e);
    }
    println!("[Modpack] 所有并行任务完成，开始合并 version.json");
    let effective_loader_name = if matches!(parsed.loader_type, ModpackLoaderType::Vanilla) {
        parsed.mc_version.clone()
    } else {
        final_version_name.clone()
    };
    if let Err(e) = merge_version_jsons_for_modpack(
        &instance_name,
        &parsed.mc_version,
        &effective_loader_name,
        &loader_type_hint,
        minecraft_path,
    ) {
        println!(
            "[Modpack] 警告: 合并 version.json 失败 ({}), 回退到简单 inherits 模式",
            e
        );
        let version_dir = minecraft_path.join("versions").join(&instance_name);
        fs::create_dir_all(&version_dir).ok();
        let json_path = version_dir.join(format!("{}.json", instance_name));
        if !json_path.exists() {
            use serde_json::json;
            let fallback = json!({
                "id": instance_name,
                "inheritsFrom": effective_loader_name,
                "type": "release",
                "time": current_iso_time(),
                "releaseTime": current_iso_time(),
                "minimumLauncherVersion": 21,
                "libraries": [],
            });
            let _ = fs::write(
                &json_path,
                serde_json::to_string_pretty(&fallback).unwrap_or_default(),
            );
        }
        let vanilla_jar = minecraft_path
            .join("versions")
            .join(&parsed.mc_version)
            .join(format!("{}.jar", parsed.mc_version));
        let target_jar = version_dir.join(format!("{}.jar", instance_name));
        if vanilla_jar.exists() && !target_jar.exists() {
            let _ = fs::copy(&vanilla_jar, &target_jar);
        }
        for sub in &["mods", "resourcepacks", "shaderpacks", "config", "saves"] {
            let _ = fs::create_dir_all(version_dir.join(sub));
        }
    }
    if let Some(tx) = &progress_tx {
        let _ = tx.try_send((
            total_files,
            total_files,
            "完成".to_string(),
            "完成".to_string(),
        ));
    }
    // 确保 options.txt 存在并设置语言为中文
    let versions_dir = minecraft_path.join("versions").join(&instance_name);
    let options_path = versions_dir.join("options.txt");
    if options_path.exists() {
        let content = fs::read_to_string(&options_path)?;
        if content.contains("lang:") {
            let new_content = content
                .lines()
                .map(|line| {
                    if line.trim().starts_with("lang:") {
                        "lang:zh_cn"
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&options_path, new_content)?;
        } else {
            fs::write(&options_path, format!("{}\nlang:zh_cn", content))?;
        }
    } else {
        fs::write(&options_path, "lang:zh_cn")?;
    }
    
    println!(
        "[Modpack] 整合包安装完成: {} (版本名: {})",
        parsed.name, instance_name
    );
    Ok((instance_name, task_count))
}
fn sanitize_instance_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
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
        "modpack-instance".to_string()
    } else {
        trimmed.to_string()
    }
}
fn merge_version_jsons_for_modpack(
    instance_name: &str,
    mc_version: &str,
    loader_version_name: &str,
    loader_type_hint: &str,
    minecraft_path: &Path,
) -> Result<()> {
    use serde_json::Value;
    let versions_root = minecraft_path.join("versions");
    let vanilla_json_path = versions_root
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    let vanilla: Value = if vanilla_json_path.exists() {
        let text = fs::read_to_string(&vanilla_json_path)
            .with_context(|| format!("读取原版 version.json 失败: {:?}", vanilla_json_path))?;
        serde_json::from_str(&text).context("解析原版 version.json 失败")?
    } else {
        bail!("找不到原版 version.json: {:?}", vanilla_json_path);
    };
    let loader: Option<Value> = {
        let mut try_paths: Vec<std::path::PathBuf> = Vec::new();
        try_paths.push(
            versions_root
                .join(loader_version_name)
                .join(format!("{}.json", loader_version_name)),
        );
        if let Ok(entries) = fs::read_dir(&versions_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = match entry.file_name().into_string() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if dir_name == mc_version || dir_name == instance_name {
                    continue;
                }
                if !dir_name.starts_with(mc_version) {
                    continue;
                }
                let json_path = path.join(format!("{}.json", dir_name));
                if json_path.exists() {
                    let is_match = if !loader_type_hint.is_empty() {
                        let dir_lower = dir_name.to_ascii_lowercase();
                        let hint_lower = loader_type_hint.to_ascii_lowercase();
                        dir_lower.contains(&hint_lower)
                    } else {
                        false
                    };
                    if is_match {
                        if try_paths.len() < 2 {
                            try_paths.push(json_path);
                        } else {
                            try_paths.insert(1, json_path);
                        }
                    } else {
                        try_paths.push(json_path);
                    }
                }
            }
        }
        let mut result: Option<Value> = None;
        for p in &try_paths {
            if p.exists() {
                if let Ok(text) = fs::read_to_string(p) {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if v.get("mainClass").is_some() {
                            println!("[Modpack] 找到 loader version.json: {}", p.display());
                            result = Some(v);
                            break;
                        }
                    }
                }
            }
        }
        result
    };
    let mut result = vanilla.clone();
    {
        let obj = result
            .as_object_mut()
            .ok_or_else(|| anyhow!("原版 version.json 顶层不是 JSON 对象"))?;
        obj.insert("id".to_string(), Value::String(instance_name.to_string()));
        obj.insert("time".to_string(), Value::String(current_iso_time()));
        obj.insert("releaseTime".to_string(), Value::String(current_iso_time()));
        if !obj.contains_key("minimumLauncherVersion") {
            obj.insert(
                "minimumLauncherVersion".to_string(),
                Value::Number(21.into()),
            );
        }
        obj.remove("inheritsFrom");
        obj.remove("inherits-from");
        if let Some(loader_obj) = loader.as_ref().and_then(|v| v.as_object()) {
            if let Some(mc) = loader_obj.get("mainClass") {
                obj.insert("mainClass".to_string(), mc.clone());
            }
            if let Some(loader_args) = loader_obj.get("arguments").and_then(|v| v.as_object()) {
                let vanilla_args_obj = obj.get("arguments").and_then(|v| v.as_object()).cloned();
                let get_array = |o: &serde_json::Map<String, Value>, key: &str| -> Vec<Value> {
                    o.get(key)
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default()
                };
                let loader_jvm = get_array(loader_args, "jvm");
                let vanilla_jvm = vanilla_args_obj
                    .as_ref()
                    .map(|o| get_array(o, "jvm"))
                    .unwrap_or_default();
                let mut merged_jvm = loader_jvm;
                merged_jvm.extend(vanilla_jvm);
                let loader_game = get_array(loader_args, "game");
                let vanilla_game = vanilla_args_obj
                    .as_ref()
                    .map(|o| get_array(o, "game"))
                    .unwrap_or_default();
                let mut merged_game = loader_game;
                merged_game.extend(vanilla_game);
                let mut new_args = serde_json::Map::new();
                if !merged_jvm.is_empty() {
                    new_args.insert("jvm".to_string(), Value::Array(merged_jvm));
                }
                if !merged_game.is_empty() {
                    new_args.insert("game".to_string(), Value::Array(merged_game));
                }
                for (k, v) in loader_args {
                    if k != "jvm" && k != "game" {
                        new_args.insert(k.clone(), v.clone());
                    }
                }
                obj.insert("arguments".to_string(), Value::Object(new_args));
            } else if let Some(mc_args) = loader_obj.get("minecraftArguments") {
                obj.insert("minecraftArguments".to_string(), mc_args.clone());
            }
            if let Some(typ) = loader_obj.get("type") {
                obj.insert("type".to_string(), typ.clone());
            }
            for (k, v) in loader_obj {
                if k == "libraries"
                    || k == "id"
                    || k == "inheritsFrom"
                    || k == "inherits-from"
                    || k == "downloads"
                    || k == "assetIndex"
                    || k == "assets"
                    || k == "logging"
                    || k == "time"
                    || k == "releaseTime"
                    || k == "mainClass"
                    || k == "arguments"
                    || k == "minecraftArguments"
                    || k == "type"
                {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
            }
        }
        let mut merged_libs: Vec<Value> = Vec::new();
        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        let extract_gac = |name: &str| -> String {
            let parts: Vec<&str> = name.split(':').collect();
            match parts.len() {
                3 => {
                    format!("{}:{}", parts[0], parts[1])
                }
                4 => {
                    format!("{}:{}:{}", parts[0], parts[1], parts[3])
                }
                _ => name.to_string(),
            }
        };
        let extract_path_key = |path: &str| -> String {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 3 {
                let take = parts.len() - 2;
                parts[..take].join("/")
            } else {
                path.to_string()
            }
        };
        if let Some(loader_obj) = loader.as_ref().and_then(|v| v.as_object()) {
            if let Some(libs) = loader_obj.get("libraries").and_then(|v| v.as_array()) {
                for lib in libs {
                    if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
                        let key = extract_gac(name);
                        if seen_keys.insert(key) {
                            merged_libs.push(lib.clone());
                        }
                    } else if let Some(artifact_path) = lib
                        .get("downloads")
                        .and_then(|d| d.get("artifact"))
                        .and_then(|a| a.get("path"))
                        .and_then(|p| p.as_str())
                    {
                        let key = extract_path_key(artifact_path);
                        if seen_keys.insert(key) {
                            merged_libs.push(lib.clone());
                        }
                    } else {
                        merged_libs.push(lib.clone());
                    }
                }
            }
        }
        if let Some(libs) = obj.get("libraries").and_then(|v| v.as_array()) {
            for lib in libs {
                if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
                    let key = extract_gac(name);
                    if seen_keys.insert(key) {
                        merged_libs.push(lib.clone());
                    }
                } else if let Some(artifact_path) = lib
                    .get("downloads")
                    .and_then(|d| d.get("artifact"))
                    .and_then(|a| a.get("path"))
                    .and_then(|p| p.as_str())
                {
                    let key = extract_path_key(artifact_path);
                    if seen_keys.insert(key) {
                        merged_libs.push(lib.clone());
                    }
                } else {
                    merged_libs.push(lib.clone());
                }
            }
        }
        obj.insert("libraries".to_string(), Value::Array(merged_libs));
    }
    let version_dir = versions_root.join(instance_name);
    fs::create_dir_all(&version_dir).ok();
    let json_path = version_dir.join(format!("{}.json", instance_name));
    let text = serde_json::to_string_pretty(&result).context("序列化合并后的 version.json 失败")?;
    fs::write(&json_path, text)
        .with_context(|| format!("写入合并后的 version.json 失败: {:?}", json_path))?;
    let vanilla_jar = versions_root
        .join(mc_version)
        .join(format!("{}.jar", mc_version));
    let target_jar = version_dir.join(format!("{}.jar", instance_name));
    if vanilla_jar.exists() {
        let _ = fs::copy(&vanilla_jar, &target_jar);
    }
    let vanilla_natives = versions_root
        .join(mc_version)
        .join(format!("{}-natives", mc_version));
    let target_natives = version_dir.join(format!("{}-natives", instance_name));
    if vanilla_natives.exists() {
        let _ = dircpy(&vanilla_natives, &target_natives);
    }
    for sub in &["mods", "resourcepacks", "shaderpacks", "config", "saves"] {
        let _ = fs::create_dir_all(version_dir.join(sub));
    }
    println!(
        "[Modpack] version.json merged: {} <- ({}, {})",
        json_path.display(),
        mc_version,
        loader_version_name
    );
    Ok(())
}
fn dircpy(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            dircpy(&path, &target)?;
        } else {
            let _ = fs::copy(&path, &target);
        }
    }
    Ok(())
}
fn current_iso_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let years = 1970 + (secs / 31536000);
    format!("{}-01-01T00:00:00+08:00", years)
}