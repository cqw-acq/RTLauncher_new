use crate::http_client::{
    curseforge_class_ids, curseforge_client, get_with_retry, global_semaphore, modrinth_client,
    shared_client, RetryConfig,
};
use futures::future::join_all;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModLink {
    pub name: String,
    pub url: String,
}
#[tauri::command]
pub async fn get_mod_links(modId: String) -> Result<String, String> {
    let url = format!("https://www.curseforge.com/minecraft/mc-mods/{}", modId);
    let client = shared_client().await;
    let links_to_resolve: Vec<(String, String)> = {
        match client.get(&url).send().await {
            Ok(response) => match response.text().await {
                Ok(html) => {
                    let document = Html::parse_document(&html);
                    let ul_selector_str = String::from("ul.common-link-icon-frame");
                    let ul_selector = Selector::parse(&ul_selector_str).unwrap();
                    let mut links = Vec::new();
                    for ul in document.select(&ul_selector) {
                        let li_selector = Selector::parse("li").unwrap();
                        for li in ul.select(&li_selector) {
                            let a_selector = Selector::parse("a").unwrap();
                            for a in li.select(&a_selector) {
                                let name = a.text().collect::<String>();
                                if let Some(href) = a.value().attr("href") {
                                    if href.starts_with("/") {
                                        let full_url =
                                            format!("https://www.curseforge.com{}", href);
                                        links.push((name, full_url));
                                    }
                                }
                            }
                        }
                    }
                    links
                }
                Err(e) => {
                    eprintln!("Failed to get response text: {}", e);
                    return Err(format!("Failed to get response text: {}", e));
                }
            },
            Err(e) => {
                eprintln!("Request failed: {}", e);
                return Err(format!("Request failed: {}", e));
            }
        }
    };
    let mut links = Vec::new();
    for (name, full_url) in links_to_resolve {
        match client.get(&full_url).send().await {
            Ok(resp) => {
                let final_url: String = resp.url().to_string();
                links.push(ModLink {
                    name,
                    url: final_url,
                });
            }
            Err(e) => {
                eprintln!("Failed to get redirect URL: {}", e);
            }
        }
    }
    match serde_json::to_string(&links) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Failed to serialize result: {}", e)),
    }
}
#[tauri::command]
pub async fn get_curseforge_mod_files(modId: String) -> Result<String, String> {
    let url = format!("https://www.curseforge.com/minecraft/mc-mods/{}", modId);
    let client = shared_client().await;
    let links_to_check: Vec<(String, String)> = {
        match client.get(&url).send().await {
            Ok(response) => match response.text().await {
                Ok(html) => {
                    let document = Html::parse_document(&html);
                    let ul_selector_str = String::from("ul.common-link-icon-frame");
                    let ul_selector = Selector::parse(&ul_selector_str).unwrap();
                    let mut links = Vec::new();
                    for ul in document.select(&ul_selector) {
                        let li_selector = Selector::parse("li").unwrap();
                        for li in ul.select(&li_selector) {
                            let a_selector = Selector::parse("a").unwrap();
                            for a in li.select(&a_selector) {
                                let name = a.text().collect::<String>();
                                if let Some(href) = a.value().attr("href") {
                                    if href.starts_with("/") {
                                        let full_url =
                                            format!("https://www.curseforge.com{}", href);
                                        links.push((name, full_url));
                                    }
                                }
                            }
                        }
                    }
                    links
                }
                Err(e) => {
                    eprintln!("Failed to get response text: {}", e);
                    return Err(format!("Failed to get response text: {}", e));
                }
            },
            Err(e) => {
                eprintln!("Request failed: {}", e);
                return Err(format!("Request failed: {}", e));
            }
        }
    };
    for (_, full_url) in links_to_check {
        match client.head(&full_url).send().await {
            Ok(resp) => {
                let final_url: String = resp.url().to_string();
                if final_url.contains("curseforge") {
                    let parts: Vec<&str> = final_url.split('/').collect();
                    if let Some(mod_id) = parts.last() {
                        match get_mod_files(mod_id).await {
                            Ok(mod_files) => match serde_json::to_string(&mod_files) {
                                Ok(json) => return Ok(json),
                                Err(e) => return Err(format!("Failed to serialize result: {}", e)),
                            },
                            Err(e) => {
                                eprintln!("Failed to get mod files: {}", e);
                                continue;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to get redirect URL: {}", e);
            }
        }
    }
    Err("No curseforge link found or failed to get mod files".to_string())
}
#[tauri::command]
pub async fn get_mod_files_by_slug(slug: String) -> Result<String, String> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return Err("slug cannot be empty".to_string());
    }
    match get_mod_files(trimmed).await {
        Ok(mod_files) => match serde_json::to_string(&mod_files) {
            Ok(json) => Ok(json),
            Err(e) => Err(format!("Failed to serialize result: {}", e)),
        },
        Err(e) => Err(format!("Failed to get mod files: {}", e)),
    }
}
#[tauri::command]
pub async fn get_modrinth_mod_files(slug: String) -> Result<String, String> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return Err("slug cannot be empty".to_string());
    }
    let client = modrinth_client().await;
    let versions_url = format!("https://api.modrinth.com/v2/project/{}/version", trimmed);
    let files_response = get_with_retry(
        &client,
        &versions_url,
        Some(RetryConfig {
            max_retries: 3,
            initial_delay_ms: 600,
            max_delay_ms: 3000,
        }),
    )
    .await
    .map_err(|e| format!("Modrinth version fetch failed (retried 3 times): {}", e))?;
    if !files_response.status().is_success() {
        let status = files_response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(format!("Project not found on Modrinth: {}", trimmed));
        }
        return Err(format!(
            "Modrinth version API returned error status: {}",
            status
        ));
    }
    let files_text = files_response
        .text()
        .await
        .map_err(|e| format!("Failed to read version response: {}", e))?;
    let versions_json: serde_json::Value = serde_json::from_str(&files_text)
        .map_err(|e| format!("Failed to parse version JSON: {}", e))?;
    let versions = versions_json
        .as_array()
        .ok_or_else(|| String::from("Array not found in version response"))?;
    if versions.is_empty() {
        return Err(format!(
            "Modrinth project {} has no available release versions",
            trimmed
        ));
    }
    let mut version_map: HashMap<String, Vec<(Vec<String>, String)>> = HashMap::new();
    for version in versions {
        let files_field = version.get("files").and_then(|f| f.as_array());
        let download_url = files_field.and_then(|files_list| {
            files_list
                .iter()
                .find(|file| {
                    file.get("primary")
                        .and_then(|p| p.as_bool())
                        .unwrap_or(false)
                })
                .or_else(|| files_list.first())
                .and_then(|f| f.get("url").and_then(|u| u.as_str()))
                .map(|s| s.to_string())
        });
        let download_url = match download_url {
            Some(url) => url,
            None => continue,
        };
        let game_versions: Vec<serde_json::Value> = version
            .get("game_versions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let version_type = version
            .get("version_type")
            .and_then(|t| t.as_str())
            .unwrap_or("release");
        let version_number = version
            .get("version_number")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let loaders: Vec<serde_json::Value> = version
            .get("loaders")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        let loader_names: Vec<String> = loaders
            .iter()
            .filter_map(|l| {
                l.as_str().map(|s| {
                    let s_lower = s.to_lowercase();
                    if s_lower == "fabric" {
                        "Fabric".to_string()
                    } else if s_lower == "neoforge" {
                        "NeoForge".to_string()
                    } else if s_lower == "quilt" {
                        "Quilt".to_string()
                    } else {
                        s.to_string()
                    }
                })
            })
            .collect();
        for gv in game_versions.iter().filter_map(|v| v.as_str()) {
            let mut entry_tags: Vec<String> = Vec::new();
            entry_tags.push(version_number.clone());
            for loader in &loader_names {
                entry_tags.push(loader.clone());
            }
            let release_tag = match version_type {
                "release" => "正式版".to_string(),
                "beta" => "Beta".to_string(),
                "alpha" => "Alpha".to_string(),
                _ => "正式版".to_string(),
            };
            entry_tags.push(release_tag);
            let gv_str = gv.to_string();
            let mut dot_count = 0;
            let mut all_digit_or_dot = true;
            for c in gv_str.chars() {
                if c == '.' {
                    dot_count += 1;
                } else if !c.is_ascii_digit() {
                    all_digit_or_dot = false;
                    break;
                }
            }
            if all_digit_or_dot && (dot_count == 1 || dot_count == 2) {
                version_map
                    .entry(gv_str)
                    .or_insert_with(Vec::new)
                    .push((entry_tags, download_url.clone()));
            }
        }
    }
    if version_map.is_empty() {
        return Err("No valid mod files found (mod may not have releases on Modrinth)".to_string());
    }
    match serde_json::to_string(&version_map) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Failed to serialize result: {}", e)),
    }
}
async fn get_mod_files(
    mod_id: &str,
) -> Result<HashMap<String, Vec<(Vec<String>, String)>>, Box<dyn std::error::Error>> {
    let client = curseforge_client().await;
    let class_ids = curseforge_class_ids::all();
    let mut search_urls: Vec<String> = Vec::with_capacity(class_ids.len() + 2);
    search_urls.push(format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&searchFilter={}",
        mod_id
    ));
    for cid in &class_ids {
        search_urls.push(format!(
            "https://api.curseforge.com/v1/mods/search?gameId=432&classId={}&searchFilter={}",
            cid, mod_id
        ));
    }
    search_urls.push(format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&searchFilter={}",
        mod_id
    ));
    let mod_id_owned = mod_id.to_string();
    let retry_cfg = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 600,
        max_delay_ms: 3000,
    };
    let semaphore = global_semaphore().await;
    let search_futures = search_urls.into_iter().map(|url| {
        let client = Arc::clone(&client);
        let mod_id = mod_id_owned.clone();
        let sem = Arc::clone(&semaphore);
        async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("信号量获取失败: {}", e))?;
            match get_with_retry(&client, &url, Some(retry_cfg)).await {
                Ok(response) => {
                    if !response.status().is_success() {
                        return Err(format!(
                            "CurseForge API 返回错误状态: {}",
                            response.status()
                        ));
                    }
                    let text = response
                        .text()
                        .await
                        .map_err(|e| format!("读取响应失败: {}", e))?;
                    let json: serde_json::Value = serde_json::from_str(&text)
                        .map_err(|e| format!("解析 JSON 失败: {}", e))?;
                    Ok::<Option<(i64, bool)>, String>(
                        json.get("data")
                            .and_then(|d| d.as_array())
                            .and_then(|data| {
                                let exact = data.iter().find(|item| {
                                    item.get("slug")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.eq_ignore_ascii_case(&mod_id))
                                        .unwrap_or(false)
                                });
                                let chosen = exact.or_else(|| data.first());
                                chosen
                                    .and_then(|item| item.get("id").and_then(|i| i.as_i64()))
                                    .map(|id| (id, exact.is_some()))
                            }),
                    )
                }
                Err(e) => Err(format!(
                    "CurseForge API request failed (retried 3 times): {}",
                    e
                )),
            }
        }
    });
    let search_results: Vec<Result<Option<(i64, bool)>, String>> = join_all(search_futures).await;
    let mut cur_mod_id: Option<i64> = None;
    let mut last_error: Option<String> = None;
    for result in search_results {
        match result {
            Ok(Some((id, is_exact))) => {
                if is_exact {
                    cur_mod_id = Some(id);
                    break;
                }
                cur_mod_id = Some(id);
            }
            Ok(None) => {}
            Err(e) => {
                last_error = Some(e);
            }
        }
    }
    let cur_mod_id = match cur_mod_id {
        Some(id) => id,
        None => {
            return Err(last_error
                .unwrap_or_else(|| format!("CurseForge 上未找到项目: {}", mod_id))
                .into());
        }
    };
    let files_url = format!("https://api.curseforge.com/v1/mods/{}/files", cur_mod_id);
    let files_response = get_with_retry(&client, &files_url, Some(retry_cfg))
        .await
        .map_err(|e| format!("Mod files request failed (retried 3 times): {}", e))?;
    if !files_response.status().is_success() {
        return Err(format!(
            "Files API returned error status: {}",
            files_response.status()
        )
        .into());
    }
    let files_text = files_response
        .text()
        .await
        .map_err(|e| format!("Failed to read files response: {}", e))?;
    let files_json: serde_json::Value = serde_json::from_str(&files_text)
        .map_err(|e| format!("Failed to parse files JSON: {}", e))?;
    let mut version_map: HashMap<String, Vec<(Vec<String>, String)>> = HashMap::new();
    fn is_mc_version_like(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let trimmed = s.trim_end_matches(|c: char| c == 'x' || c == 'X' || c == '.');
        if trimmed.is_empty() {
            return false;
        }
        let mut dot_count = 0;
        for c in trimmed.chars() {
            if c == '.' {
                dot_count += 1;
            } else if !c.is_ascii_digit() {
                return false;
            }
        }
        dot_count >= 1 && dot_count <= 3
    }
    if let Some(files_data) = files_json.get("data").and_then(|d| d.as_array()) {
        for file in files_data {
            let download_url_opt = file
                .get("downloadUrl")
                .and_then(|u| u.as_str())
                .map(|s| s.to_string());
            let download_url = match download_url_opt {
                Some(url) if !url.is_empty() => url,
                _ => {
                    let file_id = match file.get("id").and_then(|i| i.as_i64()) {
                        Some(id) => id,
                        None => continue,
                    };
                    let file_name = match file.get("fileName").and_then(|n| n.as_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let id_str = file_id.to_string();
                    let (prefix, suffix) = if id_str.len() > 4 {
                        (&id_str[..4], &id_str[4..])
                    } else {
                        (&id_str[..], "")
                    };
                    let suffix_clean = suffix.trim_start_matches('0');
                    let suffix_final = if suffix_clean.is_empty() {
                        "0"
                    } else {
                        suffix_clean
                    };
                    format!(
                        "https://edge.forgecdn.net/files/{}/{}/{}",
                        prefix,
                        suffix_final,
                        urlencoding(&file_name)
                    )
                }
            };
            if let Some(game_versions) = file.get("gameVersions").and_then(|v| v.as_array()) {
                let mut mc_versions: Vec<String> = Vec::new();
                let mut other_versions: Vec<String> = Vec::new();
                for version in game_versions {
                    if let Some(v_str) = version.as_str() {
                        if is_mc_version_like(v_str) {
                            mc_versions.push(v_str.to_string());
                        } else {
                            other_versions.push(v_str.to_string());
                        }
                    }
                }
                if !mc_versions.is_empty() {
                    let release_type =
                        if let Some(rt) = file.get("releaseType").and_then(|r| r.as_i64()) {
                            match rt {
                                1 => "release",
                                2 => "beta",
                                _ => continue,
                            }
                        } else {
                            continue;
                        };
                    let mut versions_with_type = other_versions;
                    versions_with_type.push(release_type.to_string());
                    for mc_ver in &mc_versions {
                        version_map
                            .entry(mc_ver.clone())
                            .or_insert_with(Vec::new)
                            .push((versions_with_type.clone(), download_url.to_string()));
                    }
                }
            }
        }
    }
    if version_map.is_empty() {
        return Err(String::from("No valid files found matching Minecraft version").into());
    }
    Ok(version_map)
}
#[derive(Debug, Serialize, Clone)]
struct ModDownloadProgressPayload {
    task_id: u64,
    percent: f64,
}
#[derive(Debug, Serialize, Clone)]
struct ModDownloadFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
}
static MOD_TASK_COUNTER: AtomicU64 = AtomicU64::new(1000);
struct ModActiveTaskInfo {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    _download_url: String,
    _file_name: String,
}
fn mod_active_tasks() -> &'static Mutex<std::collections::HashMap<u64, ModActiveTaskInfo>> {
    use std::collections::HashMap;
    static TASKS: std::sync::OnceLock<Mutex<HashMap<u64, ModActiveTaskInfo>>> =
        std::sync::OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn get_resource_download_dir(
    resource_kind: &str,
    mc_version: &str,
    mod_loader: &str,
) -> Result<PathBuf, String> {
    use crate::handler::cache_paths::{
        get_cache_dir_for_version, get_mod_cache_dir, parse_mod_loader, parse_resource_kind,
        CacheResourceKind, ModLoaderKind,
    };
    let kind = parse_resource_kind(resource_kind).unwrap_or(CacheResourceKind::Mod);
    if kind == CacheResourceKind::Mod {
        let loader = parse_mod_loader(mod_loader).unwrap_or(ModLoaderKind::Vanilla);
        get_mod_cache_dir(mc_version, loader)
    } else {
        get_cache_dir_for_version(kind, mc_version)
    }
}
fn extract_filename_from_url(url: &str) -> String {
    let cleaned = url.split('?').next().unwrap_or(url);
    let path = PathBuf::from(cleaned);
    let raw_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "mod-{}.jar",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            )
        });
    percent_decode(&raw_name)
}
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                result.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| s.to_string())
}
fn urlencoding(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02x}", b));
            }
        }
    }
    result
}
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
fn extract_world_archive(zip_path: &Path, target_dir: &Path) -> Result<PathBuf, String> {
    let file_name = zip_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("无效的文件名: {}", zip_path.display()))?
        .to_string();
    let ext_zip = ".zip";
    let ext_zip_upper = ".ZIP";
    let ext_mcworld = ".mcworld";
    let ext_mcworld_upper = ".MCWORLD";
    let ext_rar = ".rar";
    let ext_rar_upper = ".RAR";
    let ext_7z = ".7z";
    let ext_7z_upper = ".7Z";
    let dir_name = file_name
        .replace(ext_zip, "")
        .replace(ext_zip_upper, "")
        .replace(ext_mcworld, "")
        .replace(ext_mcworld_upper, "")
        .replace(ext_rar, "")
        .replace(ext_rar_upper, "")
        .replace(ext_7z, "")
        .replace(ext_7z_upper, "");
    let extract_dir = target_dir.join(&dir_name);
    if extract_dir.exists() {
        let _ = fs::remove_dir_all(&extract_dir);
    }
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("创建解压目录失败 {}: {}", extract_dir.display(), e))?;
    let file =
        File::open(zip_path).map_err(|e| format!("打开压缩包 {}: {}", zip_path.display(), e))?;
    let reader = BufReader::new(file);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| format!("解析压缩包 {}: {}", zip_path.display(), e))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取压缩包内文件[{}]: {}", i, e))?;
        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        if outpath.components().any(|c| match c {
            std::path::Component::ParentDir => true,
            _ => false,
        }) {
            continue;
        }
        let entry_path = extract_dir.join(&outpath);
        if file.is_dir() {
            fs::create_dir_all(&entry_path)
                .map_err(|e| format!("创建目录 {}: {}", entry_path.display(), e))?;
        } else {
            if let Some(parent) = entry_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("创建父目录 {}: {}", parent.display(), e))?;
                }
            }
            let mut outfile = File::create(&entry_path)
                .map_err(|e| format!("创建文件 {}: {}", entry_path.display(), e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("写入文件 {}: {}", entry_path.display(), e))?;
        }
    }
    if let Ok(entries) = fs::read_dir(&extract_dir) {
        let top_level: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        if top_level.len() == 1 {
            let only_dir = &top_level[0];
            if only_dir.is_dir() {
                if let Ok(sub_entries) = fs::read_dir(only_dir) {
                    for entry in sub_entries.filter_map(|e| e.ok()) {
                        let src = entry.path();
                        let dst = extract_dir.join(entry.file_name());
                        let _ = fs::rename(&src, &dst);
                    }
                    let _ = fs::remove_dir(only_dir);
                }
            }
        }
    }
    Ok(extract_dir)
}
#[tauri::command]
#[allow(unused_variables)]
pub async fn download_resource_file(
    app: AppHandle,
    resourceKind: String,
    resourceSlug: String,
    resourceName: String,
    mcVersion: String,
    modLoader: String,
    downloadUrl: String,
) -> Result<u64, String> {
    let task_id = MOD_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let save_dir = get_resource_download_dir(&resourceKind, &mcVersion, &modLoader)?;
    let file_name = extract_filename_from_url(&downloadUrl);
    let save_dir_clone = save_dir.clone();
    let file_name_clone = file_name.clone();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let mut tasks = mod_active_tasks().lock().unwrap();
        tasks.insert(
            task_id,
            ModActiveTaskInfo {
                cancel: cancel.clone(),
                _download_url: downloadUrl.clone(),
                _file_name: file_name.clone(),
            },
        );
    }
    let event_name = "mod-download-progress";
    // NOTE: 不要在这里立刻发 0% 事件！
    // 对于已存在且校验通过的文件，modular_download 内部会直接推送 100% 进度，
    // 如果这里先推送 0%，用户会看到进度条闪一下 0% 再跳到完成，体验不好。
    // 真正下载时，modular_download 的 try_download_to_temp 会从 0% 开始主动推送进度。
    let task = crate::downloader::modular_download::DownloadTask {
        file_name,
        target_dir: save_dir,
        urls: vec![downloadUrl.clone()],
        sha1: None, 
    };
    let app_clone = app.clone();
    let task_id_clone = task_id;
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<(u64, u64)>(32);
    tokio::spawn(async move {
        let app2 = app_clone.clone();
        let tid = task_id_clone;
        let progress_forward = tokio::spawn(async move {
            while let Some((downloaded, total)) = progress_rx.recv().await {
                let percent = if total > 0 {
                    (downloaded as f64 / total as f64) * 100.0
                } else {
                    downloaded as f64 / 1_000_000.0
                };
                let progress_event = "mod-download-progress";
                let _ = app2.emit(
                    progress_event,
                    ModDownloadProgressPayload {
                        task_id: tid,
                        percent,
                    },
                );
            }
        });
        let result = crate::downloader::modular_download::download_file(
            &task,
            Some(progress_tx),
            Some(cancel.clone()),
        )
        .await;
        let _ = progress_forward.await;
        match result {
            crate::downloader::modular_download::SingleDownloadResult::Success { size, .. } => {
                let kind_lower = resourceKind.to_ascii_lowercase();
                if kind_lower == "world" || kind_lower == "worlds" {
                    let zip_path = save_dir_clone.join(&file_name_clone);
                    match extract_world_archive(&zip_path, &save_dir_clone) {
                        Ok(extract_dir) => {
                            println!("[world] 存档解压完成: {}", extract_dir.display());
                            let _ = fs::remove_file(&zip_path);
                        }
                        Err(e) => {
                            eprintln!("[world] 存档解压失败: {}", e);
                        }
                    }
                }
                let finished_event = "mod-download-finished";
                let _ = app_clone.emit(
                    finished_event,
                    ModDownloadFinishedPayload {
                        task_id: task_id_clone,
                        success: true,
                        error: None,
                    },
                );
                let _ = size;
            }
            crate::downloader::modular_download::SingleDownloadResult::Failed { error, .. } => {
                let finished_event2 = "mod-download-finished";
                let _ = app_clone.emit(
                    finished_event2,
                    ModDownloadFinishedPayload {
                        task_id: task_id_clone,
                        success: false,
                        error: Some(error),
                    },
                );
            }
        }
    });
    Ok(task_id)
}
#[tauri::command]
pub async fn download_mod_file(
    app: AppHandle,
    modSlug: String,
    modName: String,
    mcVersion: String,
    modLoader: String,
    downloadUrl: String,
) -> Result<u64, String> {
    download_resource_file(
        app,
        "mod".to_string(),
        modSlug,
        modName,
        mcVersion,
        modLoader,
        downloadUrl,
    )
    .await
}
#[tauri::command]
pub fn cancel_mod_download(taskId: u64) -> Result<(), String> {
    let tasks = mod_active_tasks().lock().unwrap();
    if let Some(task) = tasks.get(&taskId) {
        task.cancel.store(true, Ordering::SeqCst);
        return Ok(());
    }
    Err("任务不存在".to_string())
}
#[tauri::command]
pub async fn search_curseforge_projects(
    query: String,
    category: String,
    page_size: Option<u32>,
) -> Result<String, String> {
    let client = curseforge_client().await;
    let semaphore = global_semaphore().await;
    let retry_cfg = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 500,
        max_delay_ms: 3000,
    };
    let ps = page_size.unwrap_or(50);
    let is_mod_category = category == "mod" || category == "mods";
    let encoded_query = urlencoding(&query);
    let mut search_urls: Vec<String> = Vec::new();
    if is_mod_category {
        search_urls.push(format!(
            "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&searchFilter={}&pageSize={}",
            encoded_query, ps
        ));
    } else {
        let candidate_ids = curseforge_class_ids::candidates_for_type(&category);
        for cid in &candidate_ids {
            search_urls.push(format!(
                "https://api.curseforge.com/v1/mods/search?gameId=432&classId={}&searchFilter={}&pageSize={}",
                cid, encoded_query, ps
            ));
        }
        search_urls.push(format!(
            "https://api.curseforge.com/v1/mods/search?gameId=432&searchFilter={}&pageSize={}",
            encoded_query, ps
        ));
    }
    let search_futures = search_urls.into_iter().map(|url| {
        let client = Arc::clone(&client);
        let sem = Arc::clone(&semaphore);
        async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("信号量获取失败: {}", e))?;
            match get_with_retry(&client, &url, Some(retry_cfg)).await {
                Ok(response) => {
                    if !response.status().is_success() {
                        return Err(format!(
                            "CurseForge API 返回错误状态: {}",
                            response.status()
                        ));
                    }
                    let text = response
                        .text()
                        .await
                        .map_err(|e| format!("读取响应失败: {}", e))?;
                    let json: serde_json::Value = serde_json::from_str(&text)
                        .map_err(|e| format!("解析 JSON 失败: {}", e))?;
                    Ok::<serde_json::Value, String>(json)
                }
                Err(e) => Err(format!(
                    "CurseForge API request failed (retried 3 times): {}",
                    e
                )),
            }
        }
    });
    let results: Vec<Result<serde_json::Value, String>> = join_all(search_futures).await;
    let mut all_projects: Vec<serde_json::Value> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for result in results {
        let Ok(json) = result else {
            continue;
        };
        let Some(data) = json.get("data").and_then(|d| d.as_array()) else {
            continue;
        };
        for item in data {
            let id = item
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string())
                .unwrap_or_default();
            if !id.is_empty() {
                if !seen_ids.insert(id) {
                    continue;
                }
            }
            if !is_mod_category {
                let class_id = item
                    .get("classId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                let website_url = item
                    .get("links")
                    .and_then(|l| l.get("websiteUrl"))
                    .and_then(|w| w.as_str())
                    .unwrap_or("");
                if curseforge_class_ids::matches_type(class_id, website_url, &category) {
                    all_projects.push(item.clone());
                }
            } else {
                all_projects.push(item.clone());
            }
        }
    }
    all_projects.sort_by(|a, b| {
        let da = a
            .get("downloadCount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let db = b
            .get("downloadCount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
    let total_count = all_projects.len();
    let mut response_data = serde_json::Map::new();
    response_data.insert(
        "data".to_string(),
        serde_json::to_value(all_projects).unwrap(),
    );
    let mut pagination = serde_json::Map::new();
    pagination.insert(
        "total".to_string(),
        serde_json::Value::Number(total_count.into()),
    );
    pagination.insert("pageSize".to_string(), serde_json::Value::Number(ps.into()));
    pagination.insert("index".to_string(), serde_json::Value::Number(0u32.into()));
    response_data.insert(
        "pagination".to_string(),
        serde_json::Value::Object(pagination),
    );
    serde_json::to_string(&response_data).map_err(|_e| "Failed to serialize JSON".to_string())
}

#[allow(dependency_on_unit_never_type_fallback)]
#[tauri::command]
pub async fn search_modrinth_projects(
    query: String,
    project_type: Option<String>,
    limit: Option<u32>,
) -> Result<String, String> {
    let client = modrinth_client().await;
    let semaphore = global_semaphore().await;
    let retry_cfg = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 500,
        max_delay_ms: 3000,
    };
    let lm = limit.unwrap_or(25);
    let encoded_query = urlencoding(&query);
    let mut facets = String::new();
    if let Some(pt) = project_type {
        let pt_lower = pt.to_lowercase();
        facets = format!(
            "[[\"project_type:{}\"]]",
            match pt_lower.as_str() {
                "modpack" => "modpack",
                "resourcepack" | "texturepack" => "resourcepack",
                "shaderpack" | "shaders" => "shader",
                "datapack" => "datapack",
                "world" | "worlds" => "project:world",
                _ => "mod",
            }
        );
    }
    let encoded_facets = urlencoding(&facets);
    let url = if facets.is_empty() {
        format!(
            "https://api.modrinth.com/v2/search?query={}&limit={}",
            encoded_query, lm
        )
    } else {
        format!(
            "https://api.modrinth.com/v2/search?query={}&limit={}&facets={}",
            encoded_query, lm, encoded_facets
        )
    };
    let _permit = semaphore
        .acquire()
        .await
        .map_err(|e| format!("信号量获取失败: {}", e))?;
    let response = get_with_retry(&client, &url, Some(retry_cfg))
        .await
        .map_err(|e| format!("Modrinth API request failed (retried 3 times): {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Modrinth API 返回错误状态: {}", response.status()));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    Ok(text)
}