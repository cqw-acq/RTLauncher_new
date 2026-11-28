use crate::handler::mod_links::{self, InstalledModIndex, ModCandidateIdentity};
use crate::handler::mod_parser::ModInfo;
use crate::handler::mod_parser::parse_mods_in_dir;
use crate::http_client::{get_with_retry, modrinth_client, curseforge_client, global_semaphore, shared_client, RetryConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use futures::future::join_all;
use zip::write::FileOptions;
use zip::ZipWriter;
use zip::CompressionMethod;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependencyIssue {
    pub mod_id: String,
    pub mod_name: String,
    pub issue_type: String,
    pub required_by: Vec<String>,
    pub version: Option<String>,
    pub recommended_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependenciesAnalysis {
    pub total_mods: usize,
    pub missing_dependencies: Vec<ModDependencyIssue>,
    pub version_mismatches: Vec<ModDependencyIssue>,
    pub incompatible_mods: Vec<ModDependencyIssue>,
    pub all_resolved: bool,
}

pub fn analyze_mod_dependencies(instance_dir: &Path) -> ModDependenciesAnalysis {
    let mods_dir = instance_dir.join("mods");
    
    if !mods_dir.exists() {
        return ModDependenciesAnalysis {
            total_mods: 0,
            missing_dependencies: vec![],
            version_mismatches: vec![],
            incompatible_mods: vec![],
            all_resolved: true,
        };
    }

    let mods = parse_mods_in_dir(mods_dir.to_string_lossy().to_string());
    let total_mods = mods.len();

    let mut missing_dependencies = Vec::new();
    let mut version_mismatches = Vec::new();
    let mut incompatible_mods = Vec::new();

    let mod_map: std::collections::HashMap<String, &ModInfo> = mods.iter()
        .map(|m| (m.mod_id.clone(), m))
        .collect();

    for mod_info in &mods {
        for dep in &mod_info.dependencies {
            if dep.mandatory {
                if let Some(dep_mod) = mod_map.get(&dep.mod_id) {
                    if let Some(ref version_range) = dep.version_range {
                        if !check_version_match(&dep_mod.version, version_range) {
                            version_mismatches.push(ModDependencyIssue {
                                mod_id: dep.mod_id.clone(),
                                mod_name: dep_mod.name.clone(),
                                issue_type: "version_mismatch".to_string(),
                                required_by: vec![mod_info.mod_id.clone()],
                                version: Some(dep_mod.version.clone()),
                                recommended_version: Some(version_range.clone()),
                            });
                        }
                    }
                } else {
                    missing_dependencies.push(ModDependencyIssue {
                        mod_id: dep.mod_id.clone(),
                        mod_name: dep.mod_id.clone(),
                        issue_type: "missing".to_string(),
                        required_by: vec![mod_info.mod_id.clone()],
                        version: None,
                        recommended_version: dep.version_range.clone(),
                    });
                }
            }
        }

        for dep in &mod_info.incompatible_dependencies {
            if let Some(dep_mod) = mod_map.get(&dep.mod_id) {
                incompatible_mods.push(ModDependencyIssue {
                    mod_id: dep.mod_id.clone(),
                    mod_name: dep_mod.name.clone(),
                    issue_type: "incompatible".to_string(),
                    required_by: vec![mod_info.mod_id.clone()],
                    version: Some(dep_mod.version.clone()),
                    recommended_version: None,
                });
            }
        }
    }

    let all_resolved = missing_dependencies.is_empty() 
        && version_mismatches.is_empty() 
        && incompatible_mods.is_empty();

    ModDependenciesAnalysis {
        total_mods,
        missing_dependencies,
        version_mismatches,
        incompatible_mods,
        all_resolved,
    }
}

fn check_version_match(version: &str, range: &str) -> bool {
    if range.is_empty() || range == "*" {
        return true;
    }

    if range.starts_with(">=") {
        let required = &range[2..];
        return version >= required;
    }

    if range.starts_with(">") {
        let required = &range[1..];
        return version > required;
    }

    if range.starts_with("<=") {
        let required = &range[2..];
        return version <= required;
    }

    if range.starts_with("<") {
        let required = &range[1..];
        return version < required;
    }

    if range.starts_with("=") {
        let required = &range[1..];
        return version == required;
    }

    if range.contains('-') {
        let parts: Vec<&str> = range.split('-').collect();
        if parts.len() == 2 {
            let (start, end) = (parts[0], parts[1]);
            return version >= start && version <= end;
        }
    }

    version == range
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub java_version: String,
    pub java_path: String,
}

pub fn collect_system_info() -> SystemInfo {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    
    let os_version = match os.as_str() {
        "windows" => {
            std::process::Command::new("cmd")
                .args(["/c", "ver"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        "linux" => {
            std::process::Command::new("cat")
                .arg("/etc/os-release")
                .output()
                .map(|o| {
                    let output = String::from_utf8_lossy(&o.stdout);
                    output.lines()
                        .find(|line| line.starts_with("PRETTY_NAME"))
                        .and_then(|line| line.split('=').nth(1))
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        "macos" => {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        _ => "Unknown".to_string(),
    };

    let cpu_model = match os.as_str() {
        "windows" => {
            std::process::Command::new("cmd")
                .args(["/c", "wmic cpu get name"])
                .output()
                .map(|o| {
                    let output = String::from_utf8_lossy(&o.stdout);
                    output.lines().nth(1).unwrap_or("Unknown").trim().to_string()
                })
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        "linux" => {
            std::process::Command::new("cat")
                .arg("/proc/cpuinfo")
                .output()
                .map(|o| {
                    let output = String::from_utf8_lossy(&o.stdout);
                    output.lines()
                        .find(|line| line.starts_with("model name"))
                        .and_then(|line| line.split(':').nth(1))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        "macos" => {
            std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        _ => "Unknown".to_string(),
    };

    let cpu_cores = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);

    let (total_mem, available_mem) = get_memory_info();

    SystemInfo {
        os,
        os_version,
        arch,
        cpu_model,
        cpu_cores,
        total_memory_mb: total_mem / (1024 * 1024),
        available_memory_mb: available_mem / (1024 * 1024),
        java_version: "".to_string(),
        java_path: "".to_string(),
    }
}

fn get_memory_info() -> (u64, u64) {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("cmd")
            .args(["/c", "wmic OS get FreePhysicalMemory,TotalVisibleMemorySize /Value"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut total = 0u64;
            let mut free = 0u64;

            for line in stdout.lines() {
                if line.starts_with("TotalVisibleMemorySize=") {
                    if let Some(val) = line.split('=').nth(1) {
                        total = val.trim().parse().unwrap_or(0) * 1024;
                    }
                }
                if line.starts_with("FreePhysicalMemory=") {
                    if let Some(val) = line.split('=').nth(1) {
                        free = val.trim().parse().unwrap_or(0) * 1024;
                    }
                }
            }
            return (total, free);
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("cat")
            .arg("/proc/meminfo")
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut total = 0u64;
            let mut available = 0u64;

            for line in stdout.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total = val.parse().unwrap_or(0) * 1024; // Convert KB to bytes
                    }
                }
                if line.starts_with("MemAvailable:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        available = val.parse().unwrap_or(0) * 1024; // Convert KB to bytes
                    }
                }
            }
            // If MemAvailable is not available, calculate it from MemFree + Buffers + Cached
            if available == 0 {
                let mut mem_free = 0u64;
                let mut buffers = 0u64;
                let mut cached = 0u64;

                for line in stdout.lines() {
                    if line.starts_with("MemFree:") {
                        if let Some(val) = line.split_whitespace().nth(1) {
                            mem_free = val.parse().unwrap_or(0) * 1024;
                        }
                    }
                    if line.starts_with("Buffers:") {
                        if let Some(val) = line.split_whitespace().nth(1) {
                            buffers = val.parse().unwrap_or(0) * 1024;
                        }
                    }
                    if line.starts_with("Cached:") {
                        if let Some(val) = line.split_whitespace().nth(1) {
                            cached = val.parse().unwrap_or(0) * 1024;
                        }
                    }
                }
                available = mem_free + buffers + cached;
            }
            return (total, available);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(total) = stdout.trim().parse::<u64>() {
                // For macOS, we'll use total as available since getting available memory is complex
                return (total, total);
            }
        }
    }

    (0, 0)
}

#[tauri::command]
pub async fn get_mod_dependencies_analysis(instance_dir: String) -> Result<ModDependenciesAnalysis, String> {
    let path = Path::new(&instance_dir);
    Ok(analyze_mod_dependencies(path))
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    Ok(collect_system_info())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModSearchResult {
    pub source: String,
    pub mod_id: String,
    pub slug: String,
    pub title: String,
    pub download_url: Option<String>,
    pub file_name: Option<String>,
    pub mc_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub version_number: Option<String>,
    /// Modrinth project id or CurseForge mod id of the selected file.
    #[serde(default)]
    pub project_id: Option<String>,
    /// SHA-1 of the selected file, used to recognise an identical installed jar.
    #[serde(default)]
    pub file_sha1: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDownloadResult {
    pub mod_id: String,
    pub source: String,
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
}

/// Search hit for `query`: an exact slug or title match beats relevance order.
fn pick_search_hit<'a>(
    hits: &'a [serde_json::Value],
    query: &str,
    slug_key: &str,
    title_key: &str,
) -> Option<&'a serde_json::Value> {
    let wanted = mod_links::normalize_mod_key(query);
    hits.iter()
        .find(|hit| {
            [slug_key, title_key].iter().any(|key| {
                hit.get(*key)
                    .and_then(|value| value.as_str())
                    .map(|value| !wanted.is_empty() && mod_links::normalize_mod_key(value) == wanted)
                    .unwrap_or(false)
            })
        })
        .or_else(|| hits.first())
}

fn json_strings(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|values| values.as_array())
        .map(|values| values.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn modrinth_search_url(query: &str, mc_version: &str, loader: &str) -> String {
    let mut facets = Vec::new();
    if let Some(loader) = mod_links::canonical_mod_loader(loader) {
        facets.push(serde_json::json!([format!("categories:{}", loader)]));
    }
    facets.push(serde_json::json!([format!("versions:{}", mc_version)]));
    facets.push(serde_json::json!(["project_type:mod"]));
    format!(
        "https://api.modrinth.com/v2/search?query={}&facets={}&limit=5",
        urlencoding::encode(query),
        urlencoding::encode(&serde_json::Value::Array(facets).to_string())
    )
}

/// The Modrinth project best matching `query`, as (slug, title).
async fn search_modrinth_project(query: &str, mc_version: &str, loader: &str) -> Option<(String, String)> {
    let client = modrinth_client().await;
    let response = get_with_retry(
        &client,
        &modrinth_search_url(query, mc_version, loader),
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 2000,
        }),
    ).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().await.ok()?;
    let hits = json.get("hits")?.as_array()?;
    let hit = pick_search_hit(hits, query, "slug", "title")?;
    let slug = hit.get("slug")?.as_str()?.to_string();
    let title = hit.get("title").and_then(|t| t.as_str()).unwrap_or(&slug).to_string();
    Some((slug, title))
}

/// The version of a Modrinth project (id or slug) to install on this instance:
/// exact Minecraft version and loader, release before beta before alpha.
async fn modrinth_candidate(
    project: &str,
    identity: Option<(String, String)>,
    query: &str,
    mc_version: &str,
    loader: &str,
) -> Option<(ModSearchResult, serde_json::Value)> {
    let client = modrinth_client().await;
    let versions = mod_links::fetch_modrinth_project_versions(&client, project, mc_version, loader).await.ok()?;
    let version = mod_links::select_modrinth_version(&versions, mc_version, loader)?.clone();
    let file = mod_links::primary_modrinth_file(&version)?;
    let download_url = file.get("url")?.as_str()?.to_string();
    let file_name = file.get("filename")?.as_str()?.to_string();
    let project_id = version
        .get("project_id")
        .and_then(|id| id.as_str())
        .unwrap_or(project)
        .to_string();
    let (slug, title) = match identity {
        Some(identity) => identity,
        None => mod_links::get_modrinth_project_identity(&client, &project_id).await,
    };
    let result = ModSearchResult {
        source: "modrinth".to_string(),
        mod_id: query.to_string(),
        slug,
        title,
        download_url: Some(download_url),
        file_name: Some(file_name),
        mc_versions: json_strings(&version, "game_versions"),
        loaders: json_strings(&version, "loaders"),
        version_number: version.get("version_number").and_then(|v| v.as_str()).map(str::to_owned),
        project_id: Some(project_id),
        file_sha1: mod_links::modrinth_file_sha1s(&version),
    };
    Some((result, version))
}

async fn search_modrinth(query: &str, mc_version: &str, loader: &str) -> Option<ModSearchResult> {
    let (slug, title) = search_modrinth_project(query, mc_version, loader).await?;
    let project = slug.clone();
    modrinth_candidate(&project, Some((slug, title)), query, mc_version, loader)
        .await
        .map(|(result, _)| result)
}

fn curseforge_loader_type(loader: &str) -> &'static str {
    match mod_links::canonical_mod_loader(loader) {
        Some("forge") => "1",
        Some("liteloader") => "3",
        Some("fabric") => "4",
        Some("quilt") => "5",
        Some("neoforge") => "6",
        _ => "",
    }
}

/// CurseForge projects for `query` as (id, slug, name): an exact slug or name
/// match first, then the top search hits.
async fn search_curseforge_projects(query: &str, mc_version: &str, loader: &str) -> Vec<(i64, String, String)> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&searchFilter={}&gameVersion={}&modLoaderType={}&pageSize=5",
        urlencoding::encode(query),
        urlencoding::encode(mc_version),
        curseforge_loader_type(loader)
    );

    let Ok(_permit) = semaphore.acquire().await else {
        return Vec::new();
    };
    let response = match get_with_retry(
        &client,
        &url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 600,
            max_delay_ms: 3000,
        }),
    ).await {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };
    let Ok(json) = response.json::<serde_json::Value>().await else {
        return Vec::new();
    };
    let data = json.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();

    let mut ordered: Vec<&serde_json::Value> = pick_search_hit(&data, query, "slug", "name").into_iter().collect();
    for item in data.iter().take(3) {
        if !ordered.iter().any(|picked| std::ptr::eq(*picked, item)) {
            ordered.push(item);
        }
    }
    ordered
        .into_iter()
        .filter_map(|item| {
            Some((
                item.get("id")?.as_i64()?,
                item.get("slug")?.as_str()?.to_string(),
                item.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
            ))
        })
        .collect()
}

/// The CurseForge file of `mod_id` to install on this instance: exact Minecraft
/// version and loader, release before beta before alpha.
async fn curseforge_candidate(
    mod_id: i64,
    identity: Option<(String, String)>,
    query: &str,
    mc_version: &str,
    loader: &str,
) -> Option<(ModSearchResult, serde_json::Value)> {
    let client = curseforge_client().await;
    let files = mod_links::get_curseforge_files(&client, mod_id, Some(mc_version)).await.ok()?;
    // Only files the author lets third-party launchers download directly.
    let distributable: Vec<serde_json::Value> = files
        .into_iter()
        .filter(|file| {
            file.get("downloadUrl")
                .and_then(|url| url.as_str())
                .map(|url| !url.is_empty())
                .unwrap_or(false)
        })
        .collect();
    let file = mod_links::select_curseforge_compatible_file(&distributable, mc_version, loader)?;
    let download_url = file.get("downloadUrl")?.as_str()?.to_string();
    let file_name = file.get("fileName")?.as_str()?.to_string();
    let (slug, title) = match identity {
        Some(identity) => identity,
        None => mod_links::get_curseforge_project_identity(&client, mod_id).await,
    };
    let result = ModSearchResult {
        source: "curseforge".to_string(),
        mod_id: query.to_string(),
        slug,
        title,
        download_url: Some(download_url),
        file_name: Some(file_name),
        mc_versions: json_strings(&file, "gameVersions"),
        loaders: vec![loader.to_string()],
        version_number: file.get("displayName").and_then(|v| v.as_str()).map(str::to_owned),
        project_id: Some(mod_id.to_string()),
        file_sha1: mod_links::curseforge_file_sha1s(&file),
    };
    Some((result, file))
}

async fn search_curseforge(query: &str, mc_version: &str, loader: &str) -> Option<ModSearchResult> {
    for (mod_id, slug, name) in search_curseforge_projects(query, mc_version, loader).await {
        let title = if name.is_empty() { slug.clone() } else { name };
        if let Some((result, _)) = curseforge_candidate(mod_id, Some((slug, title)), query, mc_version, loader).await {
            return Some(result);
        }
    }
    None
}

async fn search_mod_both_sources(query: &str, mc_version: &str, loader: &str) -> (Option<ModSearchResult>, Option<ModSearchResult>) {
    let modrinth_fut = search_modrinth(query, mc_version, loader);
    let curseforge_fut = search_curseforge(query, mc_version, loader);
    
    let (modrinth_result, curseforge_result) = tokio::join!(modrinth_fut, curseforge_fut);
    
    (modrinth_result, curseforge_result)
}

async fn download_file(url: &str, dest_path: &Path) -> Result<(), String> {
    let client = shared_client().await;
    let response = get_with_retry(
        &client,
        url,
        Some(RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
        }),
    )
    .await
    .map_err(|e| format!("下载失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载返回错误状态: {}", response.status()));
    }

    let bytes = response.bytes().await.map_err(|e| format!("读取内容失败: {}", e))?;

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    std::fs::write(dest_path, bytes).map_err(|e| format!("写入文件失败: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn search_missing_dependency(
    mod_id: String,
    mc_version: String,
    loader: String,
) -> Result<serde_json::Value, String> {
    let (modrinth_result, curseforge_result) = search_mod_both_sources(&mod_id, &mc_version, &loader).await;

    Ok(serde_json::json!({
        "mod_id": mod_id,
        "modrinth": modrinth_result,
        "curseforge": curseforge_result,
    }))
}

#[tauri::command]
pub fn check_mod_installed(instance_dir: String, mod_id: String) -> Result<bool, String> {
    let mods_dir = Path::new(&instance_dir).join("mods");
    if !mods_dir.exists() {
        return Ok(false);
    }
    let mods = parse_mods_in_dir(mods_dir.to_string_lossy().to_string());
    let target = mod_id.to_ascii_lowercase();
    Ok(mods.iter().any(|m| m.mod_id.to_ascii_lowercase() == target))
}

enum InstallOutcome {
    Installed { path: String, file_name: String },
    AlreadyInstalled(String),
    Failed(String),
}

fn candidate_identity(result: &ModSearchResult) -> ModCandidateIdentity {
    ModCandidateIdentity {
        modrinth_project_id: if result.source == "modrinth" { result.project_id.clone() } else { None },
        names: vec![result.slug.clone()],
        sha1: result.file_sha1.clone(),
        file_name: result.file_name.clone(),
    }
}

fn already_installed_result(mod_id: &str, title: &str, reason: &str) -> DependencyDownloadResult {
    DependencyDownloadResult {
        mod_id: mod_id.to_string(),
        source: "local".to_string(),
        success: true,
        message: format!("模组已存在，无需下载: {}（{}）", title, reason),
        file_path: None,
    }
}

/// Installs `result` into `mods_dir` unless the same mod is already there.
///
/// The jar is saved under a hidden temporary name first (not `*.jar`, so the
/// game never loads it) and the mod ids it declares are compared with the
/// installed ones. That also catches a mod that is installed under another
/// slug or was downloaded from the other platform.
async fn install_mod_candidate(
    mods_dir: &Path,
    index: &mut InstalledModIndex,
    result: &ModSearchResult,
) -> InstallOutcome {
    let identity = candidate_identity(result);
    if let Some(reason) = index.find(&identity) {
        return InstallOutcome::AlreadyInstalled(reason);
    }
    let Some(url) = result.download_url.as_deref() else {
        return InstallOutcome::Failed("缺少下载地址".to_string());
    };
    let Some(file_name) = result
        .file_name
        .as_deref()
        .and_then(|name| Path::new(name).file_name())
        .and_then(|name| name.to_str())
        .map(str::to_owned)
    else {
        return InstallOutcome::Failed("缺少有效的文件名".to_string());
    };
    let final_path = mods_dir.join(&file_name);
    let temp_path = mods_dir.join(format!(".{}.rtl-part", file_name));
    if let Err(error) = download_file(url, &temp_path).await {
        let _ = std::fs::remove_file(&temp_path);
        return InstallOutcome::Failed(error);
    }

    let inspect_path = temp_path.clone();
    let (mod_ids, sha1) = tokio::task::spawn_blocking(move || {
        (mod_links::jar_mod_ids(&inspect_path), mod_links::file_sha1(&inspect_path))
    })
    .await
    .unwrap_or_default();
    let downloaded = ModCandidateIdentity {
        sha1: sha1.iter().cloned().collect(),
        ..Default::default()
    };
    let duplicate = index.find(&downloaded).or_else(|| {
        index
            .conflicting_mod_id(&mod_ids)
            .map(|mod_id| format!("已安装 mod id 为 {} 的模组", mod_id))
    });
    if let Some(reason) = duplicate {
        let _ = std::fs::remove_file(&temp_path);
        return InstallOutcome::AlreadyInstalled(reason);
    }
    if let Err(error) = std::fs::rename(&temp_path, &final_path) {
        let _ = std::fs::remove_file(&temp_path);
        return InstallOutcome::Failed(format!("保存文件失败: {}", error));
    }
    index.record(&identity);
    index.add_jar(&file_name, sha1.as_deref(), &mod_ids);
    InstallOutcome::Installed {
        path: final_path.to_string_lossy().to_string(),
        file_name,
    }
}

#[tauri::command]
pub async fn auto_download_missing_dependency(
    instance_dir: String,
    mod_id: String,
    mc_version: String,
    loader: String,
) -> Result<DependencyDownloadResult, String> {
    let mods_dir = Path::new(&instance_dir).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| format!("创建mods目录失败: {}", e))?;

    // 快速本地检查：如果已经安装该模组，直接返回成功（不发起网络请求）
    {
        let mods = parse_mods_in_dir(mods_dir.to_string_lossy().to_string());
        let target = mod_id.to_ascii_lowercase();
        if let Some(existing) = mods.iter().find(|m| m.mod_id.to_ascii_lowercase() == target) {
            return Ok(DependencyDownloadResult {
                mod_id: mod_id.clone(),
                source: "local".to_string(),
                success: true,
                message: format!("模组已存在，无需下载: {} v{}", existing.name, existing.version),
                file_path: None,
            });
        }
    }

    // 完整检查：provides / quilt.mod.json / 文件哈希 / Modrinth 项目
    let mut index = mod_links::load_installed_mod_index(mods_dir.clone()).await;
    let requested = ModCandidateIdentity {
        names: vec![mod_id.clone()],
        ..Default::default()
    };
    if let Some(reason) = index.find(&requested) {
        return Ok(already_installed_result(&mod_id, &mod_id, &reason));
    }

    let (modrinth_result, curseforge_result) = search_mod_both_sources(&mod_id, &mc_version, &loader).await;
    let found_any = modrinth_result.is_some() || curseforge_result.is_some();

    for search_result in [modrinth_result, curseforge_result].into_iter().flatten() {
        match install_mod_candidate(&mods_dir, &mut index, &search_result).await {
            InstallOutcome::Installed { path, file_name } => {
                return Ok(DependencyDownloadResult {
                    mod_id: mod_id.clone(),
                    source: search_result.source.clone(),
                    success: true,
                    message: format!("成功从 {} 下载: {}", search_result.source, file_name),
                    file_path: Some(path),
                });
            }
            InstallOutcome::AlreadyInstalled(reason) => {
                return Ok(already_installed_result(&mod_id, &search_result.title, &reason));
            }
            InstallOutcome::Failed(error) => {
                eprintln!("从 {} 下载 {} 失败: {}", search_result.source, mod_id, error);
            }
        }
    }

    if !found_any {
        return Ok(DependencyDownloadResult {
            mod_id: mod_id.clone(),
            source: "none".to_string(),
            success: false,
            message: format!("在 Modrinth 和 CurseForge 上都未找到模组 '{}'", mod_id),
            file_path: None,
        });
    }

    Ok(DependencyDownloadResult {
        mod_id: mod_id.clone(),
        source: "both_failed".to_string(),
        success: false,
        message: format!("模组 '{}' 找到但下载失败，请检查网络", mod_id),
        file_path: None,
    })
}

#[tauri::command]
pub async fn auto_download_all_missing_dependencies(
    instance_dir: String,
    mc_version: String,
    loader: String,
    missing_deps: Vec<String>,
) -> Result<Vec<DependencyDownloadResult>, String> {
    let mut results = Vec::new();

    let sem = Arc::new(tokio::sync::Semaphore::new(2));
    let mut tasks = Vec::new();

    let instance_dir_arc = Arc::new(instance_dir);
    let mc_version_arc = Arc::new(mc_version);
    let loader_arc = Arc::new(loader);

    for dep_mod_id in missing_deps {
        let permit = sem.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let instance_dir_clone = Arc::clone(&instance_dir_arc);
        let mc_version_clone = Arc::clone(&mc_version_arc);
        let loader_clone = Arc::clone(&loader_arc);
        let mod_id_clone = dep_mod_id.clone();

        let task = tokio::spawn(async move {
            let _permit = permit;
            let result = auto_download_missing_dependency(
                instance_dir_clone.to_string(),
                mod_id_clone,
                mc_version_clone.to_string(),
                loader_clone.to_string(),
            ).await.unwrap_or_else(|e| DependencyDownloadResult {
                mod_id: dep_mod_id,
                source: "error".to_string(),
                success: false,
                message: e,
                file_path: None,
            });
            result
        });

        tasks.push(task);
    }

    let task_results = join_all(tasks).await;
    for task_result in task_results {
        match task_result {
            Ok(result) => results.push(result),
            Err(e) => results.push(DependencyDownloadResult {
                mod_id: "unknown".to_string(),
                source: "error".to_string(),
                success: false,
                message: format!("任务执行失败: {}", e),
                file_path: None,
            }),
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDependency {
    pub mod_id: String,
    pub slug: Option<String>,
    pub dependency_type: String,
    pub version_range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrinth_project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge_mod_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModWithDeps {
    pub search_result: ModSearchResult,
    pub dependencies: Vec<ApiDependency>,
}

async fn modrinth_version_project(client: &reqwest::Client, version_id: &str) -> Option<String> {
    let url = format!("https://api.modrinth.com/v2/version/{}", urlencoding::encode(version_id));
    let response = get_with_retry(
        client,
        &url,
        Some(RetryConfig { max_retries: 1, initial_delay_ms: 300, max_delay_ms: 1000 }),
    ).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().await.ok()?;
    json.get("project_id")?.as_str().map(str::to_owned)
}

async fn modrinth_project_slugs(
    client: &reqwest::Client,
    project_ids: &[String],
) -> std::collections::HashMap<String, String> {
    let mut slugs = std::collections::HashMap::new();
    if project_ids.is_empty() {
        return slugs;
    }
    let url = format!(
        "https://api.modrinth.com/v2/projects?ids={}",
        urlencoding::encode(&serde_json::json!(project_ids).to_string())
    );
    let Ok(response) = get_with_retry(
        client,
        &url,
        Some(RetryConfig { max_retries: 1, initial_delay_ms: 300, max_delay_ms: 1000 }),
    ).await else {
        return slugs;
    };
    if let Ok(projects) = response.json::<serde_json::Value>().await {
        for project in projects.as_array().into_iter().flatten() {
            if let (Some(id), Some(slug)) = (
                project.get("id").and_then(|v| v.as_str()),
                project.get("slug").and_then(|v| v.as_str()),
            ) {
                slugs.insert(id.to_string(), slug.to_string());
            }
        }
    }
    slugs
}

/// Dependencies declared by a Modrinth version, with their project ids.
async fn fetch_modrinth_version_dependencies(version: &serde_json::Value) -> Vec<ApiDependency> {
    let client = modrinth_client().await;
    let mut pending: Vec<(String, String, Option<String>)> = Vec::new();

    for dep in version.get("dependencies").and_then(|d| d.as_array()).into_iter().flatten() {
        let dep_type = dep.get("dependency_type")
            .and_then(|t| t.as_str())
            .unwrap_or("required")
            .to_string();
        if dep_type != "required" && dep_type != "optional" && dep_type != "incompatible" {
            continue;
        }
        let version_range = dep.get("version_range").and_then(|v| v.as_str()).map(|s| s.to_string());
        let project_id = match dep.get("project_id").and_then(|p| p.as_str()) {
            Some(project_id) => Some(project_id.to_string()),
            // Version-only references: find the project the version belongs to.
            None => match dep.get("version_id").and_then(|v| v.as_str()) {
                Some(version_id) => modrinth_version_project(&client, version_id).await,
                None => None,
            },
        };
        match project_id {
            Some(project_id) => pending.push((project_id, dep_type, version_range)),
            None => eprintln!("[ModDeps] 无法确定 Modrinth 依赖所属项目: {}", dep),
        }
    }

    let project_ids: Vec<String> = pending.iter().map(|(id, _, _)| id.clone()).collect();
    let slugs = modrinth_project_slugs(&client, &project_ids).await;
    pending
        .into_iter()
        .map(|(project_id, dependency_type, version_range)| {
            let slug = slugs.get(&project_id).cloned();
            ApiDependency {
                mod_id: slug.clone().unwrap_or_else(|| project_id.clone()),
                slug,
                dependency_type,
                version_range,
                modrinth_project_id: Some(project_id),
                curseforge_mod_id: None,
            }
        })
        .collect()
}

/// CurseForge `FileRelationType`: 1 embedded library, 2 optional, 3 required,
/// 4 tool, 5 incompatible, 6 include.
fn curseforge_relation_kind(relation_type: u64) -> Option<&'static str> {
    match relation_type {
        3 => Some("required"),
        2 => Some("optional"),
        5 => Some("incompatible"),
        _ => None,
    }
}

/// Dependencies declared by a CurseForge file, with their mod ids.
async fn fetch_curseforge_mod_dependencies(file: &serde_json::Value) -> Vec<ApiDependency> {
    let client = curseforge_client().await;
    let mut deps = Vec::new();

    for dep in file.get("dependencies").and_then(|d| d.as_array()).into_iter().flatten() {
        let Some(dep_type) = dep
            .get("relationType")
            .and_then(|r| r.as_u64())
            .and_then(curseforge_relation_kind)
        else {
            continue;
        };
        let Some(dep_mod_id) = dep.get("modId").and_then(|m| m.as_i64()) else {
            continue;
        };
        let fallback = dep_mod_id.to_string();
        let (slug, name) = mod_links::get_curseforge_project_identity(&client, dep_mod_id).await;
        let slug = (slug != fallback).then_some(slug);
        let name = (name != fallback).then_some(name);
        deps.push(ApiDependency {
            mod_id: slug.clone().or(name).unwrap_or_else(|| format!("curseforge_{}", dep_mod_id)),
            slug,
            dependency_type: dep_type.to_string(),
            version_range: None,
            modrinth_project_id: None,
            curseforge_mod_id: Some(dep_mod_id),
        });
    }

    deps
}

async fn modrinth_with_deps(
    project: &str,
    identity: Option<(String, String)>,
    query: &str,
    mc_version: &str,
    loader: &str,
) -> Option<ModWithDeps> {
    let (search_result, version) = modrinth_candidate(project, identity, query, mc_version, loader).await?;
    let dependencies = fetch_modrinth_version_dependencies(&version).await;
    Some(ModWithDeps { search_result, dependencies })
}

async fn search_modrinth_with_deps(query: &str, mc_version: &str, loader: &str) -> Option<ModWithDeps> {
    let (slug, title) = search_modrinth_project(query, mc_version, loader).await?;
    let project = slug.clone();
    modrinth_with_deps(&project, Some((slug, title)), query, mc_version, loader).await
}

async fn curseforge_with_deps(
    mod_id: i64,
    identity: Option<(String, String)>,
    query: &str,
    mc_version: &str,
    loader: &str,
) -> Option<ModWithDeps> {
    let (search_result, file) = curseforge_candidate(mod_id, identity, query, mc_version, loader).await?;
    let dependencies = fetch_curseforge_mod_dependencies(&file).await;
    Some(ModWithDeps { search_result, dependencies })
}

async fn search_curseforge_with_deps(query: &str, mc_version: &str, loader: &str) -> Option<ModWithDeps> {
    for (mod_id, slug, name) in search_curseforge_projects(query, mc_version, loader).await {
        let title = if name.is_empty() { slug.clone() } else { name };
        if let Some(found) = curseforge_with_deps(mod_id, Some((slug, title)), query, mc_version, loader).await {
            return Some(found);
        }
    }
    None
}

/// A mod to install while completing dependencies.
#[derive(Debug, Clone)]
struct DependencyTarget {
    /// Mod id or project slug: shown in results and used for name search.
    query: String,
    modrinth_project_id: Option<String>,
    curseforge_mod_id: Option<i64>,
    depth: usize,
}

impl DependencyTarget {
    fn root(query: String) -> Self {
        Self { query, modrinth_project_id: None, curseforge_mod_id: None, depth: 0 }
    }

    fn required_by(dependency: &ApiDependency, depth: usize) -> Self {
        Self {
            query: dependency.slug.clone().unwrap_or_else(|| dependency.mod_id.clone()),
            modrinth_project_id: dependency.modrinth_project_id.clone(),
            curseforge_mod_id: dependency.curseforge_mod_id,
            depth,
        }
    }

    /// Visit keys known before the target is resolved.
    fn keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let Some(project_id) = &self.modrinth_project_id {
            keys.push(format!("modrinth:{}", project_id));
        }
        if let Some(mod_id) = self.curseforge_mod_id {
            keys.push(format!("curseforge:{}", mod_id));
        }
        if keys.is_empty() {
            keys.push(format!("name:{}", mod_links::normalize_mod_key(&self.query)));
        }
        keys
    }

    fn identity(&self) -> ModCandidateIdentity {
        ModCandidateIdentity {
            modrinth_project_id: self.modrinth_project_id.clone(),
            names: vec![self.query.clone()],
            ..Default::default()
        }
    }

    /// Whether the query is only an id, i.e. the project slug is unknown.
    fn query_is_id(&self) -> bool {
        self.modrinth_project_id.as_deref() == Some(self.query.as_str())
            || self.curseforge_mod_id.map(|id| id.to_string()).as_deref() == Some(self.query.as_str())
    }
}

/// Visit keys of a resolved candidate: its project on its platform and its
/// slug, so a mod reached through another platform or reference is handled once.
fn candidate_keys(result: &ModSearchResult) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(project_id) = &result.project_id {
        keys.push(format!("{}:{}", result.source, project_id));
    }
    let slug_key = mod_links::normalize_mod_key(&result.slug);
    if !slug_key.is_empty() {
        keys.push(format!("name:{}", slug_key));
    }
    keys
}

/// Marks `keys` as visited. Returns false, without marking anything, when one
/// of them other than those in `own` was visited before.
fn mark_visited(visited: &mut HashSet<String>, keys: &[String], own: &[String]) -> bool {
    if keys.iter().any(|key| !own.contains(key) && visited.contains(key)) {
        return false;
    }
    visited.extend(keys.iter().cloned());
    true
}

/// Candidates for a target, best first. Dependencies with a known project are
/// fetched directly from their platform; a name search on both platforms is the
/// fallback, and for dependencies it only accepts an exact slug match.
async fn resolve_dependency_candidates(target: &DependencyTarget, mc_version: &str, loader: &str) -> Vec<ModWithDeps> {
    let direct = if let Some(project_id) = &target.modrinth_project_id {
        modrinth_with_deps(project_id, None, &target.query, mc_version, loader).await
    } else if let Some(mod_id) = target.curseforge_mod_id {
        curseforge_with_deps(mod_id, None, &target.query, mc_version, loader).await
    } else {
        None
    };
    if let Some(candidate) = direct {
        return vec![candidate];
    }
    let has_project = target.modrinth_project_id.is_some() || target.curseforge_mod_id.is_some();
    if has_project && target.query_is_id() {
        return Vec::new();
    }

    let (modrinth, curseforge) = tokio::join!(
        search_modrinth_with_deps(&target.query, mc_version, loader),
        search_curseforge_with_deps(&target.query, mc_version, loader)
    );
    let wanted = mod_links::normalize_mod_key(&target.query);
    modrinth
        .into_iter()
        .chain(curseforge)
        .filter(|candidate| !has_project || mod_links::normalize_mod_key(&candidate.search_result.slug) == wanted)
        .collect()
}

#[tauri::command]
pub async fn auto_download_with_dependencies(
    instance_dir: String,
    mod_id: String,
    mc_version: String,
    loader: String,
) -> Result<Vec<DependencyDownloadResult>, String> {
    const MAX_DEPTH: usize = 5;
    const MAX_TARGETS: usize = 32;

    let mods_dir = Path::new(&instance_dir).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| format!("创建mods目录失败: {}", e))?;

    // Everything already in mods/ (hash, mod id, Modrinth project) is treated as
    // installed: a second copy of a mod crashes Forge / NeoForge on startup.
    let mut index = mod_links::load_installed_mod_index(mods_dir.clone()).await;
    let mut results = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<DependencyTarget> = VecDeque::from([DependencyTarget::root(mod_id)]);
    let mut processed = 0;

    while let Some(target) = queue.pop_front() {
        if target.depth > MAX_DEPTH {
            continue;
        }
        if processed >= MAX_TARGETS {
            break;
        }
        let target_keys = target.keys();
        if !mark_visited(&mut visited, &target_keys, &[]) {
            continue;
        }
        processed += 1;

        if let Some(reason) = index.find(&target.identity()) {
            results.push(already_installed_result(&target.query, &target.query, &reason));
            continue;
        }

        let mut handled = false;
        let mut last_error: Option<String> = None;
        for candidate in resolve_dependency_candidates(&target, &mc_version, &loader).await {
            // The same project reached through another reference or platform.
            if !mark_visited(&mut visited, &candidate_keys(&candidate.search_result), &target_keys) {
                handled = true;
                break;
            }
            let source_name = candidate.search_result.source.clone();
            match install_mod_candidate(&mods_dir, &mut index, &candidate.search_result).await {
                InstallOutcome::Installed { path, file_name } => {
                    results.push(DependencyDownloadResult {
                        mod_id: target.query.clone(),
                        source: source_name.clone(),
                        success: true,
                        message: format!("从 {} 下载成功: {}", source_name, file_name),
                        file_path: Some(path),
                    });
                    for dep in &candidate.dependencies {
                        if dep.dependency_type == "required" && !is_platform_dep(&dep.mod_id) {
                            queue.push_back(DependencyTarget::required_by(dep, target.depth + 1));
                        }
                    }
                    handled = true;
                    break;
                }
                InstallOutcome::AlreadyInstalled(reason) => {
                    results.push(already_installed_result(
                        &target.query,
                        &candidate.search_result.title,
                        &reason,
                    ));
                    handled = true;
                    break;
                }
                InstallOutcome::Failed(error) => {
                    eprintln!("从 {} 下载 {} 失败: {}", source_name, target.query, error);
                    last_error = Some(error);
                }
            }
        }

        if !handled {
            let detail = last_error.map(|error| format!("：{}", error)).unwrap_or_default();
            results.push(DependencyDownloadResult {
                mod_id: target.query.clone(),
                source: "not_found".to_string(),
                success: false,
                message: format!("模组 '{}' 在双平台均未找到或下载失败{}", target.query, detail),
                file_path: None,
            });
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderDependencyError {
    pub error_type: String,
    pub mod_id: String,
    pub required_version: Option<String>,
    pub required_by: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderLogReport {
    pub has_crash: bool,
    pub errors: Vec<LoaderDependencyError>,
    pub summary: String,
}

fn is_platform_dep(mod_id: &str) -> bool {
    matches!(
        mod_id.to_ascii_lowercase().as_str(),
        "minecraft" | "forge" | "neoforge" | "fabricloader" | "fabric" | "fabric-api"
            | "quilt_loader" | "java" | "liteloader" | "minecraftforge" | "fabric-api-base"
            | "fabric-api-lookup-api-v1" | "fabric-biome-api-v1" | "fabric-block-api-v1"
            | "fabric-blockrenderlayer-v1" | "fabric-command-api-v1" | "fabric-command-api-v2"
            | "fabric-content-registries-v0" | "fabric-crash-report-info-v1"
            | "fabric-data-attachment-api-v1" | "fabric-dimensions-v1" | "fabric-entity-events-v1"
            | "fabric-events-interaction-v0" | "fabric-events-lifecycle-v0"
            | "fabric-game-rule-api-v1" | "fabric-gametest-api-v1" | "fabric-item-api-v1"
            | "fabric-item-groups-v0" | "fabric-key-binding-api-v1" | "fabric-lifecycle-events-v1"
            | "fabric-loot-api-v2" | "fabric-message-api-v1" | "fabric-mining-level-api-v1"
            | "fabric-model-loading-api-v1" | "fabric-models-v0" | "fabric-networking-api-v1"
            | "fabric-networking-v0" | "fabric-object-builder-api-v1" | "fabric-particles-v1"
            | "fabric-recipe-api-v1" | "fabric-registry-sync-v0" | "fabric-renderer-api-v1"
            | "fabric-renderer-indigo" | "fabric-rendering-fluids-v1" | "fabric-rendering-v0"
            | "fabric-rendering-v1" | "fabric-resource-loader-v0" | "fabric-screen-api-v1"
            | "fabric-screen-handler-api-v1" | "fabric-sound-api-v1" | "fabric-textures-v0"
            | "fabric-transfer-api-v1" | "fabric-transitive-access-wideners-v1"
            | "forgeconfigapiport" | "kotlinforforge" | "architectury"
            | "mixinextras" | "jni" | "com_github_llamalad7_mixinextras"
    )
}

#[tauri::command]
pub async fn analyze_loader_logs(instance_dir: String) -> Result<LoaderLogReport, String> {
    let inst_path = Path::new(&instance_dir);
    let logs_dir = inst_path.join("logs");
    let run_log = logs_dir.join("latest.log");
    let crash_dir = inst_path.join("crash-reports");

    let mut all_errors: Vec<LoaderDependencyError> = Vec::new();

    let mut try_files: Vec<PathBuf> = Vec::new();
    if run_log.exists() { try_files.push(run_log.clone()); }

    if crash_dir.exists() {
        if let Ok(mut entries) = std::fs::read_dir(&crash_dir) {
            let mut crash_files: Vec<PathBuf> = Vec::new();
            while let Some(Ok(entry)) = entries.next() {
                crash_files.push(entry.path());
            }
            crash_files.sort_by(|a, b| {
                let ta = a.metadata().and_then(|m| m.modified()).ok();
                let tb = b.metadata().and_then(|m| m.modified()).ok();
                tb.cmp(&ta)
            });
            for f in crash_files.into_iter().take(3) {
                try_files.push(f);
            }
        }
    }

    if let Ok(latestlog) = std::fs::read_to_string(&run_log) {
        parse_loader_errors(&latestlog, &mut all_errors);
    }

    for log_file in try_files.iter().skip(1) {
        if let Ok(content) = std::fs::read_to_string(log_file) {
            parse_loader_errors(&content, &mut all_errors);
        }
    }

    let has_crash = !all_errors.is_empty();

    let mut summary_parts: Vec<String> = Vec::new();
    let missing_count = all_errors.iter().filter(|e| e.error_type == "missing").count();
    let mismatch_count = all_errors.iter().filter(|e| e.error_type == "version_mismatch").count();
    let incompatible_count = all_errors.iter().filter(|e| e.error_type == "incompatible").count();

    if missing_count > 0 {
        summary_parts.push(format!("缺失 {} 个必需模组", missing_count));
    }
    if mismatch_count > 0 {
        summary_parts.push(format!("{} 个模组版本不匹配", mismatch_count));
    }
    if incompatible_count > 0 {
        summary_parts.push(format!("{} 个模组不兼容", incompatible_count));
    }

    let summary = if summary_parts.is_empty() {
        if has_crash {
            "检测到启动错误，但无法解析具体依赖问题".to_string()
        } else {
            "未检测到 ModLoader 依赖错误".to_string()
        }
    } else {
        summary_parts.join("；")
    };

    Ok(LoaderLogReport { has_crash, errors: all_errors, summary })
}

fn parse_loader_errors(log_content: &str, out_errors: &mut Vec<LoaderDependencyError>) {
    let mut seen = std::collections::HashSet::new();

    for raw_line in log_content.lines() {
        let line = raw_line.trim();

        if line.contains("MissingModsException") || line.contains("ModLoadingException") || line.contains("errors occurred during event") {
            continue;
        }

        if line.contains("net.minecraftforge.fml.MissingModsException") || line.contains("Mods require missing mods") {
            parse_forge_missing_mods_block(log_content, out_errors, &mut seen);
        }

        let forge_pat = regex_parse(
            r"Mod\s+'([^']+)'\s+requires?\s+(\w+mod|forge|mod|mods)\s+(.+?)\s+version\s+(.+?),\s+but\s+only\s+wrong\s+version\s+(.+?)\s+is\s+installed",
            line,
        );
        if let Some(caps) = forge_pat {
            let err = LoaderDependencyError {
                error_type: "version_mismatch".to_string(),
                mod_id: caps.get(2).cloned().unwrap_or_default(),
                required_version: caps.get(4).cloned(),
                required_by: caps.get(1).cloned(),
                message: line.to_string(),
            };
            let key = format!("{}:{}", err.error_type, err.mod_id);
            if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
            continue;
        }

        let forge_missing = regex_parse(
            r"^\s*-\s+Mod\s+`?([a-zA-Z0-9_.-]+)`?:\s*(.+)`,?\s*required\s+by\s+`?([a-zA-Z0-9_.-]+)`?",
            line,
        );
        if let Some(caps) = forge_missing {
            let err = LoaderDependencyError {
                error_type: "missing".to_string(),
                mod_id: caps.get(1).cloned().unwrap_or_default(),
                required_version: caps.get(2).cloned().and_then(|v| {
                    let v = v.trim_matches(|c| c == '`' || c == '\'' || c == ' ');
                    if v.is_empty() { None } else { Some(v.to_string()) }
                }),
                required_by: caps.get(3).cloned(),
                message: line.to_string(),
            };
            if !err.mod_id.is_empty() {
                let key = format!("{}:{}", err.error_type, err.mod_id);
                if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
            }
            continue;
        }

        let forge_line = regex_parse(
            r"^-\s+`?([a-zA-Z0-9_.-]+)`?\s+any\s+version\s+.{0,40}required\s+by\s+`?([a-zA-Z0-9_.-]+)`?",
            line,
        );
        if let Some(caps) = forge_line {
            let err = LoaderDependencyError {
                error_type: "missing".to_string(),
                mod_id: caps.get(1).cloned().unwrap_or_default(),
                required_version: None,
                required_by: caps.get(2).cloned(),
                message: line.to_string(),
            };
            if !err.mod_id.is_empty() {
                let key = format!("{}:{}", err.error_type, err.mod_id);
                if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
            }
            continue;
        }

        let fabric_alt = regex_parse(
            r"Mod\s+`([a-z0-9_.-]+)`\s+requires\s+any\s+version\s+of\s+`([a-z0-9_.-]+)`,\s+which\s+is\s+missing",
            line,
        );
        if let Some(caps) = fabric_alt {
            let err = LoaderDependencyError {
                error_type: "missing".to_string(),
                mod_id: caps.get(2).cloned().unwrap_or_default(),
                required_version: None,
                required_by: caps.get(1).cloned(),
                message: line.to_string(),
            };
            let key = format!("{}:{}", err.error_type, err.mod_id);
            if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
            continue;
        }

        let fabric_ver = regex_parse(
            r"Mod\s+`([a-z0-9_.-]+)`\s+requires\s+version\s+(.+?)\s+of\s+`([a-z0-9_.-]+)`,\s+which\s+is\s+missing|only\s+version\s+(.+?)\s+is\s+present",
            line,
        );
        if let Some(caps) = fabric_ver {
            let mod_id = caps.get(3).or_else(|| caps.get(1)).cloned().unwrap_or_default();
            let ver = caps.get(2).or_else(|| caps.get(4)).cloned();
            let err = LoaderDependencyError {
                error_type: if line.contains("is missing") { "missing".to_string() } else { "version_mismatch".to_string() },
                mod_id,
                required_version: ver,
                required_by: caps.get(1).cloned(),
                message: line.to_string(),
            };
            if !err.mod_id.is_empty() {
                let key = format!("{}:{}", err.error_type, err.mod_id);
                if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
            }
            continue;
        }

        if line.contains("Conflicting") || line.contains("conflicts with") || line.contains("incompatible with") {
            let incomp = regex_parse(
                r"Mod\s+`?([a-zA-Z0-9_.-]+)`?\s+(?:conflicts with|incompatible with|is incompatible)\s+`?([a-zA-Z0-9_.-]+)`?",
                line,
            );
            if let Some(caps) = incomp {
                let err = LoaderDependencyError {
                    error_type: "incompatible".to_string(),
                    mod_id: caps.get(2).cloned().unwrap_or_default(),
                    required_version: None,
                    required_by: caps.get(1).cloned(),
                    message: line.to_string(),
                };
                if !err.mod_id.is_empty() {
                    let key = format!("{}:{}", err.error_type, err.mod_id);
                    if !seen.contains(&key) { seen.insert(key); out_errors.push(err); }
                }
            }
        }
    }
}

fn regex_parse(pattern: &str, text: &str) -> Option<Vec<String>> {
    use regex::Regex;
    let re = Regex::new(pattern).ok()?;
    re.captures(text).map(|caps| {
        caps.iter().skip(1).map(|m| m.map(|x| x.as_str().to_string()).unwrap_or_default()).collect()
    })
}

fn parse_forge_missing_mods_block(log: &str, out: &mut Vec<LoaderDependencyError>, seen: &mut std::collections::HashSet<String>) {
    let mut in_block = false;
    for line in log.lines() {
        let trimmed = line.trim();
        if trimmed.contains("MissingModsException") {
            in_block = true;
            continue;
        }
        if in_block {
            if trimmed.is_empty() || (trimmed.contains("at ") && trimmed.contains('(')) {
                break;
            }

            let mod_extract = regex_parse(r"`([a-zA-Z0-9_.-]+)`", trimmed);
            if let Some(caps) = mod_extract {
                if !caps.is_empty() {
                    for mid in caps.iter() {
                        if !mid.is_empty() && !is_platform_dep(mid) {
                            let key = format!("missing:{}", mid);
                            if !seen.contains(&key) {
                                seen.insert(key.clone());
                                out.push(LoaderDependencyError {
                                    error_type: "missing".to_string(),
                                    mod_id: mid.clone(),
                                    required_version: None,
                                    required_by: None,
                                    message: trimmed.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub async fn deep_analyze_with_api(
    instance_dir: String,
    mc_version: String,
    loader: String,
) -> Result<ModDependenciesAnalysis, String> {
    let path = Path::new(&instance_dir);
    let mut analysis = analyze_mod_dependencies(path);

    let mods_dir = path.join("mods");
    if !mods_dir.exists() { return Ok(analysis); }

    let mods = parse_mods_in_dir(mods_dir.to_string_lossy().to_string());

    let mut api_missing: Vec<String> = Vec::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(3));
    let mut tasks = Vec::new();

    for mod_info in mods.iter().take(20) {
        if is_platform_dep(&mod_info.mod_id) {
            continue;
        }
        let permit = sem.clone().acquire_owned().await.ok();
        let mc = mc_version.clone();
        let ld = loader.clone();
        let mid = mod_info.mod_id.clone();
        let name = mod_info.name.clone();

        let task = tokio::spawn(async move {
            let _p = permit;
            let (mr, cf) = tokio::join!(
                search_modrinth_with_deps(&mid, &mc, &ld),
                search_curseforge_with_deps(&mid, &mc, &ld)
            );
            let deps_list: Vec<ApiDependency> = mr
                .map(|m| m.dependencies)
                .or_else(|| cf.map(|c| c.dependencies))
                .unwrap_or_default();
            (mid, name, deps_list)
        });
        tasks.push(task);
    }

    let task_results = join_all(tasks).await;
    let mut api_required_map: std::collections::HashMap<String, Vec<(String, String)>> = std::collections::HashMap::new();

    for tr in task_results {
        if let Ok((mod_id, _name, deps)) = tr {
            for dep in deps {
                if dep.dependency_type == "required" && !is_platform_dep(&dep.mod_id) {
                    api_required_map
                        .entry(dep.mod_id.clone())
                        .or_default()
                        .push((mod_id.clone(), dep.version_range.clone().unwrap_or_default()));
                }
            }
        }
    }

    let installed_ids: std::collections::HashSet<String> = mods.iter().map(|m| m.mod_id.clone()).collect();

    for (dep_id, requesters) in &api_required_map {
        if !installed_ids.contains(dep_id) {
            let required_by: Vec<String> = requesters.iter().map(|(r, _)| r.clone()).collect();
            let recommended = requesters.iter().find(|(_, v)| !v.is_empty()).map(|(_, v)| v.clone());

            if !analysis.missing_dependencies.iter().any(|m| m.mod_id == *dep_id) {
                analysis.missing_dependencies.push(ModDependencyIssue {
                    mod_id: dep_id.clone(),
                    mod_name: dep_id.clone(),
                    issue_type: "missing".to_string(),
                    required_by,
                    version: None,
                    recommended_version: recommended,
                });
            }
            api_missing.push(dep_id.clone());
        }
    }

    analysis.all_resolved = analysis.missing_dependencies.is_empty()
        && analysis.version_mismatches.is_empty()
        && analysis.incompatible_mods.is_empty();

    Ok(analysis)
}

fn format_timestamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let (year, month, day) = days_from_epoch(days_since_epoch);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!("{:04}{:02}{:02}_{:02}{:02}{:02}", year, month, day, hour, minute, second)
}

fn days_from_epoch(days: u64) -> (i32, u32, u32) {
    let mut y: i32 = 1970;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining < days_in_year as i64 {
            break;
        }
        remaining -= days_in_year as i64;
        y += 1;
    }
    let mdays: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m: u32 = 1;
    loop {
        let dim = if m == 2 && is_leap_year(y) { 29 } else { mdays[(m - 1) as usize] };
        if remaining < dim {
            break;
        }
        remaining -= dim;
        m += 1;
    }
    (y, m, (remaining + 1) as u32)
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

#[tauri::command]
pub async fn export_launch_report(
    minecraft_path: String,
    version_name: String,
    launch_parameters: String,
    account_type: String,
    report_json: String,
    console_logs: String,
) -> Result<String, String> {
    let mc_path = PathBuf::from(&minecraft_path);

    let export_dir = mc_path.join("crash-reports");
    std::fs::create_dir_all(&export_dir).map_err(|e| format!("创建导出目录失败: {}", e))?;

    let timestamp = format_timestamp();
    let safe_version = version_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let zip_filename = format!("launch_report_{}_{}.zip", safe_version, timestamp);
    let zip_path = export_dir.join(&zip_filename);

    let file = std::fs::File::create(&zip_path).map_err(|e| format!("创建zip文件失败: {}", e))?;
    let mut zip = ZipWriter::new(file);

    let report_data = report_json.as_bytes();
    zip.start_file(
        "report.json",
        FileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .map_err(|e| format!("添加report.json失败: {}", e))?;
    zip.write_all(report_data)
        .map_err(|e| format!("写入report.json失败: {}", e))?;

    let params_content = format!(
        "版本: {}\n账户类型: {}\n启动参数:\n{}",
        version_name, account_type, launch_parameters
    );
    zip.start_file(
        "launch_parameters.txt",
        FileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .map_err(|e| format!("添加launch_parameters.txt失败: {}", e))?;
    zip.write_all(params_content.as_bytes())
        .map_err(|e| format!("写入launch_parameters.txt失败: {}", e))?;

    let logs_dir = mc_path.join("logs");
    let latest_log = logs_dir.join("latest.log");
    if latest_log.exists() {
        if let Ok(log_content) = std::fs::read_to_string(&latest_log) {
            zip.start_file(
                "latest.log",
                FileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .map_err(|e| format!("添加latest.log失败: {}", e))?;
            zip.write_all(log_content.as_bytes())
                .map_err(|e| format!("写入latest.log失败: {}", e))?;
        }
    }

    if !console_logs.trim().is_empty() {
        zip.start_file(
            "console.log",
            FileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .map_err(|e| format!("添加console.log失败: {}", e))?;
        zip.write_all(console_logs.as_bytes())
            .map_err(|e| format!("写入console.log失败: {}", e))?;
    }

    let result = zip.finish();
    if let Err(e) = result {
        return Err(format!("完成zip失败: {}", e));
    }

    Ok(zip_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod dependency_install_tests {
    use super::*;
    use serde_json::json;
    use std::io::Read;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn jar_bytes(fabric_mod_json: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        writer.start_file("fabric.mod.json", options).unwrap();
        writer.write_all(fabric_mod_json.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// Minimal HTTP server on 127.0.0.1 serving `files` by path.
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> (String, Arc<AtomicUsize>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                }
                counter.fetch_add(1, Ordering::SeqCst);
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = files.iter().find(|(p, _)| *p == path).map(|(_, b)| b.clone());
                let (status, body) = match body {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", Vec::new()),
                };
                let head = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: application/java-archive\r\nConnection: close\r\n\r\n",
                    status,
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        (base, hits)
    }

    fn temp_mods_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rtl-dep-install-{}-{}-{}", name, std::process::id(), nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn search_result(slug: &str, url: String, file_name: &str) -> ModSearchResult {
        ModSearchResult {
            source: "modrinth".to_string(),
            mod_id: slug.to_string(),
            slug: slug.to_string(),
            title: slug.to_string(),
            download_url: Some(url),
            file_name: Some(file_name.to_string()),
            mc_versions: vec!["1.21.1".to_string()],
            loaders: vec!["fabric".to_string()],
            version_number: None,
            project_id: Some(format!("project-{}", slug)),
            file_sha1: Vec::new(),
        }
    }

    fn jar_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn installs_missing_mod_and_never_adds_a_second_copy() {
        let sodium_json = r#"{"schemaVersion":1,"id":"sodium","version":"0.8.13+mc1.21.1"}"#;
        let (base, hits) = serve(vec![
            ("/sodium-beta.jar", jar_bytes(r#"{"schemaVersion":1,"id":"sodium","version":"0.8.14-beta.2"}"#)),
            ("/sodium-extra.jar", jar_bytes(r#"{"schemaVersion":1,"id":"sodium-extra","version":"0.9.5"}"#)),
        ]);
        let mods_dir = temp_mods_dir("install");
        std::fs::write(mods_dir.join("sodium-fabric-0.8.13+mc1.21.1.jar"), jar_bytes(sodium_json)).unwrap();
        let mut index = InstalledModIndex::scan(&mods_dir);

        // Same project slug: recognised before anything is downloaded.
        let sodium = search_result("sodium", format!("{}/sodium-beta.jar", base), "sodium-fabric-0.8.14-beta.2+mc1.21.1.jar");
        assert!(matches!(
            install_mod_candidate(&mods_dir, &mut index, &sodium).await,
            InstallOutcome::AlreadyInstalled(_)
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        // Unknown slug, but the downloaded jar declares the installed mod id:
        // the temporary file is discarded instead of becoming a duplicate.
        let mirror = search_result("sodium-mirror", format!("{}/sodium-beta.jar", base), "sodium-mirror.jar");
        match install_mod_candidate(&mods_dir, &mut index, &mirror).await {
            InstallOutcome::AlreadyInstalled(reason) => assert!(reason.contains("sodium"), "{}", reason),
            InstallOutcome::Installed { .. } => panic!("duplicate sodium was installed"),
            InstallOutcome::Failed(error) => panic!("unexpected failure: {}", error),
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(jar_names(&mods_dir), vec!["sodium-fabric-0.8.13+mc1.21.1.jar"]);

        // A missing mod is installed, then recognised on the next attempt.
        let extra = search_result("sodium-extra", format!("{}/sodium-extra.jar", base), "sodium-extra-fabric-0.9.5+mc1.21.1.jar");
        assert!(matches!(
            install_mod_candidate(&mods_dir, &mut index, &extra).await,
            InstallOutcome::Installed { .. }
        ));
        assert_eq!(
            jar_names(&mods_dir),
            vec!["sodium-extra-fabric-0.9.5+mc1.21.1.jar", "sodium-fabric-0.8.13+mc1.21.1.jar"]
        );
        assert!(matches!(
            install_mod_candidate(&mods_dir, &mut index, &extra).await,
            InstallOutcome::AlreadyInstalled(_)
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 2);

        let _ = std::fs::remove_dir_all(&mods_dir);
    }

    #[test]
    fn search_prefers_exact_slug_or_title() {
        let hits = vec![
            json!({"slug": "sodium-extra", "title": "Sodium Extra"}),
            json!({"slug": "sodium", "title": "Sodium"}),
        ];
        assert_eq!(pick_search_hit(&hits, "sodium", "slug", "title").unwrap()["slug"], "sodium");
        assert_eq!(pick_search_hit(&hits, "Sodium Extra", "slug", "title").unwrap()["slug"], "sodium-extra");
        // Without an exact match the relevance order is kept.
        assert_eq!(pick_search_hit(&hits, "rubidium", "slug", "title").unwrap()["slug"], "sodium-extra");
    }

    #[test]
    fn curseforge_relation_types_follow_the_api() {
        assert_eq!(curseforge_relation_kind(3), Some("required"));
        assert_eq!(curseforge_relation_kind(2), Some("optional"));
        assert_eq!(curseforge_relation_kind(5), Some("incompatible"));
        // Embedded libraries, tools and includes are not dependencies to install.
        assert_eq!(curseforge_relation_kind(1), None);
        assert_eq!(curseforge_relation_kind(4), None);
        assert_eq!(curseforge_relation_kind(6), None);
    }

    #[test]
    fn visited_keys_stop_cycles_and_cross_platform_repeats() {
        let mut visited = HashSet::new();
        let root = DependencyTarget::root("sodium-extra".to_string());
        assert!(mark_visited(&mut visited, &root.keys(), &[]));
        let mut root_result = search_result("sodium-extra", String::new(), "a.jar");
        root_result.project_id = Some("PtjYWJkn".to_string());
        assert!(mark_visited(&mut visited, &candidate_keys(&root_result), &root.keys()));

        let sodium = DependencyTarget::required_by(
            &ApiDependency {
                mod_id: "sodium".to_string(),
                slug: Some("sodium".to_string()),
                dependency_type: "required".to_string(),
                version_range: None,
                modrinth_project_id: Some("AANobbMI".to_string()),
                curseforge_mod_id: None,
            },
            1,
        );
        assert_eq!(sodium.keys(), vec!["modrinth:AANobbMI".to_string()]);
        assert!(mark_visited(&mut visited, &sodium.keys(), &[]));
        let mut sodium_result = search_result("sodium", String::new(), "b.jar");
        sodium_result.project_id = Some("AANobbMI".to_string());
        assert!(mark_visited(&mut visited, &candidate_keys(&sodium_result), &sodium.keys()));

        // A cycle back to the root project is not processed again.
        let back_to_root = DependencyTarget {
            query: "sodium-extra".to_string(),
            modrinth_project_id: Some("PtjYWJkn".to_string()),
            curseforge_mod_id: None,
            depth: 2,
        };
        assert!(!mark_visited(&mut visited, &back_to_root.keys(), &[]));

        // The same mod found on CurseForge is recognised through its slug.
        let cf_sodium = DependencyTarget {
            query: "sodium".to_string(),
            modrinth_project_id: None,
            curseforge_mod_id: Some(394468),
            depth: 2,
        };
        assert!(mark_visited(&mut visited, &cf_sodium.keys(), &[]));
        let mut cf_result = search_result("sodium", String::new(), "c.jar");
        cf_result.source = "curseforge".to_string();
        cf_result.project_id = Some("394468".to_string());
        assert!(!mark_visited(&mut visited, &candidate_keys(&cf_result), &cf_sodium.keys()));
    }

    #[test]
    fn id_only_dependency_targets_are_not_name_searched() {
        let target = DependencyTarget {
            query: "AANobbMI".to_string(),
            modrinth_project_id: Some("AANobbMI".to_string()),
            curseforge_mod_id: None,
            depth: 1,
        };
        assert!(target.query_is_id());
        assert!(!DependencyTarget::root("sodium".to_string()).query_is_id());
    }
}
