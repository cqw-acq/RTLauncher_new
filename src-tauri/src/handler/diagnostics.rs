use crate::handler::mod_parser::ModInfo;
use crate::handler::mod_parser::parse_mods_in_dir;
use crate::http_client::{get_with_retry, modrinth_client, curseforge_client, global_semaphore, shared_client, RetryConfig};
use serde::{Deserialize, Serialize};
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
        "macos" => {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        },
        _ => "Unknown".to_string(),
    };

    let cpu_model = std::process::Command::new("cmd")
        .args(["/c", "wmic cpu get name"])
        .output()
        .map(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            output.lines().nth(1).unwrap_or("Unknown").trim().to_string()
        })
        .unwrap_or_else(|_| "Unknown".to_string());

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDownloadResult {
    pub mod_id: String,
    pub source: String,
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
}

async fn search_modrinth(query: &str, mc_version: &str, loader: &str) -> Option<ModSearchResult> {
    let client = modrinth_client().await;
    let encoded_query = urlencoding::encode(query);
    let facets = format!(
        "[[\"categories:{}\"],[\"versions:{}\"],[\"project_type:mod\"]]",
        loader.to_lowercase(),
        mc_version
    );
    let url = format!(
        "https://api.modrinth.com/v2/search?query={}&facets={}&limit=5",
        encoded_query,
        urlencoding::encode(&facets)
    );

    let response = get_with_retry(
        &client,
        &url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 2000,
        }),
    ).await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let hits = json.get("hits")?.as_array()?;

    if hits.is_empty() {
        return None;
    }

    let first_hit = &hits[0];
    let slug = first_hit.get("slug")?.as_str()?.to_string();

    let versions_url = format!("https://api.modrinth.com/v2/project/{}/version", slug);
    let versions_response = get_with_retry(
        &client,
        &versions_url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 400,
            max_delay_ms: 2000,
        }),
    ).await.ok()?;

    if !versions_response.status().is_success() {
        return None;
    }

    let versions_text = versions_response.text().await.ok()?;
    let versions_json: serde_json::Value = serde_json::from_str(&versions_text).ok()?;
    let versions = versions_json.as_array()?;

    for version in versions {
        let game_versions = version
            .get("game_versions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        
        let loaders = version
            .get("loaders")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();

        let mc_prefix = mc_version.split('.').take(2).collect::<Vec<_>>().join(".");
        let matches_mc = game_versions.iter().any(|gv| {
            gv.as_str().map(|s| s == mc_version || s.starts_with(&mc_prefix))
                .unwrap_or(false)
        });

        let matches_loader = loaders.iter().any(|l| {
            l.as_str().map(|s| s.to_lowercase() == loader.to_lowercase()).unwrap_or(false)
        });

        if matches_mc || matches_loader {
            let files_field = version.get("files").and_then(|f| f.as_array());
            let file_info = files_field.and_then(|files_list| {
                files_list
                    .iter()
                    .find(|file| {
                        file.get("primary")
                            .and_then(|p| p.as_bool())
                            .unwrap_or(false)
                    })
                    .or_else(|| files_list.first())
            });

            if let Some(file) = file_info {
                let download_url = file.get("url").and_then(|u| u.as_str())?.to_string();
                let file_name = file.get("filename").and_then(|f| f.as_str())?.to_string();
                let version_number = version.get("version_number").and_then(|v| v.as_str())?.to_string();

                let mc_list: Vec<String> = game_versions.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                let loader_list: Vec<String> = loaders.iter()
                    .filter_map(|l| l.as_str().map(|s| s.to_string()))
                    .collect();

                return Some(ModSearchResult {
                    source: "modrinth".to_string(),
                    mod_id: query.to_string(),
                    slug: slug.clone(),
                    title: first_hit.get("title").and_then(|t| t.as_str()).unwrap_or(&slug).to_string(),
                    download_url: Some(download_url),
                    file_name: Some(file_name),
                    mc_versions: mc_list,
                    loaders: loader_list,
                    version_number: Some(version_number),
                });
            }
        }
    }

    None
}

