use crate::http_client::{
    curseforge_class_ids, curseforge_client, get_with_retry, global_semaphore, modrinth_client,
    shared_client, RetryConfig,
};
use futures::future::join_all;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
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

/// Search Modrinth through the native HTTP client instead of the WebView.
/// This avoids WebView CORS and proxy-policy failures being reported as an
/// unavailable Modrinth source.
#[tauri::command]
pub async fn search_modrinth_projects(
    query: String,
    project_type: Option<String>,
    limit: Option<u32>,
) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("query cannot be empty".to_string());
    }

    let limit = limit.unwrap_or(25).clamp(1, 100);
    let url = if let Some(project_type) = project_type {
        let project_type = project_type.trim().to_ascii_lowercase();
        if !matches!(
            project_type.as_str(),
            "mod" | "modpack" | "resourcepack" | "shader" | "datapack" | "world"
        ) {
            return Err(format!("unsupported Modrinth project type: {}", project_type));
        }
        let facets = format!(r#"[["project_type:{}"]]"#, project_type);
        format!(
            "https://api.modrinth.com/v2/search?query={}&limit={}&facets={}",
            urlencoding(query),
            limit,
            urlencoding(&facets),
        )
    } else {
        format!(
            "https://api.modrinth.com/v2/search?query={}&limit={}",
            urlencoding(query),
            limit,
        )
    };
    let client = modrinth_client().await;
    let response = get_with_retry(
        &client,
        &url,
        Some(RetryConfig {
            max_retries: 3,
            initial_delay_ms: 500,
            max_delay_ms: 3000,
        }),
    )
    .await
    .map_err(|error| format!("Modrinth search failed (retried 3 times): {}", error))?;

    if !response.status().is_success() {
        return Err(format!("Modrinth search returned HTTP {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|error| format!("Failed to read Modrinth search response: {}", error))
}

#[tauri::command]
pub async fn get_modrinth_project(slug: String) -> Result<String, String> {
    let slug = slug.trim();
    if slug.is_empty() {
        return Err("slug cannot be empty".to_string());
    }

    let url = format!("https://api.modrinth.com/v2/project/{}", urlencoding(slug));
    let client = modrinth_client().await;
    let response = get_with_retry(&client, &url, None)
        .await
        .map_err(|error| format!("Modrinth project fetch failed: {}", error))?;
    if !response.status().is_success() {
        return Err(format!("Modrinth project fetch returned HTTP {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|error| format!("Failed to read Modrinth project response: {}", error))
}

const MAX_AUTO_RESOLVED_DEPENDENCIES: usize = 32;
/// CurseForge caps `pageSize` at 50. Together with the `gameVersion` filter a few
/// pages cover every file a project published for one Minecraft version.
const CURSEFORGE_FILES_PAGE_SIZE: usize = 50;
const CURSEFORGE_MAX_FILE_PAGES: usize = 8;
/// Loader keywords in match order: `neoforge` must be tested before `forge`,
/// which it contains.
const KNOWN_MOD_LOADERS: [&str; 6] = ["neoforge", "fabric", "quilt", "liteloader", "ornithe", "forge"];

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedModDependency {
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub download_url: String,
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| field.as_str()).map(str::to_owned)
}

/// Maps a loader name from the UI, an instance or a folder name to the loader
/// tag used by Modrinth and CurseForge. `None` means the loader is unknown
/// (vanilla, "universal", or a name without any loader keyword).
pub(crate) fn canonical_mod_loader(raw: &str) -> Option<&'static str> {
    let lower = raw.trim().to_ascii_lowercase();
    KNOWN_MOD_LOADERS.iter().copied().find(|loader| lower.contains(loader))
}

pub(crate) fn primary_modrinth_file(version: &serde_json::Value) -> Option<&serde_json::Value> {
    let files = version.get("files")?.as_array()?;
    files
        .iter()
        .find(|file| file.get("primary").and_then(|primary| primary.as_bool()).unwrap_or(false))
        .or_else(|| files.first())
}

pub(crate) fn primary_modrinth_file_url(version: &serde_json::Value) -> Option<String> {
    primary_modrinth_file(version)?
        .get("url")?
        .as_str()
        .map(str::to_owned)
}

fn primary_modrinth_file_name(version: &serde_json::Value) -> Option<String> {
    primary_modrinth_file(version)?
        .get("filename")?
        .as_str()
        .map(str::to_owned)
}

/// SHA-1 of every file of a Modrinth version, lower-cased.
pub(crate) fn modrinth_file_sha1s(version: &serde_json::Value) -> Vec<String> {
    version
        .get("files")
        .and_then(|files| files.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file.get("hashes")?.get("sha1")?.as_str())
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default()
}

fn required_modrinth_dependencies(version: &serde_json::Value) -> Vec<serde_json::Value> {
    version
        .get("dependencies")
        .and_then(|dependencies| dependencies.as_array())
        .map(|dependencies| {
            dependencies
                .iter()
                .filter(|dependency| {
                    dependency
                        .get("dependency_type")
                        .and_then(|kind| kind.as_str())
                        == Some("required")
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// A Modrinth version is compatible only when it lists the exact Minecraft
/// version and the instance's loader. An unknown loader matches nothing:
/// guessing one would install a jar the instance cannot load.
fn version_is_compatible(
    version: &serde_json::Value,
    mc_version: &str,
    mod_loader: &str,
) -> bool {
    let mc_version = mc_version.trim();
    let supports_game_version = !mc_version.is_empty()
        && version
            .get("game_versions")
            .and_then(|versions| versions.as_array())
            .map(|versions| versions.iter().any(|version| version.as_str() == Some(mc_version)))
            .unwrap_or(false);
    if !supports_game_version {
        return false;
    }

    let Some(loader) = canonical_mod_loader(mod_loader) else {
        return false;
    };
    version
        .get("loaders")
        .and_then(|loaders| loaders.as_array())
        .map(|loaders| {
            loaders.iter().any(|candidate| {
                candidate
                    .as_str()
                    .map(|candidate| candidate.eq_ignore_ascii_case(loader))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn modrinth_channel_rank(version: &serde_json::Value) -> u8 {
    match version.get("version_type").and_then(|kind| kind.as_str()) {
        Some("release") => 0,
        Some("beta") => 1,
        Some("alpha") => 2,
        _ => 3,
    }
}

/// Chooses the version to install for a dependency: among versions that match
/// the Minecraft version and loader, a release always beats a beta, which beats
/// an alpha. Within a channel the newest `date_published` wins, then API order.
pub(crate) fn select_modrinth_version<'a>(
    versions: &'a [serde_json::Value],
    mc_version: &str,
    mod_loader: &str,
) -> Option<&'a serde_json::Value> {
    let published = |version: &serde_json::Value| {
        version
            .get("date_published")
            .and_then(|date| date.as_str())
            .unwrap_or("")
            .to_owned()
    };
    versions
        .iter()
        .enumerate()
        .filter(|(_, version)| {
            version_is_compatible(version, mc_version, mod_loader)
                && primary_modrinth_file_url(version).is_some()
        })
        .min_by(|(left_index, left), (right_index, right)| {
            modrinth_channel_rank(left)
                .cmp(&modrinth_channel_rank(right))
                .then_with(|| published(right).cmp(&published(left)))
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(_, version)| version)
}

async fn get_modrinth_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, String> {
    let response = get_with_retry(client, url, Some(RetryConfig {
        max_retries: 2,
        initial_delay_ms: 400,
        max_delay_ms: 1600,
    }))
    .await?;
    if !response.status().is_success() {
        return Err(format!("Modrinth API returned {} for {}", response.status(), url));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Failed to parse Modrinth response: {}", error))
}

/// Versions of a Modrinth project, narrowed server-side to the Minecraft
/// version and loader. `select_modrinth_version` re-checks both locally.
pub(crate) async fn fetch_modrinth_project_versions(
    client: &reqwest::Client,
    project: &str,
    mc_version: &str,
    mod_loader: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut params = Vec::new();
    let mc_version = mc_version.trim();
    if !mc_version.is_empty() {
        params.push(format!(
            "game_versions={}",
            urlencoding(&serde_json::json!([mc_version]).to_string()),
        ));
    }
    if let Some(loader) = canonical_mod_loader(mod_loader) {
        params.push(format!(
            "loaders={}",
            urlencoding(&serde_json::json!([loader]).to_string()),
        ));
    }
    let mut url = format!("https://api.modrinth.com/v2/project/{}/version", urlencoding(project));
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    let versions = get_modrinth_json(client, &url).await?;
    Ok(versions.as_array().cloned().unwrap_or_default())
}

pub(crate) async fn get_modrinth_project_identity(
    client: &reqwest::Client,
    project_id: &str,
) -> (String, String) {
    let fallback = project_id.to_string();
    let url = format!("https://api.modrinth.com/v2/project/{}", urlencoding(project_id));
    let Ok(project) = get_modrinth_json(client, &url).await else {
        return (fallback.clone(), fallback);
    };
    let slug = json_string(&project, "slug").unwrap_or_else(|| fallback.clone());
    let name = json_string(&project, "title").unwrap_or_else(|| slug.clone());
    (slug, name)
}

async fn resolve_modrinth_dependency_version(
    client: &reqwest::Client,
    dependency: &serde_json::Value,
    mc_version: &str,
    mod_loader: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut project_id = json_string(dependency, "project_id");

    // A pinned version is used only when it fits this instance; otherwise the
    // best version of the same project is chosen instead.
    if let Some(version_id) = json_string(dependency, "version_id") {
        let url = format!("https://api.modrinth.com/v2/version/{}", urlencoding(&version_id));
        match get_modrinth_json(client, &url).await {
            Ok(version)
                if version_is_compatible(&version, mc_version, mod_loader)
                    && primary_modrinth_file_url(&version).is_some() =>
            {
                return Ok(Some(version));
            }
            Ok(version) => {
                println!(
                    "[ModDeps] 前置固定版本 {} 不兼容 {} / {}，改为按项目选择版本",
                    version_id, mc_version, mod_loader
                );
                if project_id.is_none() {
                    project_id = json_string(&version, "project_id");
                }
            }
            Err(error) if project_id.is_none() => return Err(error),
            Err(error) => eprintln!("[ModDeps] 获取前置固定版本 {} 失败: {}", version_id, error),
        }
    }

    let Some(project_id) = project_id else {
        return Ok(None);
    };
    let versions = fetch_modrinth_project_versions(client, &project_id, mc_version, mod_loader).await?;
    Ok(select_modrinth_version(&versions, mc_version, mod_loader).cloned())
}

// ---------------------------------------------------------------------------
// Installed-mod detection
// ---------------------------------------------------------------------------

/// Loose identity key for mod ids and project slugs: `sodium-extra`,
/// `sodium_extra` and `Sodium Extra` all become `sodiumextra`.
pub(crate) fn normalize_mod_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// What a mods directory already contains. Used to avoid installing a second
/// copy of a mod while completing required dependencies: Fabric tolerates
/// duplicates, Forge and NeoForge refuse to start.
#[derive(Debug, Default, Clone)]
pub(crate) struct InstalledModIndex {
    sha1: HashSet<String>,
    file_names: HashSet<String>,
    mod_keys: HashSet<String>,
    modrinth_project_ids: HashSet<String>,
}

/// How a dependency candidate identifies itself before it is installed.
#[derive(Debug, Default, Clone)]
pub(crate) struct ModCandidateIdentity {
    pub modrinth_project_id: Option<String>,
    /// Project slugs or mod ids naming the candidate (compared loosely).
    pub names: Vec<String>,
    pub sha1: Vec<String>,
    pub file_name: Option<String>,
}

impl InstalledModIndex {
    /// Indexes every `.jar` in `dir`: file name, SHA-1 and declared mod ids.
    /// Disabled jars (`*.jar.disabled`) are not loaded and are ignored.
    pub(crate) fn scan(dir: &Path) -> Self {
        use rayon::prelude::*;

        let mut index = Self::default();
        let Ok(entries) = fs::read_dir(dir) else {
            return index;
        };
        let jars: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .map(|extension| extension.eq_ignore_ascii_case("jar"))
                        .unwrap_or(false)
            })
            .collect();
        let scanned: Vec<(String, Option<String>, Vec<String>)> = jars
            .par_iter()
            .map(|path| {
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                (file_name, file_sha1(path), jar_mod_ids(path))
            })
            .collect();
        for (file_name, sha1, mod_ids) in scanned {
            index.add_jar(&file_name, sha1.as_deref(), &mod_ids);
        }
        index
    }

    pub(crate) fn add_jar(&mut self, file_name: &str, sha1: Option<&str>, mod_ids: &[String]) {
        if !file_name.is_empty() {
            self.file_names.insert(file_name.to_ascii_lowercase());
        }
        if let Some(sha1) = sha1 {
            self.sha1.insert(sha1.to_ascii_lowercase());
        }
        self.add_names(mod_ids);
    }

    pub(crate) fn add_modrinth_project(&mut self, project_id: &str, slug: Option<&str>) {
        if !project_id.is_empty() {
            self.modrinth_project_ids.insert(project_id.to_string());
        }
        if let Some(slug) = slug {
            self.add_names(&[slug.to_string()]);
        }
    }

    fn add_names(&mut self, names: &[String]) {
        for name in names {
            let key = normalize_mod_key(name);
            if !key.is_empty() {
                self.mod_keys.insert(key);
            }
        }
    }

    pub(crate) fn sha1_hashes(&self) -> Vec<String> {
        let mut hashes: Vec<String> = self.sha1.iter().cloned().collect();
        hashes.sort();
        hashes
    }

    /// Why `candidate` is already installed, or `None` when it is not.
    pub(crate) fn find(&self, candidate: &ModCandidateIdentity) -> Option<String> {
        if let Some(hash) = candidate
            .sha1
            .iter()
            .find(|hash| self.sha1.contains(&hash.to_ascii_lowercase()))
        {
            return Some(format!("相同文件已存在 (sha1 {})", hash.chars().take(12).collect::<String>()));
        }
        if let Some(project_id) = candidate
            .modrinth_project_id
            .as_ref()
            .filter(|project_id| self.modrinth_project_ids.contains(*project_id))
        {
            return Some(format!("Modrinth 项目 {} 已安装", project_id));
        }
        if let Some(name) = candidate
            .names
            .iter()
            .find(|name| self.has_mod_key(name))
        {
            return Some(format!("已安装 mod id 为 {} 的模组", name));
        }
        if let Some(file_name) = candidate
            .file_name
            .as_ref()
            .filter(|file_name| self.file_names.contains(&file_name.to_ascii_lowercase()))
        {
            return Some(format!("同名文件已存在: {}", file_name));
        }
        None
    }

    pub(crate) fn has_mod_key(&self, name: &str) -> bool {
        let key = normalize_mod_key(name);
        !key.is_empty() && self.mod_keys.contains(&key)
    }

    /// The first of `mod_ids` (read from a freshly downloaded jar) that is
    /// already provided by an installed mod.
    pub(crate) fn conflicting_mod_id<'a>(&self, mod_ids: &'a [String]) -> Option<&'a str> {
        mod_ids
            .iter()
            .find(|mod_id| self.has_mod_key(mod_id))
            .map(String::as_str)
    }

    /// Records a candidate chosen for installation so that later candidates in
    /// the same run are deduplicated against it.
    pub(crate) fn record(&mut self, candidate: &ModCandidateIdentity) {
        for hash in &candidate.sha1 {
            self.sha1.insert(hash.to_ascii_lowercase());
        }
        if let Some(project_id) = &candidate.modrinth_project_id {
            self.modrinth_project_ids.insert(project_id.clone());
        }
        self.add_names(&candidate.names);
        if let Some(file_name) = &candidate.file_name {
            self.file_names.insert(file_name.to_ascii_lowercase());
        }
    }
}

pub(crate) fn file_sha1(path: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    use std::io::Read;

    let mut file = File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hex::encode(hasher.finalize()))
}

/// Mod ids a jar declares. The primary id comes from `mod_parser`
/// (fabric.mod.json, mods.toml, neoforge.mods.toml, mcmod.info); on top of that
/// this reads what `mod_parser` skips but duplicate detection needs:
/// quilt.mod.json, Fabric/Quilt `provides` and every `[[mods]]` entry.
pub(crate) fn jar_mod_ids(path: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(info) = crate::handler::mod_parser::parse_mod_file(path) {
        ids.push(info.mod_id);
    }
    if let Ok(file) = File::open(path) {
        if let Ok(mut archive) = ZipArchive::new(BufReader::new(file)) {
            if let Some(content) = read_zip_text(&mut archive, "fabric.mod.json") {
                ids.extend(fabric_mod_json_ids(&content));
            }
            if let Some(content) = read_zip_text(&mut archive, "quilt.mod.json") {
                ids.extend(quilt_mod_json_ids(&content));
            }
            for entry in ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
                if let Some(content) = read_zip_text(&mut archive, entry) {
                    ids.extend(mods_toml_mod_ids(&content));
                }
            }
        }
    }
    let mut seen = HashSet::new();
    ids.retain(|id| !id.trim().is_empty() && seen.insert(id.trim().to_ascii_lowercase()));
    ids
}

fn read_zip_text<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut content = String::new();
    std::io::Read::read_to_string(&mut entry, &mut content).ok()?;
    Some(content)
}

fn fabric_mod_json_ids(content: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = json_string(&json, "id").into_iter().collect();
    if let Some(provides) = json.get("provides").and_then(|provides| provides.as_array()) {
        ids.extend(provides.iter().filter_map(|id| id.as_str()).map(str::to_owned));
    }
    ids
}

fn quilt_mod_json_ids(content: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let Some(loader) = json.get("quilt_loader") else {
        return Vec::new();
    };
    let mut ids: Vec<String> = json_string(loader, "id").into_iter().collect();
    if let Some(provides) = loader.get("provides").and_then(|provides| provides.as_array()) {
        ids.extend(provides.iter().filter_map(|provided| {
            provided
                .as_str()
                .map(str::to_owned)
                .or_else(|| json_string(provided, "id"))
        }));
    }
    ids
}

/// `modId` of every `[[mods]]` table. `modId` keys inside `[[dependencies.*]]`
/// name required mods, not this jar, and are ignored.
fn mods_toml_mod_ids(content: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut in_mods_table = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            let header: String = line
                .split('#')
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            in_mods_table = header == "[[mods]]";
            continue;
        }
        if !in_mods_table {
            continue;
        }
        let Some(rest) = line.strip_prefix("modId") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        if let Some(end) = rest[1..].find(quote) {
            ids.push(rest[1..1 + end].to_string());
        }
    }
    ids
}

/// Adds the Modrinth projects of the indexed files (looked up by SHA-1). This
/// identifies installed jars exactly even when their mod id differs from the
/// project slug, and gives CurseForge candidates a slug to match against.
/// Network failures only make detection less precise; they never abort.
pub(crate) async fn enrich_installed_index_with_modrinth(
    client: &reqwest::Client,
    index: &mut InstalledModIndex,
) {
    let hashes = index.sha1_hashes();
    if hashes.is_empty() {
        return;
    }
    let mut project_ids: Vec<String> = Vec::new();
    for chunk in hashes.chunks(200) {
        let body = serde_json::json!({ "hashes": chunk, "algorithm": "sha1" });
        let response = client
            .post("https://api.modrinth.com/v2/version_files")
            .timeout(std::time::Duration::from_secs(20))
            .json(&body)
            .send()
            .await;
        let versions = match response {
            Ok(response) if response.status().is_success() => {
                match response.json::<serde_json::Value>().await {
                    Ok(versions) => versions,
                    Err(error) => {
                        eprintln!("[ModDeps] 解析 Modrinth 文件哈希查询结果失败: {}", error);
                        continue;
                    }
                }
            }
            Ok(response) => {
                eprintln!("[ModDeps] Modrinth 文件哈希查询返回 {}", response.status());
                continue;
            }
            Err(error) => {
                eprintln!("[ModDeps] Modrinth 文件哈希查询失败: {}", error);
                continue;
            }
        };
        if let Some(versions) = versions.as_object() {
            for version in versions.values() {
                if let Some(project_id) = json_string(version, "project_id") {
                    if !project_ids.contains(&project_id) {
                        project_ids.push(project_id);
                    }
                }
            }
        }
    }
    for project_id in &project_ids {
        index.add_modrinth_project(project_id, None);
    }
    for chunk in project_ids.chunks(100) {
        let ids = serde_json::json!(chunk).to_string();
        let url = format!("https://api.modrinth.com/v2/projects?ids={}", urlencoding(&ids));
        match get_modrinth_json(client, &url).await {
            Ok(projects) => {
                for project in projects.as_array().into_iter().flatten() {
                    if let Some(project_id) = json_string(project, "id") {
                        index.add_modrinth_project(&project_id, json_string(project, "slug").as_deref());
                    }
                }
            }
            Err(error) => eprintln!("[ModDeps] 获取已安装 Modrinth 项目信息失败: {}", error),
        }
    }
}

/// Scans `dir` off the async runtime and enriches it with Modrinth projects.
pub(crate) async fn load_installed_mod_index(dir: PathBuf) -> InstalledModIndex {
    let mut index = tokio::task::spawn_blocking(move || InstalledModIndex::scan(&dir))
        .await
        .unwrap_or_default();
    let client = modrinth_client().await;
    enrich_installed_index_with_modrinth(&client, &mut index).await;
    index
}

/// Index of the directory `download_resource_file` saves mods into for this
/// Minecraft version and loader, i.e. where the dependencies will land.
async fn installed_index_for_download_target(mc_version: &str, mod_loader: &str) -> InstalledModIndex {
    match get_resource_download_dir("mod", mc_version, mod_loader) {
        Ok(dir) => load_installed_mod_index(dir).await,
        Err(error) => {
            eprintln!("[ModDeps] 无法定位 Mod 下载目录，跳过已安装检查: {}", error);
            InstalledModIndex::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency walk
// ---------------------------------------------------------------------------

/// A dependency resolved to a concrete file, plus the references it requires.
pub(crate) struct ResolvedDependencyNode<D> {
    /// Stable project key such as `modrinth:AANobbMI` or `curseforge:394468`.
    pub key: String,
    pub identity: ModCandidateIdentity,
    pub dependency: ResolvedModDependency,
    pub requires: Vec<D>,
}

/// Breadth-first walk over required dependencies and their own requirements.
///
/// * `key_of` gives a reference's project key when it is known up front;
///   version-only references learn theirs from `resolve`.
/// * Each project is visited once, so dependency cycles terminate.
/// * A dependency already in `installed` (or picked earlier in this walk) is
///   skipped together with its own requirements.
pub(crate) async fn walk_required_dependencies<D, K, F, Fut>(
    roots: Vec<D>,
    root_key: Option<String>,
    installed: &mut InstalledModIndex,
    key_of: K,
    mut resolve: F,
) -> Result<Vec<ResolvedModDependency>, String>
where
    K: Fn(&D) -> Option<String>,
    F: FnMut(D) -> Fut,
    Fut: std::future::Future<Output = Result<Option<ResolvedDependencyNode<D>>, String>>,
{
    let mut queue: VecDeque<D> = roots.into();
    let mut seen: HashSet<String> = root_key.into_iter().collect();
    let mut resolved = Vec::new();

    while let Some(reference) = queue.pop_front() {
        if resolved.len() >= MAX_AUTO_RESOLVED_DEPENDENCIES {
            break;
        }
        let reference_key = key_of(&reference);
        if let Some(key) = &reference_key {
            if !seen.insert(key.clone()) {
                continue;
            }
        }
        let Some(node) = resolve(reference).await? else {
            continue;
        };
        if reference_key.as_deref() != Some(node.key.as_str()) && !seen.insert(node.key.clone()) {
            continue;
        }
        if let Some(reason) = installed.find(&node.identity) {
            println!(
                "[ModDeps] 跳过已安装的前置 {}: {}",
                node.dependency.project_name, reason
            );
            continue;
        }
        installed.record(&node.identity);
        queue.extend(node.requires);
        resolved.push(node.dependency);
    }

    Ok(resolved)
}

async fn resolve_modrinth_dependency_node(
    client: &reqwest::Client,
    dependency: serde_json::Value,
    mc_version: &str,
    mod_loader: &str,
) -> Result<Option<ResolvedDependencyNode<serde_json::Value>>, String> {
    let Some(version) =
        resolve_modrinth_dependency_version(client, &dependency, mc_version, mod_loader).await?
    else {
        return Ok(None);
    };
    let Some(download_url) = primary_modrinth_file_url(&version) else {
        return Ok(None);
    };
    let Some(project_id) =
        json_string(&version, "project_id").or_else(|| json_string(&dependency, "project_id"))
    else {
        return Ok(None);
    };
    let (project_slug, project_name) = get_modrinth_project_identity(client, &project_id).await;
    Ok(Some(ResolvedDependencyNode {
        key: format!("modrinth:{}", project_id),
        identity: ModCandidateIdentity {
            modrinth_project_id: Some(project_id.clone()),
            names: vec![project_slug.clone()],
            sha1: modrinth_file_sha1s(&version),
            file_name: primary_modrinth_file_name(&version),
        },
        requires: required_modrinth_dependencies(&version),
        dependency: ResolvedModDependency {
            project_id,
            project_slug,
            project_name,
            download_url,
        },
    }))
}

/// Resolve the selected Modrinth file's transitive `required` dependencies.
///
/// Dependencies already present in the download directory for this Minecraft
/// version and loader are left out. A URL that does not belong to Modrinth
/// deliberately returns an empty list: the selected CurseForge file cannot be
/// safely matched without its file ID.
#[allow(dependency_on_unit_never_type_fallback)]
#[tauri::command]
pub async fn get_modrinth_required_dependencies(
    project_slug: String,
    mc_version: String,
    mod_loader: String,
    download_url: String,
) -> Result<Vec<ResolvedModDependency>, String> {
    let project_slug = project_slug.trim();
    if project_slug.is_empty() || download_url.trim().is_empty() {
        return Ok(Vec::new());
    }

    let client = modrinth_client().await;
    let root_versions_url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        urlencoding(project_slug),
    );
    let root_versions = get_modrinth_json(&client, &root_versions_url).await?;
    let Some(root_version) = root_versions.as_array().and_then(|versions| {
        versions.iter().find(|version| {
            version
                .get("files")
                .and_then(|files| files.as_array())
                .map(|files| {
                    files.iter().any(|file| {
                        file.get("url").and_then(|url| url.as_str()) == Some(download_url.as_str())
                    })
                })
                .unwrap_or(false)
        })
    }) else {
        return Ok(Vec::new());
    };

    let mut installed = installed_index_for_download_target(&mc_version, &mod_loader).await;
    let walk_client = client.clone();
    walk_required_dependencies(
        required_modrinth_dependencies(root_version),
        json_string(root_version, "project_id").map(|id| format!("modrinth:{}", id)),
        &mut installed,
        |dependency: &serde_json::Value| {
            json_string(dependency, "project_id").map(|id| format!("modrinth:{}", id))
        },
        move |dependency: serde_json::Value| {
            let client = walk_client.clone();
            let mc_version = mc_version.clone();
            let mod_loader = mod_loader.clone();
            async move {
                resolve_modrinth_dependency_node(&client, dependency, &mc_version, &mod_loader).await
            }
        },
    )
    .await
}

pub(crate) fn curseforge_file_download_url(file: &serde_json::Value) -> Option<String> {
    if let Some(url) = json_string(file, "downloadUrl").filter(|url| !url.is_empty()) {
        return Some(url);
    }

    let file_id = file.get("id")?.as_i64()?;
    let file_name = json_string(file, "fileName")?;
    let first4 = file_id / 1000;
    let last3 = file_id % 1000;
    Some(format!(
        "https://edge.forgecdn.net/files/{}/{:03}/{}",
        first4,
        last3,
        urlencoding(&file_name),
    ))
}

fn curseforge_required_dependency_ids(file: &serde_json::Value) -> Vec<i64> {
    file.get("dependencies")
        .and_then(|dependencies| dependencies.as_array())
        .map(|dependencies| {
            dependencies
                .iter()
                // CurseForge FileRelationType::RequiredDependency is 3.
                .filter(|dependency| dependency.get("relationType").and_then(|kind| kind.as_i64()) == Some(3))
                .filter_map(|dependency| dependency.get("modId").and_then(|id| id.as_i64()))
                .collect()
        })
        .unwrap_or_default()
}

/// SHA-1 hashes of a CurseForge file (`HashAlgo::Sha1` is 1), lower-cased.
pub(crate) fn curseforge_file_sha1s(file: &serde_json::Value) -> Vec<String> {
    file.get("hashes")
        .and_then(|hashes| hashes.as_array())
        .map(|hashes| {
            hashes
                .iter()
                .filter(|hash| hash.get("algo").and_then(|algo| algo.as_i64()) == Some(1))
                .filter_map(|hash| hash.get("value").and_then(|value| value.as_str()))
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default()
}

fn curseforge_file_supports(
    file: &serde_json::Value,
    mc_version: &str,
    mod_loader: &str,
) -> bool {
    let Some(game_versions) = file.get("gameVersions").and_then(|versions| versions.as_array()) else {
        return false;
    };
    let mc_version = mc_version.trim();
    if mc_version.is_empty() || !game_versions.iter().any(|version| version.as_str() == Some(mc_version)) {
        return false;
    }

    let tagged_loaders: Vec<String> = game_versions
        .iter()
        .filter_map(|version| version.as_str())
        .map(|version| version.to_ascii_lowercase())
        .filter(|version| KNOWN_MOD_LOADERS.contains(&version.as_str()))
        .collect();

    // Files without a loader tag are legacy (mostly pre-1.14 Forge) or
    // loader-agnostic libraries, so they fit any loader. A tagged file must
    // carry the instance's loader; with an unknown loader only untagged files fit.
    match canonical_mod_loader(mod_loader) {
        Some(loader) => tagged_loaders.is_empty() || tagged_loaders.iter().any(|tag| tag == loader),
        None => tagged_loaders.is_empty(),
    }
}

/// Chooses the CurseForge file for a dependency: compatible with the Minecraft
/// version and loader, release before beta before alpha, newest `fileDate`
/// within a channel. Server packs and unavailable files are never picked.
pub(crate) fn select_curseforge_compatible_file(
    files: &[serde_json::Value],
    mc_version: &str,
    mod_loader: &str,
) -> Option<serde_json::Value> {
    for release_type in [1_i64, 2, 3] {
        let best = files
            .iter()
            .filter(|file| {
                file.get("releaseType").and_then(|kind| kind.as_i64()) == Some(release_type)
                    && !file.get("isServerPack").and_then(|flag| flag.as_bool()).unwrap_or(false)
                    && file.get("isAvailable").and_then(|flag| flag.as_bool()).unwrap_or(true)
                    && curseforge_file_supports(file, mc_version, mod_loader)
                    && curseforge_file_download_url(file).is_some()
            })
            .max_by(|left, right| {
                let date = |file: &serde_json::Value| json_string(file, "fileDate").unwrap_or_default();
                let id = |file: &serde_json::Value| file.get("id").and_then(|id| id.as_i64()).unwrap_or(0);
                date(left).cmp(&date(right)).then_with(|| id(left).cmp(&id(right)))
            });
        if let Some(file) = best {
            return Some(file.clone());
        }
    }
    None
}

async fn get_curseforge_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, String> {
    let response = get_with_retry(client, url, Some(RetryConfig {
        max_retries: 2,
        initial_delay_ms: 400,
        max_delay_ms: 1600,
    }))
    .await?;
    if !response.status().is_success() {
        return Err(format!("CurseForge API returned {} for {}", response.status(), url));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Failed to parse CurseForge response: {}", error))
}

/// Files of a CurseForge project, newest first. With `mc_version` the list is
/// filtered server-side, so older Minecraft versions are not cut off by paging.
pub(crate) async fn get_curseforge_files(
    client: &reqwest::Client,
    project_id: i64,
    mc_version: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let game_version_filter = mc_version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(|version| format!("&gameVersion={}", urlencoding(version)))
        .unwrap_or_default();
    let mut files = Vec::new();
    for page in 0..CURSEFORGE_MAX_FILE_PAGES {
        let url = format!(
            "https://api.curseforge.com/v1/mods/{}/files?pageSize={}&index={}{}",
            project_id,
            CURSEFORGE_FILES_PAGE_SIZE,
            page * CURSEFORGE_FILES_PAGE_SIZE,
            game_version_filter,
        );
        let response = get_curseforge_json(client, &url).await?;
        let page_files = response
            .get("data")
            .and_then(|files| files.as_array())
            .cloned()
            .unwrap_or_default();
        let page_len = page_files.len();
        files.extend(page_files);
        let total = response
            .get("pagination")
            .and_then(|pagination| pagination.get("totalCount"))
            .and_then(|total| total.as_u64())
            .map(|total| total as usize);
        if page_len < CURSEFORGE_FILES_PAGE_SIZE || total.map(|total| files.len() >= total).unwrap_or(false) {
            break;
        }
    }
    Ok(files)
}

async fn get_curseforge_project_by_slug(
    client: &reqwest::Client,
    slug: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&searchFilter={}",
        urlencoding(slug),
    );
    let response = get_curseforge_json(client, &url).await?;
    response
        .get("data")
        .and_then(|projects| projects.as_array())
        .and_then(|projects| {
            projects.iter().find(|project| {
                project.get("slug").and_then(|project_slug| project_slug.as_str())
                    .map(|project_slug| project_slug.eq_ignore_ascii_case(slug))
                    .unwrap_or(false)
            })
        })
        .cloned()
        .ok_or_else(|| format!("Project not found on CurseForge: {}", slug))
}

pub(crate) async fn get_curseforge_project_identity(
    client: &reqwest::Client,
    project_id: i64,
) -> (String, String) {
    let fallback = project_id.to_string();
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let Ok(response) = get_curseforge_json(client, &url).await else {
        return (fallback.clone(), fallback);
    };
    let project = response.get("data").unwrap_or(&response);
    let slug = json_string(project, "slug").unwrap_or_else(|| fallback.clone());
    let name = json_string(project, "name").unwrap_or_else(|| slug.clone());
    (slug, name)
}

async fn resolve_curseforge_dependency_node(
    client: &reqwest::Client,
    project_id: i64,
    mc_version: &str,
    mod_loader: &str,
) -> Result<Option<ResolvedDependencyNode<i64>>, String> {
    let files = get_curseforge_files(client, project_id, Some(mc_version)).await?;
    let Some(file) = select_curseforge_compatible_file(&files, mc_version, mod_loader) else {
        return Ok(None);
    };
    let Some(download_url) = curseforge_file_download_url(&file) else {
        return Ok(None);
    };
    let (project_slug, project_name) = get_curseforge_project_identity(client, project_id).await;
    Ok(Some(ResolvedDependencyNode {
        key: format!("curseforge:{}", project_id),
        identity: ModCandidateIdentity {
            modrinth_project_id: None,
            names: vec![project_slug.clone()],
            sha1: curseforge_file_sha1s(&file),
            file_name: json_string(&file, "fileName"),
        },
        requires: curseforge_required_dependency_ids(&file),
        dependency: ResolvedModDependency {
            project_id: project_id.to_string(),
            project_slug,
            project_name,
            download_url,
        },
    }))
}

/// Resolve the selected CurseForge file's transitive required dependencies.
/// CurseForge identifies required files by project rather than file ID, so the
/// best compatible file (release first) is selected for each dependency
/// project. Dependencies already present in the download directory for this
/// Minecraft version and loader are left out.
#[allow(dependency_on_unit_never_type_fallback)]
#[tauri::command]
pub async fn get_curseforge_required_dependencies(
    project_slug: String,
    mc_version: String,
    mod_loader: String,
    download_url: String,
) -> Result<Vec<ResolvedModDependency>, String> {
    let project_slug = project_slug.trim();
    if project_slug.is_empty() || download_url.trim().is_empty() {
        return Ok(Vec::new());
    }

    let client = curseforge_client().await;
    let root_project = get_curseforge_project_by_slug(&client, project_slug).await?;
    let root_project_id = root_project
        .get("id")
        .and_then(|id| id.as_i64())
        .ok_or_else(|| format!("CurseForge project {} has no ID", project_slug))?;
    let matches_selected = |file: &serde_json::Value| {
        curseforge_file_download_url(file).as_deref() == Some(download_url.as_str())
    };
    let mut root_file = get_curseforge_files(&client, root_project_id, Some(mc_version.as_str()))
        .await?
        .into_iter()
        .find(|file| matches_selected(file));
    if root_file.is_none() {
        root_file = get_curseforge_files(&client, root_project_id, None)
            .await?
            .into_iter()
            .find(|file| matches_selected(file));
    }
    let Some(root_file) = root_file else {
        return Ok(Vec::new());
    };

    let mut installed = installed_index_for_download_target(&mc_version, &mod_loader).await;
    let walk_client = client.clone();
    walk_required_dependencies(
        curseforge_required_dependency_ids(&root_file),
        Some(format!("curseforge:{}", root_project_id)),
        &mut installed,
        |project_id: &i64| Some(format!("curseforge:{}", project_id)),
        move |project_id: i64| {
            let client = walk_client.clone();
            let mc_version = mc_version.clone();
            let mod_loader = mod_loader.clone();
            async move {
                resolve_curseforge_dependency_node(&client, project_id, &mc_version, &mod_loader).await
            }
        },
    )
    .await
}

async fn get_mod_files(mod_id: &str) -> Result<HashMap<String, Vec<(Vec<String>, String)>>, Box<dyn std::error::Error>> {
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

#[cfg(test)]
mod dependency_resolution_tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn modrinth_version(
        number: &str,
        kind: &str,
        game_versions: &[&str],
        loaders: &[&str],
        published: &str,
    ) -> serde_json::Value {
        json!({
            "id": format!("id-{}", number),
            "project_id": "AANobbMI",
            "version_number": number,
            "version_type": kind,
            "game_versions": game_versions,
            "loaders": loaders,
            "date_published": published,
            "files": [{
                "url": format!("https://cdn.modrinth.com/{}.jar", number),
                "filename": format!("{}.jar", number),
                "primary": true,
                "hashes": { "sha1": format!("SHA-{}", number) }
            }]
        })
    }

    fn selected_number(versions: &[serde_json::Value], mc: &str, loader: &str) -> Option<String> {
        select_modrinth_version(versions, mc, loader)
            .and_then(|version| json_string(version, "version_number"))
    }

    #[test]
    fn release_is_preferred_over_newer_beta() {
        // Mirrors Sodium for Fabric 1.21.1 in the E2E run: the two newest
        // versions are betas, the auto-installer must still pick 0.8.13.
        let versions = vec![
            modrinth_version("0.8.14-beta.2", "beta", &["1.21.1"], &["fabric"], "2025-11-20T00:00:00Z"),
            modrinth_version("0.8.14-beta.1", "beta", &["1.21.1"], &["fabric"], "2025-11-10T00:00:00Z"),
            modrinth_version("0.8.13", "release", &["1.21.1"], &["fabric"], "2025-11-01T00:00:00Z"),
            modrinth_version("0.8.12", "release", &["1.21.1"], &["fabric"], "2025-10-01T00:00:00Z"),
        ];
        assert_eq!(selected_number(&versions, "1.21.1", "Fabric").as_deref(), Some("0.8.13"));
    }

    #[test]
    fn beta_is_used_only_without_a_matching_release() {
        // Releases exist, but not for Fabric on 1.21.1: the beta wins over the
        // alpha, and the releases of other loaders / versions stay excluded.
        let versions = vec![
            modrinth_version("2.0-alpha", "alpha", &["1.21.1"], &["fabric"], "2025-12-01T00:00:00Z"),
            modrinth_version("1.9-beta", "beta", &["1.21.1"], &["fabric"], "2025-11-01T00:00:00Z"),
            modrinth_version("1.8-neoforge", "release", &["1.21.1"], &["neoforge"], "2025-12-02T00:00:00Z"),
            modrinth_version("1.8-1.21.4", "release", &["1.21.4"], &["fabric"], "2025-12-03T00:00:00Z"),
        ];
        assert_eq!(selected_number(&versions, "1.21.1", "fabric").as_deref(), Some("1.9-beta"));
        assert_eq!(selected_number(&versions, "1.21.1", "neoforge").as_deref(), Some("1.8-neoforge"));
    }

    #[test]
    fn wrong_loader_or_minecraft_version_is_never_selected() {
        let versions = vec![
            modrinth_version("forge-build", "release", &["1.21.1"], &["forge"], "2025-01-01T00:00:00Z"),
            modrinth_version("other-mc", "release", &["1.21", "1.21.2"], &["fabric"], "2025-01-01T00:00:00Z"),
        ];
        assert_eq!(selected_number(&versions, "1.21.1", "fabric"), None);
        assert_eq!(selected_number(&versions, "1.21.1", "neoforge"), None);
        // An unknown loader cannot be matched, so nothing is guessed.
        assert_eq!(selected_number(&versions, "1.21.1", "universal"), None);
        assert_eq!(selected_number(&versions, "1.21.1", ""), None);
    }

    #[test]
    fn newest_release_wins_within_the_channel() {
        let versions = vec![
            modrinth_version("older", "release", &["1.20.1"], &["forge"], "2024-01-01T00:00:00Z"),
            modrinth_version("newer", "release", &["1.20.1"], &["forge"], "2024-06-01T00:00:00Z"),
        ];
        assert_eq!(selected_number(&versions, "1.20.1", "forge").as_deref(), Some("newer"));
    }

    #[test]
    fn loader_names_are_canonicalized() {
        assert_eq!(canonical_mod_loader("NeoForge"), Some("neoforge"));
        assert_eq!(canonical_mod_loader("1.21.1-neoforge-21.1.267"), Some("neoforge"));
        assert_eq!(canonical_mod_loader("fabricloader"), Some("fabric"));
        assert_eq!(canonical_mod_loader("e2e-fabric-1.21.1"), Some("fabric"));
        assert_eq!(canonical_mod_loader("quilt_loader"), Some("quilt"));
        assert_eq!(canonical_mod_loader("Forge"), Some("forge"));
        assert_eq!(canonical_mod_loader("universal"), None);
        assert_eq!(canonical_mod_loader("Vanilla"), None);
    }

    fn curseforge_file(id: i64, release_type: i64, versions: &[&str], date: &str) -> serde_json::Value {
        json!({
            "id": id,
            "fileName": format!("file-{}.jar", id),
            "downloadUrl": format!("https://edge.forgecdn.net/files/{}.jar", id),
            "releaseType": release_type,
            "gameVersions": versions,
            "fileDate": date,
            "hashes": [{ "value": format!("SHA{}", id), "algo": 1 }, { "value": "md5", "algo": 2 }]
        })
    }

    #[test]
    fn curseforge_prefers_release_for_the_matching_loader() {
        let mut server_pack = curseforge_file(5, 1, &["1.20.1", "Forge"], "2025-05-01T00:00:00Z");
        server_pack["isServerPack"] = json!(true);
        let files = vec![
            curseforge_file(1, 2, &["1.20.1", "Forge"], "2025-04-01T00:00:00Z"),
            curseforge_file(2, 1, &["1.20.1", "Fabric"], "2025-03-01T00:00:00Z"),
            curseforge_file(3, 1, &["1.20.1", "Forge"], "2025-02-01T00:00:00Z"),
            curseforge_file(4, 1, &["1.19.2", "Forge"], "2025-06-01T00:00:00Z"),
            server_pack,
        ];
        let picked = select_curseforge_compatible_file(&files, "1.20.1", "forge").unwrap();
        assert_eq!(picked["id"], 3);
        assert_eq!(curseforge_file_sha1s(&picked), vec!["sha3".to_string()]);

        // Only a beta exists for NeoForge: it is used as the fallback.
        let neo_files = vec![
            curseforge_file(6, 2, &["1.21.1", "NeoForge"], "2025-04-01T00:00:00Z"),
            curseforge_file(7, 1, &["1.21.1", "Fabric"], "2025-05-01T00:00:00Z"),
        ];
        let picked = select_curseforge_compatible_file(&neo_files, "1.21.1", "NeoForge").unwrap();
        assert_eq!(picked["id"], 6);
        assert!(select_curseforge_compatible_file(&neo_files, "1.20.1", "neoforge").is_none());
    }

    #[test]
    fn installed_index_matches_hash_project_and_mod_id() {
        let mut index = InstalledModIndex::default();
        index.add_jar("sodium-fabric-0.8.13+mc1.21.1.jar", Some("003C114C"), &["sodium".into(), "indium".into()]);
        index.add_modrinth_project("P7dR8mSH", Some("fabric-api"));

        let same_file = ModCandidateIdentity { sha1: vec!["003c114c".into()], ..Default::default() };
        assert!(index.find(&same_file).is_some());

        let same_project = ModCandidateIdentity {
            modrinth_project_id: Some("P7dR8mSH".into()),
            names: vec!["something-else".into()],
            ..Default::default()
        };
        assert!(index.find(&same_project).is_some());

        // A newer Sodium build: other hash and file name, same mod id.
        let newer_sodium = ModCandidateIdentity {
            modrinth_project_id: Some("AANobbMI".into()),
            names: vec!["sodium".into()],
            sha1: vec!["a9b9ad08".into()],
            file_name: Some("sodium-fabric-0.8.14-beta.2+mc1.21.1.jar".into()),
        };
        assert!(index.find(&newer_sodium).is_some());
        // Ids an installed mod `provides` count as installed too.
        assert!(index.has_mod_key("Indium"));

        let sodium_extra = ModCandidateIdentity {
            modrinth_project_id: Some("PtjYWJkn".into()),
            names: vec!["sodium-extra".into()],
            sha1: vec!["7a4eddd9".into()],
            file_name: Some("sodium-extra-fabric-0.9.5+mc1.21.1.jar".into()),
        };
        assert_eq!(index.find(&sodium_extra), None);

        index.record(&sodium_extra);
        assert!(index.find(&sodium_extra).is_some());
        assert_eq!(index.conflicting_mod_id(&["sodium_extra".into()]), Some("sodium_extra"));
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rtl-mod-deps-{}-{}-{}", name, std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_jar(dir: &Path, name: &str, entries: &[(&str, &str)]) {
        let file = File::create(dir.join(name)).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (entry, content) in entries {
            writer.start_file(*entry, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn scan_reads_fabric_quilt_and_forge_metadata() {
        let dir = unique_temp_dir("scan");
        write_jar(&dir, "sodium-fabric.jar", &[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"sodium","version":"0.8.13","provides":["indium"]}"#,
        )]);
        write_jar(&dir, "qsl.jar", &[(
            "quilt.mod.json",
            r#"{"quilt_loader":{"id":"quilted_fabric_api","provides":["fabric-api",{"id":"fabric"}]}}"#,
        )]);
        write_jar(&dir, "neo-pack.jar", &[(
            "META-INF/neoforge.mods.toml",
            "modLoader=\"javafml\"\n[[mods]]\nmodId=\"firstmod\"\n[[mods]] # second\nmodId = 'secondmod'\n[[dependencies.firstmod]]\nmodId=\"geckolib\"\ntype=\"required\"\n",
        )]);
        write_jar(&dir, "disabled.jar.disabled", &[("fabric.mod.json", r#"{"id":"disabledmod"}"#)]);

        let index = InstalledModIndex::scan(&dir);
        for mod_id in ["sodium", "indium", "quilted_fabric_api", "fabric-api", "fabric", "firstmod", "secondmod"] {
            assert!(index.has_mod_key(mod_id), "{} should be indexed", mod_id);
        }
        // Dependencies declared in mods.toml are not installed mods.
        assert!(!index.has_mod_key("geckolib"));
        // Disabled jars are not loaded by the game.
        assert!(!index.has_mod_key("disabledmod"));
        assert_eq!(index.sha1_hashes().len(), 3);
        assert_eq!(
            index.sha1_hashes().contains(&file_sha1(&dir.join("qsl.jar")).unwrap()),
            true
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mods_toml_ids_skip_dependency_tables() {
        let content = "[[mods]]\nmodId=\"create\"\n[[dependencies.create]]\nmodId=\"flywheel\"\n[mods.extra]\nmodId=\"not-a-mod\"\n";
        assert_eq!(mods_toml_mod_ids(content), vec!["create".to_string()]);
    }

    fn graph_node(reference: &str) -> Option<ResolvedDependencyNode<String>> {
        let (project, requires): (&str, &[&str]) = match reference {
            "A" => ("A", &["ver:C", "ROOT"]),
            "B" => ("B", &["ver:A", "D"]),
            "ver:C" => ("C", &["A"]),
            "D" => ("D", &["E"]),
            "E" => ("E", &[]),
            "ROOT" => ("ROOT", &["A"]),
            _ => return None,
        };
        Some(ResolvedDependencyNode {
            key: format!("modrinth:{}", project),
            identity: ModCandidateIdentity {
                modrinth_project_id: Some(project.to_string()),
                names: vec![format!("{}-mod", project.to_ascii_lowercase())],
                ..Default::default()
            },
            dependency: ResolvedModDependency {
                project_id: project.to_string(),
                project_slug: project.to_ascii_lowercase(),
                project_name: project.to_string(),
                download_url: format!("https://example.invalid/{}.jar", project),
            },
            requires: requires.iter().map(|r| r.to_string()).collect(),
        })
    }

    #[test]
    fn walk_recurses_breaks_cycles_and_skips_installed() {
        let mut installed = InstalledModIndex::default();
        // D is already installed, so neither D nor its own requirement E is fetched.
        installed.add_jar("d.jar", None, &["d-mod".to_string()]);
        let mut resolve_calls: Vec<String> = Vec::new();

        let resolved = futures::executor::block_on(walk_required_dependencies(
            vec!["A".to_string(), "B".to_string()],
            Some("modrinth:ROOT".to_string()),
            &mut installed,
            |reference: &String| {
                (!reference.starts_with("ver:")).then(|| format!("modrinth:{}", reference))
            },
            |reference: String| {
                resolve_calls.push(reference.clone());
                std::future::ready(Ok(graph_node(&reference)))
            },
        ))
        .unwrap();

        let names: Vec<&str> = resolved.iter().map(|dep| dep.project_id.as_str()).collect();
        // C is the dependency of a dependency and is still installed.
        assert_eq!(names, vec!["A", "B", "C"]);
        // The cycles A -> ROOT and C -> A stop at already visited projects;
        // "ver:A" is resolved once to learn its project, then dropped.
        assert_eq!(resolve_calls, vec!["A", "B", "ver:C", "ver:A", "D"]);
        assert!(installed.find(&graph_node("B").unwrap().identity).is_some());
    }
}
