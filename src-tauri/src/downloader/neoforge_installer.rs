use crate::downloader::{
    concurrent_download::{self, DownloadTask},
    mod_loader_installer_shared as shared,
};
use crate::http_client::shared_client;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Debug, Deserialize, Clone)]
struct NeoForgeBmclEntry {
    #[serde(default)]
    version: String,
    #[serde(default)]
    build: Option<String>,
}
async fn fetch_bmcl_neoforge_versions(mc_version: &str) -> Option<Vec<String>> {
    let client = shared_client().await;
    let url = format!(
        "https://bmclapi2.bangbang93.com/forge/neo/{}/versions",
        mc_version
    );
    let text = client.get(&url).send().await.ok()?.text().await.ok()?;
    if let Ok(list) = serde_json::from_str::<Vec<NeoForgeBmclEntry>>(&text) {
        if !list.is_empty() {
            let mut versions: Vec<String> = list
                .into_iter()
                .map(|e| e.build.unwrap_or(e.version))
                .filter(|version| neoforge_version_matches_mc(mc_version, version))
                .collect();
            if !versions.is_empty() {
                versions.sort_by(|a, b| b.cmp(a));
                versions.dedup();
                return Some(versions);
            }
        }
    }
    None
}
/// NeoForge meta v2 返回的单条版本记录
#[derive(Debug, Deserialize, Clone)]
struct NeoForgeMetaV2Entry {
    #[serde(default)]
    mc_version: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    neoforgeVersion: Option<String>,
    #[serde(default)]
    recommended: Option<bool>,
    #[serde(default)]
    latest: Option<bool>,
}
#[derive(Debug, Deserialize, Clone)]
struct NeoForgeMetaV2Response {
    #[serde(default)]
    versions: Option<Vec<NeoForgeMetaV2Entry>>,
    #[serde(default)]
    latest: Option<serde_json::Value>,
}
/// NeoForge meta v1 回退结构
#[derive(Debug, Deserialize, Clone)]
struct NeoForgeMetaV1Response {
    #[serde(default)]
    latest: Option<serde_json::Value>,
}
/// NeoForge maven 版本查询返回的结构
#[derive(Debug, Deserialize, Clone)]
struct NeoForgeMavenVersions {
    #[serde(default)]
    versions: Option<Vec<String>>,
}

fn neoforge_version_matches_mc(mc_version: &str, neoforge_version: &str) -> bool {
    let legacy_prefix = format!("{}-", mc_version);
    if neoforge_version.starts_with(&legacy_prefix) {
        return true;
    }

    let parts: Vec<&str> = mc_version.split('.').collect();
    let short_prefix = if parts.first() == Some(&"1") {
        let Some(minor) = parts.get(1) else {
            return false;
        };
        let patch = parts.get(2).copied().unwrap_or("0");
        format!("{}.{}.", minor, patch)
    } else {
        format!("{}.", mc_version)
    };

    neoforge_version.starts_with(&short_prefix)
}

#[cfg(test)]
mod tests {
    use super::neoforge_version_matches_mc;

    #[test]
    fn matches_short_neoforge_versions_to_minecraft() {
        assert!(neoforge_version_matches_mc("1.20.2", "20.2.93"));
        assert!(neoforge_version_matches_mc("1.21", "21.0.167"));
        assert!(neoforge_version_matches_mc("1.21.1", "21.1.215"));
        assert!(neoforge_version_matches_mc("26.2", "26.2.0.40-beta"));
        assert!(neoforge_version_matches_mc("26.1.2", "26.1.2.93"));
    }

    #[test]
    fn rejects_versions_for_other_minecraft_releases() {
        assert!(!neoforge_version_matches_mc("1.20.2", "26.2.0.40-beta"));
        assert!(!neoforge_version_matches_mc("1.21.1", "21.0.167"));
    }

    #[test]
    fn keeps_legacy_minecraft_prefixed_versions() {
        assert!(neoforge_version_matches_mc("1.20.1", "1.20.1-47.1.106"));
    }
}