async fn search_curseforge(query: &str, mc_version: &str, loader: &str) -> Option<ModSearchResult> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let encoded_query = urlencoding::encode(query);
    
    let loader_filter = match loader.to_lowercase().as_str() {
        "forge" => "1",
        "fabric" => "4",
        "quilt" => "5",
        "neoforge" => "6",
        _ => "",
    };

    let url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&searchFilter={}&gameVersion={}&modLoaderType={}&pageSize=5",
        encoded_query,
        mc_version,
        loader_filter
    );

    let _permit = semaphore.acquire().await.ok()?;
    let response = get_with_retry(
        &client,
        &url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 600,
            max_delay_ms: 3000,
        }),
    ).await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let data = json.get("data")?.as_array()?;

    if data.is_empty() {
        return None;
    }

    for mod_item in data.iter().take(3) {
        let mod_id = mod_item.get("id").and_then(|id| id.as_u64())?;
        let title = mod_item.get("name").and_then(|n| n.as_str())?.to_string();
        let slug = mod_item.get("slug").and_then(|s| s.as_str())?.to_string();

        let files_url = format!(
            "https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&modLoaderType={}&pageSize=10",
            mod_id, mc_version, loader_filter
        );

        let files_response = get_with_retry(
            &client,
            &files_url,
            Some(RetryConfig {
                max_retries: 2,
                initial_delay_ms: 500,
                max_delay_ms: 2000,
            }),
        ).await.ok()?;

        if !files_response.status().is_success() {
            continue;
        }

        let files_text = files_response.text().await.ok()?;
        let files_json: serde_json::Value = serde_json::from_str(&files_text).ok()?;
        let files_data = files_json.get("data").and_then(|d| d.as_array()).unwrap_or(&vec![]).clone();

        if let Some(file) = files_data.first() {
            if let Some(dl_url) = file.get("downloadUrl").and_then(|u| u.as_str()) {
                let file_name = file.get("fileName").and_then(|f| f.as_str())?.to_string();
                let version_number = file.get("displayName").and_then(|v| v.as_str())?.to_string();
                
                let game_versions_raw = file.get("gameVersions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let mc_list: Vec<String> = game_versions_raw.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();

                return Some(ModSearchResult {
                    source: "curseforge".to_string(),
                    mod_id: query.to_string(),
                    slug,
                    title,
                    download_url: Some(dl_url.to_string()),
                    file_name: Some(file_name),
                    mc_versions: mc_list,
                    loaders: vec![loader.to_string()],
                    version_number: Some(version_number),
                });
            }
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

    let (modrinth_result, curseforge_result) = search_mod_both_sources(&mod_id, &mc_version, &loader).await;

    let sources: Vec<(&str, Option<ModSearchResult>)> = vec![
        ("modrinth", modrinth_result.clone()),
        ("curseforge", curseforge_result.clone()),
    ];

    for (source_name, result) in &sources {
        if let Some(search_result) = result {
            if let (Some(url), Some(filename)) = (&search_result.download_url, &search_result.file_name) {
                let dest_path = mods_dir.join(filename);
                if dest_path.exists() {
                    return Ok(DependencyDownloadResult {
                        mod_id: mod_id.clone(),
                        source: "local".to_string(),
                        success: true,
                        message: format!("文件已存在，跳过: {}", filename),
                        file_path: Some(dest_path.to_string_lossy().to_string()),
                    });
                }
                
                match download_file(url, &dest_path).await {
                    Ok(_) => {
                        return Ok(DependencyDownloadResult {
                            mod_id: mod_id.clone(),
                            source: source_name.to_string(),
                            success: true,
                            message: format!("成功从 {} 下载: {}", source_name, filename),
                            file_path: Some(dest_path.to_string_lossy().to_string()),
                        });
                    }
                    Err(e) => {
                        eprintln!("从 {} 下载 {} 失败: {}", source_name, mod_id, e);
                        continue;
                    }
                }
            }
        }
    }

    if modrinth_result.is_none() && curseforge_result.is_none() {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModWithDeps {
    pub search_result: ModSearchResult,
    pub dependencies: Vec<ApiDependency>,
}

async fn fetch_modrinth_version_dependencies(_project_id: &str, version_id: &str) -> Option<Vec<ApiDependency>> {
    let client = modrinth_client().await;

    let deps_url = format!("https://api.modrinth.com/v2/version/{}", version_id);
    let response = get_with_retry(
        &client,
        &deps_url,
        Some(RetryConfig { max_retries: 2, initial_delay_ms: 400, max_delay_ms: 2000 }),
    ).await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut deps = Vec::new();

    if let Some(dep_arr) = json.get("dependencies").and_then(|d| d.as_array()) {
        for dep in dep_arr {
            let dep_type = dep.get("dependency_type")
                .and_then(|t| t.as_str())
                .unwrap_or("required")
                .to_string();

            if dep_type != "required" && dep_type != "optional" && dep_type != "incompatible" {
                continue;
            }

            let version_range = dep.get("version_range").and_then(|v| v.as_str()).map(|s| s.to_string());

            if let Some(project_id_val) = dep.get("project_id").and_then(|p| p.as_str()) {
                let proj_url = format!("https://api.modrinth.com/v2/project/{}", project_id_val);
                let proj_resp = get_with_retry(
                    &client, &proj_url,
                    Some(RetryConfig { max_retries: 1, initial_delay_ms: 300, max_delay_ms: 1000 }),
                ).await.ok();
                let proj_text = match proj_resp {
                    Some(r) => r.text().await.ok(),
                    None => None,
                };
                let slug: Option<String> = proj_text
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .and_then(|j| j.get("slug").and_then(|s| s.as_str()).map(|s| s.to_string()));

                let mod_id = slug.clone().unwrap_or_else(|| project_id_val.to_string());

                deps.push(ApiDependency {
                    mod_id,
                    slug,
                    dependency_type: dep_type,
                    version_range,
                });
            } else if let Some(version_id_dep) = dep.get("version_id").and_then(|v| v.as_str()) {
                deps.push(ApiDependency {
                    mod_id: version_id_dep.to_string(),
                    slug: None,
                    dependency_type: dep_type,
                    version_range,
                });
            }
        }
    }

    Some(deps)
}

async fn fetch_curseforge_mod_dependencies(mod_id: i64, file_id: i64) -> Option<Vec<ApiDependency>> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let _permit = semaphore.acquire().await.ok()?;

    let file_url = format!("https://api.curseforge.com/v1/mods/{}/files/{}", mod_id, file_id);
    let response = get_with_retry(
        &client,
        &file_url,
        Some(RetryConfig { max_retries: 2, initial_delay_ms: 500, max_delay_ms: 2000 }),
    ).await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut deps = Vec::new();

    if let Some(dep_arr) = json.get("data")
        .and_then(|d| d.get("dependencies"))
        .and_then(|d| d.as_array()) {
        
        for dep in dep_arr {
            let relation_type = dep.get("relationType").and_then(|r| r.as_u64()).unwrap_or(0);
            let dep_mod_id = dep.get("modId").and_then(|m| m.as_u64());

            let dep_type = match relation_type {
                1 => "required",
                2 => "optional",
                3 => "incompatible",
                _ => continue,
            }.to_string();

            if let Some(dmid) = dep_mod_id {
                let mod_info_url = format!("https://api.curseforge.com/v1/mods/{}", dmid);
                let info_resp = get_with_retry(
                    &client, &mod_info_url,
                    Some(RetryConfig { max_retries: 1, initial_delay_ms: 300, max_delay_ms: 1000 }),
                ).await.ok();
                let info_text = match info_resp {
                    Some(r) => r.text().await.ok(),
                    None => None,
                };
                let (slug_opt, name_opt) = info_text
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .map(|j| {
                        let s = j.get("data").and_then(|d| d.get("slug")).and_then(|s| s.as_str()).map(|s| s.to_string());
                        let n = j.get("data").and_then(|d| d.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string());
                        (s, n)
                    })
                    .unwrap_or((None, None));

                let mod_id = slug_opt.clone()
                    .or(name_opt.clone())
                    .unwrap_or_else(|| format!("curseforge_{}", dmid));

                deps.push(ApiDependency {
                    mod_id,
                    slug: slug_opt,
                    dependency_type: dep_type,
                    version_range: None,
                });
            }
        }
    }

    Some(deps)
}

async fn search_modrinth_with_deps(query: &str, mc_version: &str, loader: &str) -> Option<ModWithDeps> {
    let client = modrinth_client().await;
    let encoded_query = urlencoding::encode(query);
    let facets = format!(
        "[[\"categories:{}\"],[\"versions:{}\"],[\"project_type:mod\"]]",
        loader.to_lowercase(), mc_version
    );
    let url = format!(
        "https://api.modrinth.com/v2/search?query={}&facets={}&limit=5",
        encoded_query, urlencoding::encode(&facets)
    );

    let response = get_with_retry(
        &client, &url,
        Some(RetryConfig { max_retries: 2, initial_delay_ms: 500, max_delay_ms: 2000 }),
    ).await.ok()?;
    if !response.status().is_success() { return None; }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let hits = json.get("hits")?.as_array()?;
    if hits.is_empty() { return None; }

    let first_hit = &hits[0];
    let slug = first_hit.get("slug")?.as_str()?.to_string();

    let versions_url = format!("https://api.modrinth.com/v2/project/{}/version", slug);
    let versions_response = get_with_retry(
        &client, &versions_url,
        Some(RetryConfig { max_retries: 2, initial_delay_ms: 400, max_delay_ms: 2000 }),
    ).await.ok()?;
    if !versions_response.status().is_success() { return None; }

    let versions_text = versions_response.text().await.ok()?;
    let versions_json: serde_json::Value = serde_json::from_str(&versions_text).ok()?;
    let versions = versions_json.as_array()?;

    let mc_prefix = mc_version.split('.').take(2).collect::<Vec<_>>().join(".");

    for version in versions {
        let game_versions = version.get("game_versions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let loaders = version.get("loaders").and_then(|l| l.as_array()).cloned().unwrap_or_default();

        let matches_mc = game_versions.iter().any(|gv| {
            gv.as_str().map(|s| s == mc_version || s.starts_with(&mc_prefix)).unwrap_or(false)
        });
        let matches_loader = loaders.iter().any(|l| {
            l.as_str().map(|s| s.to_lowercase() == loader.to_lowercase()).unwrap_or(false)
        });

        if matches_mc || matches_loader {
            let files_field = version.get("files").and_then(|f| f.as_array());
            let file_info = files_field.and_then(|files_list| {
                files_list.iter().find(|file| {
                    file.get("primary").and_then(|p| p.as_bool()).unwrap_or(false)
                }).or_else(|| files_list.first())
            });

            if let (Some(file), Some(version_id)) = (file_info, version.get("id").and_then(|v| v.as_str())) {
                let download_url = file.get("url").and_then(|u| u.as_str())?.to_string();
                let file_name = file.get("filename").and_then(|f| f.as_str())?.to_string();
                let version_number = version.get("version_number").and_then(|v| v.as_str())?.to_string();

                let mc_list: Vec<String> = game_versions.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                let loader_list: Vec<String> = loaders.iter().filter_map(|l| l.as_str().map(|s| s.to_string())).collect();

                let dependencies = fetch_modrinth_version_dependencies(&slug, version_id).await.unwrap_or_default();

                return Some(ModWithDeps {
                    search_result: ModSearchResult {
                        source: "modrinth".to_string(),
                        mod_id: query.to_string(),
                        slug: slug.clone(),
                        title: first_hit.get("title").and_then(|t| t.as_str()).unwrap_or(&slug).to_string(),
                        download_url: Some(download_url),
                        file_name: Some(file_name),
                        mc_versions: mc_list,
                        loaders: loader_list,
                        version_number: Some(version_number),
                    },
                    dependencies,
                });
            }
        }
    }
    None
}

async fn search_curseforge_with_deps(query: &str, mc_version: &str, loader: &str) -> Option<ModWithDeps> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let encoded_query = urlencoding::encode(query);

    let loader_filter = match loader.to_lowercase().as_str() {
        "forge" => "1", "fabric" => "4", "quilt" => "5", "neoforge" => "6", _ => "",
    };

    let url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&searchFilter={}&gameVersion={}&modLoaderType={}&pageSize=5",
        encoded_query, mc_version, loader_filter
    );

    let _permit = semaphore.acquire().await.ok()?;
    let response = get_with_retry(
        &client, &url,
        Some(RetryConfig { max_retries: 2, initial_delay_ms: 600, max_delay_ms: 3000 }),
    ).await.ok()?;
    if !response.status().is_success() { return None; }

    let text = response.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let data = json.get("data")?.as_array()?;
    if data.is_empty() { return None; }

    for mod_item in data.iter().take(3) {
        let cur_mod_id = mod_item.get("id").and_then(|id| id.as_i64())?;
        let title = mod_item.get("name").and_then(|n| n.as_str())?.to_string();
        let slug = mod_item.get("slug").and_then(|s| s.as_str())?.to_string();

        let files_url = format!(
            "https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&modLoaderType={}&pageSize=10",
            cur_mod_id, mc_version, loader_filter
        );

        let files_response = get_with_retry(
            &client, &files_url,
            Some(RetryConfig { max_retries: 2, initial_delay_ms: 500, max_delay_ms: 2000 }),
        ).await.ok()?;
        if !files_response.status().is_success() { continue; }

        let files_text = files_response.text().await.ok()?;
        let files_json: serde_json::Value = serde_json::from_str(&files_text).ok()?;
        let files_data = files_json.get("data").and_then(|d| d.as_array()).unwrap_or(&vec![]).clone();

        if let Some(file) = files_data.first() {
            if let (Some(dl_url), Some(file_id)) = (
                file.get("downloadUrl").and_then(|u| u.as_str()),
                file.get("id").and_then(|i| i.as_i64())
            ) {
                let file_name = file.get("fileName").and_then(|f| f.as_str())?.to_string();
                let version_number = file.get("displayName").and_then(|v| v.as_str())?.to_string();
                let game_versions_raw = file.get("gameVersions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let mc_list: Vec<String> = game_versions_raw.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();

                let dependencies = fetch_curseforge_mod_dependencies(cur_mod_id, file_id).await.unwrap_or_default();

                return Some(ModWithDeps {
                    search_result: ModSearchResult {
                        source: "curseforge".to_string(),
                        mod_id: query.to_string(),
                        slug: slug.clone(),
                        title,
                        download_url: Some(dl_url.to_string()),
                        file_name: Some(file_name),
                        mc_versions: mc_list,
                        loaders: vec![loader.to_string()],
                        version_number: Some(version_number),
                    },
                    dependencies,
                });
            }
        }
    }
    None
}

#[tauri::command]
pub async fn auto_download_with_dependencies(
    instance_dir: String,
    mod_id: String,
    mc_version: String,
    loader: String,
) -> Result<Vec<DependencyDownloadResult>, String> {
    let mods_dir = Path::new(&instance_dir).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| format!("创建mods目录失败: {}", e))?;

    let mut results = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<(String, usize)> = vec![(mod_id, 0)];
    let max_depth = 5;

    while let Some((current_id, depth)) = queue.pop() {
        if depth > max_depth || visited.contains(&current_id) {
            continue;
        }
        visited.insert(current_id.clone());

        let mods_dir_copy = mods_dir.clone();
        let mc_ver = mc_version.clone();
        let loader_copy = loader.clone();
        let id_copy = current_id.clone();

        let (mr_opt, cf_opt) = tokio::join!(
            search_modrinth_with_deps(&id_copy, &mc_ver, &loader_copy),
            search_curseforge_with_deps(&id_copy, &mc_ver, &loader_copy)
        );

        let sources: Vec<(String, Option<ModWithDeps>)> = vec![
            ("modrinth".to_string(), mr_opt),
            ("curseforge".to_string(), cf_opt),
        ];

        let mut downloaded = false;
        for (source_name, mod_with_deps_opt) in &sources {
            if let Some(mwd) = mod_with_deps_opt {
                if let (Some(url), Some(filename)) = (&mwd.search_result.download_url, &mwd.search_result.file_name) {
                    let dest_path = mods_dir_copy.join(filename);
                    if !dest_path.exists() {
                        match download_file(url, &dest_path).await {
                            Ok(_) => {
                                results.push(DependencyDownloadResult {
                                    mod_id: current_id.clone(),
                                    source: source_name.clone(),
                                    success: true,
                                    message: format!("从 {} 下载成功: {}", source_name, filename),
                                    file_path: Some(dest_path.to_string_lossy().to_string()),
                                });

                                for dep in &mwd.dependencies {
                                    if dep.dependency_type == "required" && !is_platform_dep(&dep.mod_id) {
                                        queue.push((dep.mod_id.clone(), depth + 1));
                                    }
                                }
                                downloaded = true;
                                break;
                            }
                            Err(e) => {
                                eprintln!("从 {} 下载 {} 失败: {}", source_name, current_id, e);
                            }
                        }
                    } else {
                        results.push(DependencyDownloadResult {
                            mod_id: current_id.clone(),
                            source: source_name.clone(),
                            success: true,
                            message: format!("已存在，跳过: {}", filename),
                            file_path: Some(dest_path.to_string_lossy().to_string()),
                        });

                        for dep in &mwd.dependencies {
                            if dep.dependency_type == "required" && !is_platform_dep(&dep.mod_id) {
                                queue.push((dep.mod_id.clone(), depth + 1));
                            }
                        }
                        downloaded = true;
                        break;
                    }
                }
            }
        }

        if !downloaded {
            results.push(DependencyDownloadResult {
                mod_id: current_id.clone(),
                source: "not_found".to_string(),
                success: false,
                message: format!("模组 '{}' 在双平台均未找到或下载失败", current_id),
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

    let result = zip.finish();
    if let Err(e) = result {
        return Err(format!("完成zip失败: {}", e));
    }

    Ok(zip_path.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedModDependency {
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub download_url: String,
}

#[tauri::command]
pub async fn get_modrinth_required_dependencies(
    project_slug: String,
    mc_version: String,
    mod_loader: String,
    download_url: Option<String>,
) -> Result<Vec<ResolvedModDependency>, String> {
    let client = modrinth_client().await;

    let version_id_from_url = download_url.as_ref().and_then(|u| {
        let re = regex::Regex::new(r"/v2/version_file/([a-zA-Z0-9]+)/").ok();
        re.and_then(|r| r.captures(u))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
    });

    let versions_url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        urlencoding::encode(&project_slug)
    );
    let versions_resp = get_with_retry(
        &client,
        &versions_url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 400,
            max_delay_ms: 2000,
        }),
    )
    .await
    .map_err(|e| format!("获取 Modrinth 版本列表失败: {}", e))?;

    if !versions_resp.status().is_success() {
        return Err(format!(
            "Modrinth 版本 API 返回错误: {}",
            versions_resp.status()
        ));
    }

    let versions_text = versions_resp
        .text()
        .await
        .map_err(|e| format!("读取 Modrinth 响应失败: {}", e))?;
    let versions_json: serde_json::Value =
        serde_json::from_str(&versions_text).map_err(|e| format!("解析版本 JSON 失败: {}", e))?;
    let versions = versions_json
        .as_array()
        .ok_or("Modrinth 版本列表格式不正确")?;

    let mc_prefix = mc_version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    let mut selected_version: Option<(serde_json::Value, String)> = None;

    if let Some(ref vid) = version_id_from_url {
        for v in versions {
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                if id == vid {
                    selected_version = Some((v.clone(), vid.clone()));
                    break;
                }
            }
        }
    }

    if selected_version.is_none() {
        for v in versions {
            let game_versions = v
                .get("game_versions")
                .and_then(|gv| gv.as_array())
                .cloned()
                .unwrap_or_default();
            let loaders = v
                .get("loaders")
                .and_then(|l| l.as_array())
                .cloned()
                .unwrap_or_default();

            let matches_mc = game_versions.iter().any(|gv| {
                gv.as_str()
                    .map(|s| s == mc_version || s.starts_with(&mc_prefix))
                    .unwrap_or(false)
            });
            let matches_loader = loaders.iter().any(|l| {
                l.as_str()
                    .map(|s| s.to_lowercase() == mod_loader.to_lowercase())
                    .unwrap_or(false)
            });

            if matches_mc || matches_loader {
                if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                    selected_version = Some((v.clone(), id.to_string()));
                    break;
                }
            }
        }
    }

    let selected_version = match selected_version {
        Some(v) => v,
        None => return Ok(vec![]),
    };

    let (version_value, version_id) = selected_version;
    let raw_deps = fetch_modrinth_version_dependencies(&project_slug, &version_id)
        .await
        .unwrap_or_default();

    let required_deps: Vec<ApiDependency> = raw_deps
        .into_iter()
        .filter(|d| d.dependency_type == "required" && !is_platform_dep(&d.mod_id))
        .collect();

    let mut resolved: Vec<ResolvedModDependency> = Vec::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(3));
    let mut tasks = Vec::new();

    for dep in required_deps {
        let dep_slug = dep.slug.clone().unwrap_or_else(|| dep.mod_id.clone());
        let permit = sem.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let mc_ver = mc_version.clone();
        let loader = mod_loader.clone();
        let dep_name = dep.mod_id.clone();
        let task = tokio::spawn(async move {
            let _permit = permit;
            let res = search_modrinth_with_deps(&dep_slug, &mc_ver, &loader).await;
            (dep_name, dep_slug, res)
        });
        tasks.push(task);
    }

    let task_results = join_all(tasks).await;
    for tr in task_results {
        if let Ok((dep_name, dep_slug, res_opt)) = tr {
            if let Some(mwd) = res_opt {
                if let (Some(url), Some(title)) = (
                    mwd.search_result.download_url,
                    Some(mwd.search_result.title),
                ) {
                    resolved.push(ResolvedModDependency {
                        project_id: mwd.search_result.mod_id.clone(),
                        project_slug: if mwd.search_result.slug.is_empty() { dep_slug.clone() } else { mwd.search_result.slug.clone() },
                        project_name: title,
                        download_url: url,
                    });
                    continue;
                }
            }
            // fallback: 也试试 curseforge
            let cf_res = search_curseforge_with_deps(&dep_slug, &mc_version, &mod_loader).await;
            if let Some(mwd) = cf_res {
                if let (Some(url), Some(title)) = (
                    mwd.search_result.download_url,
                    Some(mwd.search_result.title),
                ) {
                    resolved.push(ResolvedModDependency {
                        project_id: mwd.search_result.mod_id.clone(),
                        project_slug: if mwd.search_result.slug.is_empty() { dep_slug.clone() } else { mwd.search_result.slug.clone() },
                        project_name: title,
                        download_url: url,
                    });
                    continue;
                }
            }
            // 实在没找到就跳过，不影响主流程
            let _ = dep_name;
        }
    }

    // 避免重复下载
    resolved.sort_by(|a, b| a.project_slug.cmp(&b.project_slug));
    resolved.dedup_by(|a, b| a.download_url == b.download_url);

    let _ = version_value;
    Ok(resolved)
}

#[tauri::command]
pub async fn get_curseforge_required_dependencies(
    project_slug: String,
    mc_version: String,
    mod_loader: String,
    download_url: Option<String>,
) -> Result<Vec<ResolvedModDependency>, String> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let _sem_permit = semaphore.acquire().await.map_err(|e| e.to_string())?;

    let search_url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&searchFilter={}&modLoaderType={}&gameVersion={}&pageSize=5&classId=6",
        urlencoding::encode(&project_slug),
        loader_name_to_curseforge_id(&mod_loader),
        urlencoding::encode(&mc_version),
    );

    let search_resp = get_with_retry(
        &client,
        &search_url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 2000,
        }),
    )
    .await
    .map_err(|e| format!("CurseForge 搜索失败: {}", e))?;

    if !search_resp.status().is_success() {
        return Err(format!(
            "CurseForge API 返回错误: {}",
            search_resp.status()
        ));
    }

    let search_text = search_resp
        .text()
        .await
        .map_err(|e| format!("读取 CurseForge 响应失败: {}", e))?;
    let search_json: serde_json::Value =
        serde_json::from_str(&search_text).map_err(|e| format!("解析 CurseForge 搜索 JSON 失败: {}", e))?;
    let data = search_json.get("data").and_then(|d| d.as_array());
    let mods_list = data.cloned().unwrap_or_default();
    if mods_list.is_empty() {
        return Ok(vec![]);
    }

    let first_mod = &mods_list[0];
    let mod_id = first_mod
        .get("id")
        .and_then(|m| m.as_i64())
        .ok_or("CurseForge 模组 ID 无效")?;

    let files_url = format!(
        "https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&pageSize=20",
        mod_id,
        urlencoding::encode(&mc_version)
    );
    let files_resp = get_with_retry(
        &client,
        &files_url,
        Some(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 400,
            max_delay_ms: 2000,
        }),
    )
    .await
    .ok();
    let file_id_from_url = download_url.as_ref().and_then(|u| {
        let re = regex::Regex::new(r"/(\d{8,12})/").ok();
        re.and_then(|r| r.captures(u))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
    });

    let mut picked_file_id: Option<i64> = None;
    if let Some(files_resp) = files_resp {
        if files_resp.status().is_success() {
            if let Ok(text) = files_resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(files) = json.get("data").and_then(|d| d.as_array()) {
                        if let Some(fid_from_url) = file_id_from_url.clone() {
                            if let Ok(parsed) = fid_from_url.parse::<i64>() {
                                for f in files {
                                    if f.get("id").and_then(|x| x.as_i64()) == Some(parsed) {
                                        picked_file_id = Some(parsed);
                                        break;
                                    }
                                }
                            }
                        }
                        if picked_file_id.is_none() {
                            if let Some(first_file) = files.first() {
                                picked_file_id = first_file.get("id").and_then(|x| x.as_i64());
                            }
                        }
                    }
                }
            }
        }
    }

    let file_id = match picked_file_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    let deps = fetch_curseforge_mod_dependencies(mod_id, file_id)
        .await
        .unwrap_or_default();

    let required_deps: Vec<ApiDependency> = deps
        .into_iter()
        .filter(|d| d.dependency_type == "required" && !is_platform_dep(&d.mod_id))
        .collect();

    let mut resolved: Vec<ResolvedModDependency> = Vec::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(2));
    let mut tasks = Vec::new();

    for dep in required_deps {
        let dep_slug = dep.slug.clone().unwrap_or_else(|| dep.mod_id.clone());
        let permit = sem.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let mc_ver = mc_version.clone();
        let loader = mod_loader.clone();
        let task = tokio::spawn(async move {
            let _permit = permit;
            let (mr, cf) = tokio::join!(
                search_modrinth_with_deps(&dep_slug, &mc_ver, &loader),
                search_curseforge_with_deps(&dep_slug, &mc_ver, &loader)
            );
            (dep_slug, mr, cf)
        });
        tasks.push(task);
    }

    let task_results = join_all(tasks).await;
    for tr in task_results {
        if let Ok((dep_slug, mr_opt, cf_opt)) = tr {
            if let Some(mwd) = mr_opt {
                if let (Some(url), Some(title)) = (mwd.search_result.download_url, Some(mwd.search_result.title)) {
                    resolved.push(ResolvedModDependency {
                        project_id: mwd.search_result.mod_id.clone(),
                        project_slug: if mwd.search_result.slug.is_empty() { dep_slug.clone() } else { mwd.search_result.slug.clone() },
                        project_name: title,
                        download_url: url,
                    });
                    continue;
                }
            }
            if let Some(mwd) = cf_opt {
                if let (Some(url), Some(title)) = (mwd.search_result.download_url, Some(mwd.search_result.title)) {
                    resolved.push(ResolvedModDependency {
                        project_id: mwd.search_result.mod_id.clone(),
                        project_slug: if mwd.search_result.slug.is_empty() { dep_slug.clone() } else { mwd.search_result.slug.clone() },
                        project_name: title,
                        download_url: url,
                    });
                    continue;
                }
            }
        }
    }

    resolved.sort_by(|a, b| a.project_slug.cmp(&b.project_slug));
    resolved.dedup_by(|a, b| a.download_url == b.download_url);
    Ok(resolved)
}

fn loader_name_to_curseforge_id(loader: &str) -> u32 {
    // CurseForge: https://docs.curseforge.com/#tocS_ModLoaderType
    // Any = 0, Forge = 1, Cauldron = 2, LiteLoader = 3, Fabric = 4,
    // Quilt = 5, NeoForge = 6
    match loader.to_lowercase().trim() {
        l if l.contains("forge") && l.contains("neo") => 6,
        l if l.contains("forge") => 1,
        l if l.contains("fabric") => 4,
        l if l.contains("quilt") => 5,
        l if l.contains("liteloader") => 3,
        _ => 1,
    }
}