async fn fetch_official_neoforge_versions(mc_version: &str) -> Result<Vec<String>> {
    let client = shared_client().await;

    // 候选 URL 列表：按优先级逐个尝试
    // 1. v2 meta，按 mc 版本过滤
    // 2. v2 meta，无过滤
    // 3. v1 meta
    // 4. NeoForge maven versions 直查
    let v2_filtered = format!(
        "https://meta.neoforged.net/v2/versions/neo-forge?gameVersion={}",
        mc_version
    );
    let v2_all = "https://meta.neoforged.net/v2/versions/neo-forge".to_string();
    let v1_latest = "https://meta.neoforged.net/v1/versions/neo-forge".to_string();
    let maven_url =
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
            .to_string();
    let urls = [v2_filtered, v2_all, v1_latest, maven_url];

    for (idx, url) in urls.iter().enumerate() {
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let status = resp.status();
        if !status.is_success() {
            continue;
        }
        let text = match resp.text().await {
            Ok(t) => t,
            Err(_) => continue,
        };

        // 按不同 URL 解析
        match idx {
            // v2 meta（带过滤或全量）
            0 | 1 => {
                if let Ok(v2) = serde_json::from_str::<NeoForgeMetaV2Response>(&text) {
                    let mut versions: Vec<String> = Vec::new();
                    // 优先使用 versions 列表（按 mc 版本过滤）
                    if let Some(list) = &v2.versions {
                        for entry in list {
                            // 如果是按 mc_version 查询（idx==0），所有结果都应匹配；
                            // 但为了保险还是做一次过滤
                            let matches = match &entry.mc_version {
                                Some(v) => v == mc_version,
                                None => idx == 0,
                            };
                            if !matches {
                                continue;
                            }
                            if let Some(v) = &entry.neoforgeVersion {
                                versions.push(v.clone());
                            } else if let Some(v) = &entry.version {
                                versions.push(v.clone());
                            }
                        }
                    }
                    // 回退：从 latest 字段取
                    if versions.is_empty() {
                        if let Some(obj) = &v2.latest {
                            if let Some(map) = obj.as_object() {
                                if let Some(v) = map.get(mc_version).and_then(|v| v.as_str()) {
                                    versions.push(v.to_string());
                                } else {
                                    for (k, v) in map {
                                        if k.starts_with(mc_version) {
                                            if let Some(s) = v.as_str() {
                                                versions.push(s.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !versions.is_empty() {
                        versions.sort_by(|a, b| b.cmp(a));
                        versions.dedup();
                        return Ok(versions);
                    }
                }
                // 回退：尝试直接解析为数组（v1 老格式）
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&text) {
                    if !list.is_empty() {
                        let mut versions = list;
                        versions.sort_by(|a, b| b.cmp(a));
                        versions.dedup();
                        return Ok(versions);
                    }
                }
            }
            // v1 latest meta
            2 => {
                if let Ok(v1) = serde_json::from_str::<NeoForgeMetaV1Response>(&text) {
                    if let Some(obj) = &v1.latest {
                        if let Some(map) = obj.as_object() {
                            if let Some(v) = map.get(mc_version).and_then(|v| v.as_str()) {
                                return Ok(vec![v.to_string()]);
                            }
                            let mut versions: Vec<String> = map
                                .iter()
                                .filter(|(key, _)| key.as_str() == mc_version)
                                .filter_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                                .collect();
                            if !versions.is_empty() {
                                versions.sort_by(|a, b| b.cmp(a));
                                versions.dedup();
                                return Ok(versions);
                            }
                        }
                    }
                }
            }
            // maven 直查
            3 => {
                if let Ok(mv) = serde_json::from_str::<NeoForgeMavenVersions>(&text) {
                    if let Some(list) = &mv.versions {
                        // maven 版本形如 "21.0.144"，需要按 Minecraft 版本过滤。
                        // NeoForge 版本格式：
                        //   - 1.20.1 时代："{mc_version}-{build}"（例如 "1.20.1-47.3.7"）
                        //   - 之后使用简短版本号（例如 "21.0.144"）
                        let mut filtered: Vec<String> = list
                            .iter()
                            .filter(|v| neoforge_version_matches_mc(mc_version, v))
                            .cloned()
                            .collect();
                        if filtered.is_empty() {
                            continue;
                        }
                        filtered.sort_by(|a, b| b.cmp(a));
                        filtered.dedup();
                        if !filtered.is_empty() {
                            // 限制最多 50 个，避免 UI 过多
                            filtered.truncate(50);
                            return Ok(filtered);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // 所有官方源都失败了
    Err(anyhow!(
        "所有 NeoForge 版本查询源均不可用（BMCL + meta + maven），\
         Minecraft 版本：{}，请检查网络连接。",
        mc_version
    ))
}
pub async fn get_neoforge_versions(mc_version: &str) -> Result<Vec<String>> {
    if let Some(v) = fetch_bmcl_neoforge_versions(mc_version).await {
        return Ok(v);
    }
    fetch_official_neoforge_versions(mc_version).await
}
async fn download_installer(neoforge_version: &str, mc_dir: &PathBuf) -> Result<PathBuf> {
    let cache_dir = mc_dir.join("cache").join("neoforge_installer");
    std::fs::create_dir_all(&cache_dir).ok();
    let file_name = format!("neoforge-{}-installer.jar", neoforge_version);
    let target_path = cache_dir.join(&file_name);
    if target_path.exists() {
        return Ok(target_path);
    }
    // NeoForge 有两种 Maven 路径格式：
    //   - 新版（简化版本号，如 20.x.x / 21.x.x / 26.x.x）：
    //     net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar
    //   - 旧版（1.20.1 时代带 MC 前缀，如 1.20.1-47.x.x）：
    //     net/neoforged/forge/{version}/forge-{version}-installer.jar
    // 两个官方源 + 两个 BMCL 镜像都尝试，确保总有一个可用
    let urls = vec![
        format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
            neoforge_version, neoforge_version
        ),
        format!(
            "https://maven.neoforged.net/releases/net/neoforged/forge/{}/forge-{}-installer.jar",
            neoforge_version, neoforge_version
        ),
        format!(
            "https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
            neoforge_version, neoforge_version
        ),
        format!(
            "https://bmclapi2.bangbang93.com/maven/net/neoforged/forge/{}/forge-{}-installer.jar",
            neoforge_version, neoforge_version
        ),
    ];
    let task = DownloadTask {
        file_name: file_name.clone(),
        target_dir: cache_dir,
        urls,
        sha1: None,
    };
    concurrent_download::download_one(task)
        .await
        .with_context(|| format!("下载 NeoForge Installer JAR 失败: {}", file_name))
}
pub async fn install_neoforge(
    mc_version: &str,
    neoforge_version: &str,
    mc_dir: &str,
    java_path: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<f64>>,
    wait_for_original: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<String> {
    let mc_path = PathBuf::from(mc_dir);
    let installer_jar = download_installer(neoforge_version, &mc_path).await?;
    let java_executable = if java_path.is_empty() {
        let auto = shared::pick_java_executable(mc_version);
        println!("[NeoForge] 自动探测 Java: {} (java_path 为空)", auto);
        auto
    } else {
        java_path.to_string()
    };
    let cfg = shared::LoaderInstallerConfig {
        installer_jar_path: installer_jar,
        java_executable_path: PathBuf::from(java_executable),
        mc_version: mc_version.to_string(),
        mc_version_id: mc_version.to_string(),
        library_mirrors: vec![
            "https://maven.neoforged.net/releases/".to_string(),
            "https://bmclapi2.bangbang93.com/maven/".to_string(),
            "https://files.minecraftforge.net/maven/".to_string(),
            "https://libraries.minecraft.net/".to_string(),
        ],
    };
    shared::install(&cfg, &mc_path, progress_tx, wait_for_original).await
}