use crate::downloader::{
    concurrent_download::{self, DownloadTask},
    forge_installer::sort_loader_versions_desc,
    mod_loader_installer_shared as shared,
};
use crate::http_client::shared_client;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;

/// NeoForge 官方 maven（Reposilite）的版本列表 API，后面拼 artifact 名（neoforge / forge）。
const NEOFORGE_MAVEN_VERSIONS_API: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged";

/// NeoForge 在 Minecraft 1.20.1 上沿用 Forge 的坐标 `net.neoforged:forge`，
/// 版本号形如 `1.20.1-47.1.106`；从 1.20.2 起才改为 `net.neoforged:neoforge`（如 `20.2.86`）。
const LEGACY_NEOFORGE_MC_VERSIONS: &[&str] = &["1.20.1"];

fn is_legacy_neoforge_mc(mc_version: &str) -> bool {
    LEGACY_NEOFORGE_MC_VERSIONS.contains(&mc_version)
}

/// 查询某个 MC 版本的 NeoForge 列表时需要访问的 maven artifact。
fn neoforge_maven_artifacts(mc_version: &str) -> &'static [&'static str] {
    if is_legacy_neoforge_mc(mc_version) {
        &["forge"]
    } else {
        &["neoforge"]
    }
}

/// 获取 NeoForge 版本列表失败的原因，用来给出准确的提示。
#[derive(Debug)]
pub enum NeoForgeVersionListError {
    /// 查询源正常返回了，但其中没有适用于该 MC 版本的 NeoForge。
    NoVersionsForMc { mc_version: String },
    /// 权威查询源（官方 maven）请求失败、返回错误状态或响应无法解析。
    SourcesUnavailable {
        mc_version: String,
        details: Vec<String>,
    },
}

impl fmt::Display for NeoForgeVersionListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVersionsForMc { mc_version } => write!(
                f,
                "NeoForge 官方仓库中没有适用于 Minecraft {} 的版本。\
                 NeoForge 最早支持 1.20.1，较新的 Minecraft 版本也可能还没有发布对应的 NeoForge。",
                mc_version
            ),
            Self::SourcesUnavailable {
                mc_version,
                details,
            } => write!(
                f,
                "无法连接 NeoForge 版本查询源（Minecraft {}），请检查网络连接后重试。详情：{}",
                mc_version,
                details.join("；")
            ),
        }
    }
}

impl std::error::Error for NeoForgeVersionListError {}

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
    let list = serde_json::from_str::<Vec<NeoForgeBmclEntry>>(&text).ok()?;
    let versions = filter_and_sort_neoforge_versions(
        mc_version,
        list.into_iter().map(|e| e.build.unwrap_or(e.version)),
    );
    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}
/// NeoForge meta v2 返回的单条版本记录
#[derive(Debug, Deserialize, Clone)]
#[allow(non_snake_case, dead_code)]
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

/// 计算新版 NeoForge 版本号对应某个 MC 版本时的前缀：
/// - MC 1.x.y：NeoForge 版本为 `x.y.build`，如 1.21.1 → `21.1.`，1.21 → `21.0.`；
/// - MC 26.x 起：NeoForge 版本为 MC 版本补齐三段再加构建号，如 26.1 → `26.1.0.`，
///   26.1.2 → `26.1.2.`（这样 26.1 不会混入 26.1.1 / 26.1.2 的版本）。
fn neoforge_modern_prefix(mc_version: &str) -> Option<String> {
    let parts: Vec<&str> = mc_version.split('.').collect();
    if parts.first() == Some(&"1") {
        let minor = parts.get(1)?;
        let patch = parts.get(2).copied().unwrap_or("0");
        Some(format!("{}.{}.", minor, patch))
    } else {
        let major = parts.first().filter(|s| !s.is_empty())?;
        let minor = parts.get(1).copied().unwrap_or("0");
        let patch = parts.get(2).copied().unwrap_or("0");
        Some(format!("{}.{}.{}.", major, minor, patch))
    }
}

fn neoforge_version_matches_mc(mc_version: &str, neoforge_version: &str) -> bool {
    // 1.20.1 旧坐标：`1.20.1-47.1.106`
    let legacy_prefix = format!("{}-", mc_version);
    if neoforge_version.starts_with(&legacy_prefix) {
        return true;
    }
    // 前缀都以 "." 结尾，1.21.1 的 `21.1.` 不会匹配 1.21.10 / 1.21.11 的 `21.10.x` / `21.11.x`
    neoforge_modern_prefix(mc_version)
        .map(|prefix| neoforge_version.starts_with(&prefix))
        .unwrap_or(false)
}

/// 只保留属于该 MC 版本的条目，按版本号从新到旧排序并去重。不截断，保证最新版本可选。
fn filter_and_sort_neoforge_versions<I>(mc_version: &str, versions: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut out: Vec<String> = versions
        .into_iter()
        .filter(|v| neoforge_version_matches_mc(mc_version, v))
        .collect();
    sort_loader_versions_desc(&mut out);
    out
}

/// 解析 meta.neoforged.net 的响应（v2 带过滤 / v2 全量 / v1）。返回空表示没解析到可用版本。
fn parse_neoforge_meta_versions(mc_version: &str, text: &str, filtered_by_mc: bool) -> Vec<String> {
    let mut versions: Vec<String> = Vec::new();
    if let Ok(v2) = serde_json::from_str::<NeoForgeMetaV2Response>(text) {
        if let Some(list) = &v2.versions {
            for entry in list {
                let matches = match &entry.mc_version {
                    Some(v) => v == mc_version,
                    None => filtered_by_mc,
                };
                if !matches {
                    continue;
                }
                if let Some(v) = entry.neoforgeVersion.as_ref().or(entry.version.as_ref()) {
                    versions.push(v.clone());
                }
            }
        }
        // 回退：从 latest 字段取，只认与 MC 版本完全相同的键（1.21.1 不能取到 1.21.11 的值）
        if versions.is_empty() {
            if let Some(v) = v2
                .latest
                .as_ref()
                .and_then(|obj| obj.get(mc_version))
                .and_then(|v| v.as_str())
            {
                versions.push(v.to_string());
            }
        }
    }
    if versions.is_empty() {
        if let Ok(v1) = serde_json::from_str::<NeoForgeMetaV1Response>(text) {
            if let Some(v) = v1
                .latest
                .as_ref()
                .and_then(|obj| obj.get(mc_version))
                .and_then(|v| v.as_str())
            {
                versions.push(v.to_string());
            }
        }
    }
    // 老格式：直接是版本号数组（不区分 MC 版本，靠下面的过滤）
    if versions.is_empty() {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(text) {
            versions = list;
        }
    }
    filter_and_sort_neoforge_versions(mc_version, versions)
}

/// 尽力而为地查询 meta.neoforged.net；任何失败都返回 None，交给 maven 兜底。
async fn fetch_neoforge_meta_versions(mc_version: &str) -> Option<Vec<String>> {
    let client = shared_client().await;
    let urls = [
        (
            format!(
                "https://meta.neoforged.net/v2/versions/neo-forge?gameVersion={}",
                mc_version
            ),
            true,
        ),
        (
            "https://meta.neoforged.net/v2/versions/neo-forge".to_string(),
            false,
        ),
        (
            "https://meta.neoforged.net/v1/versions/neo-forge".to_string(),
            false,
        ),
    ];
    for (url, filtered_by_mc) in urls.iter() {
        let resp = match client.get(url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };
        let text = match resp.text().await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let versions = parse_neoforge_meta_versions(mc_version, &text, *filtered_by_mc);
        if !versions.is_empty() {
            return Some(versions);
        }
    }
    None
}

fn parse_neoforge_maven_versions(text: &str) -> Result<Vec<String>> {
    let parsed: NeoForgeMavenVersions =
        serde_json::from_str(text).context("响应不是有效的版本列表 JSON")?;
    parsed
        .versions
        .ok_or_else(|| anyhow!("响应中缺少 versions 字段"))
}

/// 查询官方 maven 上某个 artifact（neoforge / forge）的全部版本号。
async fn fetch_neoforge_maven_versions(artifact: &str) -> Result<Vec<String>> {
    let client = shared_client().await;
    let url = format!("{}/{}", NEOFORGE_MAVEN_VERSIONS_API, artifact);
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("请求 {} 失败", url))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow!("{} 返回 HTTP {}", url, status));
    }
    let text = resp
        .text()
        .await
        .with_context(|| format!("读取 {} 响应失败", url))?;
    parse_neoforge_maven_versions(&text).with_context(|| format!("解析 {} 失败", url))
}

/// 根据官方 maven 的查询结果得出最终列表或准确的错误：
/// - 有匹配版本 → 返回（即使另一个 artifact 查询失败）；
/// - 所有 artifact 都正常返回但没有匹配版本 → `NoVersionsForMc`；
/// - 有查询失败且没有拿到版本 → `SourcesUnavailable`（才提示检查网络）。
fn neoforge_versions_from_maven_results(
    mc_version: &str,
    results: Vec<(String, Result<Vec<String>>)>,
) -> Result<Vec<String>> {
    let mut all: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for (artifact, result) in results {
        match result {
            Ok(list) => all.extend(list),
            Err(e) => failures.push(format!("net.neoforged:{}: {:#}", artifact, e)),
        }
    }
    let versions = filter_and_sort_neoforge_versions(mc_version, all);
    if !versions.is_empty() {
        return Ok(versions);
    }
    let err = if failures.is_empty() {
        NeoForgeVersionListError::NoVersionsForMc {
            mc_version: mc_version.to_string(),
        }
    } else {
        NeoForgeVersionListError::SourcesUnavailable {
            mc_version: mc_version.to_string(),
            details: failures,
        }
    };
    Err(err.into())
}

pub async fn get_neoforge_versions(mc_version: &str) -> Result<Vec<String>> {
    if let Some(v) = fetch_bmcl_neoforge_versions(mc_version).await {
        println!("[NeoForge] BMCLAPI: {} 个版本", v.len());
        return Ok(v);
    }
    if let Some(v) = fetch_neoforge_meta_versions(mc_version).await {
        println!("[NeoForge] meta: {} 个版本", v.len());
        return Ok(v);
    }
    // 官方 maven 是权威来源：它正常返回但没有匹配版本时，说明该 MC 版本确实没有 NeoForge
    let mut results: Vec<(String, Result<Vec<String>>)> = Vec::new();
    for artifact in neoforge_maven_artifacts(mc_version) {
        let result = fetch_neoforge_maven_versions(artifact).await;
        results.push((artifact.to_string(), result));
    }
    let versions = neoforge_versions_from_maven_results(mc_version, results)?;
    println!("[NeoForge] Maven: {} 个版本", versions.len());
    Ok(versions)
}

/// 生成 NeoForge 安装器的候选下载地址：官方 maven 优先，BMCL 镜像兜底；
/// 1.20.1 的版本优先走旧坐标 `net.neoforged:forge`，减少一次必然 404 的请求。
fn neoforge_installer_urls(mc_version: &str, neoforge_version: &str) -> Vec<String> {
    let legacy_prefix = format!("{}-", mc_version);
    let mut coords: Vec<(&str, String)> = Vec::new();
    if neoforge_version.starts_with(&legacy_prefix) {
        coords.push(("forge", neoforge_version.to_string()));
        coords.push(("neoforge", neoforge_version.to_string()));
    } else if is_legacy_neoforge_mc(mc_version) {
        // 整合包清单里可能只写 `47.1.106`，旧坐标上的真实版本号是 `1.20.1-47.1.106`
        coords.push(("forge", format!("{}{}", legacy_prefix, neoforge_version)));
        coords.push(("forge", neoforge_version.to_string()));
        coords.push(("neoforge", neoforge_version.to_string()));
    } else {
        coords.push(("neoforge", neoforge_version.to_string()));
        coords.push(("forge", neoforge_version.to_string()));
    }
    let hosts = [
        "https://maven.neoforged.net/releases",
        "https://bmclapi2.bangbang93.com/maven",
    ];
    let mut urls = Vec::new();
    for host in hosts {
        for (artifact, version) in &coords {
            urls.push(format!(
                "{}/net/neoforged/{}/{}/{}-{}-installer.jar",
                host, artifact, version, artifact, version
            ));
        }
    }
    urls
}

async fn download_installer(
    mc_version: &str,
    neoforge_version: &str,
    mc_dir: &PathBuf,
) -> Result<PathBuf> {
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
    let urls = neoforge_installer_urls(mc_version, neoforge_version);
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
    let installer_jar = download_installer(mc_version, neoforge_version, &mc_path).await?;
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

#[cfg(test)]
mod tests {
    use super::{
        filter_and_sort_neoforge_versions, neoforge_installer_urls, neoforge_maven_artifacts,
        neoforge_version_matches_mc, neoforge_versions_from_maven_results,
        parse_neoforge_maven_versions, parse_neoforge_meta_versions, NeoForgeVersionListError,
    };
    use anyhow::anyhow;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

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
        // 旧坐标里混进来的无前缀条目没有对应的安装器，不能出现在 1.20.1 列表里
        assert!(!neoforge_version_matches_mc("1.20.1", "47.1.82"));
    }

    #[test]
    fn similar_minecraft_versions_do_not_bleed_into_each_other() {
        // 1.21.1 ↔ 1.21.10 / 1.21.11
        assert!(!neoforge_version_matches_mc("1.21.1", "21.11.5"));
        assert!(!neoforge_version_matches_mc("1.21.1", "21.10.0-beta"));
        assert!(neoforge_version_matches_mc("1.21.11", "21.11.5"));
        assert!(!neoforge_version_matches_mc("1.21.11", "21.1.99"));
        assert!(neoforge_version_matches_mc("1.21.10", "21.10.0-beta"));
        // 1.20.1 ↔ 1.20.10（假想）
        assert!(!neoforge_version_matches_mc("1.20.1", "1.20.10-47.1.1"));
        // 26.1 ↔ 26.1.1 / 26.1.2
        assert!(neoforge_version_matches_mc("26.1", "26.1.0.33"));
        assert!(!neoforge_version_matches_mc("26.1", "26.1.1.5"));
        assert!(!neoforge_version_matches_mc("26.1", "26.1.2.130"));
        assert!(!neoforge_version_matches_mc("26.1.1", "26.1.0.33"));
        assert!(neoforge_version_matches_mc("26.1.1", "26.1.1.5"));
    }

    #[test]
    fn full_list_is_sorted_numerically_and_not_truncated() {
        // 模拟 maven 的原始顺序（按发布顺序，字符串序会把 21.1.99 排到 21.1.267 前面）
        let mut raw: Vec<String> = (1..=267).map(|b| format!("21.1.{}", b)).collect();
        raw.extend(strings(&[
            "21.0.167",
            "21.10.0-beta",
            "21.10.64",
            "21.11.5",
            "20.2.93",
            "26.1.0.33",
        ]));
        let v = filter_and_sort_neoforge_versions("1.21.1", raw);
        assert_eq!(v.len(), 267, "1.21.1 的版本不应被截断，也不应混入其他 MC 版本");
        assert_eq!(v.first().map(String::as_str), Some("21.1.267"));
        assert_eq!(v.last().map(String::as_str), Some("21.1.1"));
        assert!(v.contains(&"21.1.100".to_string()));
        let pos = |s: &str| v.iter().position(|x| x == s).unwrap();
        assert!(pos("21.1.100") < pos("21.1.99"));
        assert!(pos("21.1.267") < pos("21.1.100"));
    }

    #[test]
    fn legacy_1_20_1_list_comes_from_forge_coordinate() {
        assert_eq!(neoforge_maven_artifacts("1.20.1"), &["forge"]);
        assert_eq!(neoforge_maven_artifacts("1.20.2"), &["neoforge"]);
        assert_eq!(neoforge_maven_artifacts("1.21.1"), &["neoforge"]);

        let raw = strings(&[
            "1.20.1-47.1.5",
            "1.20.1-47.1.99",
            "1.20.1-47.1.100",
            "1.20.1-47.1.106",
            "47.1.82",
        ]);
        let v = filter_and_sort_neoforge_versions("1.20.1", raw);
        assert_eq!(
            v,
            strings(&["1.20.1-47.1.106", "1.20.1-47.1.100", "1.20.1-47.1.99", "1.20.1-47.1.5"])
        );
    }

    #[test]
    fn parses_maven_version_api_response() {
        let text = r#"{"isSnapshot":false,"versions":["21.1.1","21.1.10","21.1.2"]}"#;
        assert_eq!(
            parse_neoforge_maven_versions(text).unwrap(),
            strings(&["21.1.1", "21.1.10", "21.1.2"])
        );
        assert!(parse_neoforge_maven_versions("Not Found").is_err());
        assert!(parse_neoforge_maven_versions(r#"{"isSnapshot":false}"#).is_err());
    }

    #[test]
    fn no_matching_version_is_not_reported_as_network_error() {
        let results = vec![(
            "neoforge".to_string(),
            Ok(strings(&["20.2.93", "21.1.267"])),
        )];
        let err = neoforge_versions_from_maven_results("1.12.2", results).unwrap_err();
        match err.downcast_ref::<NeoForgeVersionListError>() {
            Some(NeoForgeVersionListError::NoVersionsForMc { mc_version }) => {
                assert_eq!(mc_version, "1.12.2")
            }
            other => panic!("期望 NoVersionsForMc，实际 {:?}", other),
        }
        let msg = err.to_string();
        assert!(msg.contains("1.12.2"));
        assert!(!msg.contains("网络"), "没有版本时不应提示检查网络: {}", msg);
    }

    #[test]
    fn failed_source_is_reported_as_network_error() {
        let results = vec![(
            "neoforge".to_string(),
            Err(anyhow!("error sending request")),
        )];
        let err = neoforge_versions_from_maven_results("1.21.1", results).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<NeoForgeVersionListError>(),
            Some(NeoForgeVersionListError::SourcesUnavailable { .. })
        ));
        let msg = err.to_string();
        assert!(msg.contains("网络"));
        assert!(msg.contains("error sending request"));
    }

    #[test]
    fn partial_failure_still_returns_found_versions() {
        let results = vec![
            ("forge".to_string(), Ok(strings(&["1.20.1-47.1.106", "1.20.1-47.1.5"]))),
            ("neoforge".to_string(), Err(anyhow!("timeout"))),
        ];
        assert_eq!(
            neoforge_versions_from_maven_results("1.20.1", results).unwrap(),
            strings(&["1.20.1-47.1.106", "1.20.1-47.1.5"])
        );
    }

    #[test]
    fn meta_latest_map_uses_exact_mc_key() {
        let text = r#"{"latest":{"1.21.11":"21.11.5","1.21.10":"21.10.64","1.21.1":"21.1.267"}}"#;
        assert_eq!(
            parse_neoforge_meta_versions("1.21.1", text, false),
            strings(&["21.1.267"])
        );
        let only_newer = r#"{"latest":{"1.21.11":"21.11.5"}}"#;
        assert!(parse_neoforge_meta_versions("1.21.1", only_newer, false).is_empty());
        // 老格式数组也要按 MC 版本过滤
        let array = r#"["21.11.5","21.1.99","21.1.100"]"#;
        assert_eq!(
            parse_neoforge_meta_versions("1.21.1", array, false),
            strings(&["21.1.100", "21.1.99"])
        );
    }

    #[test]
    fn installer_urls_prefer_legacy_coordinate_for_1_20_1() {
        let urls = neoforge_installer_urls("1.20.1", "1.20.1-47.1.106");
        assert_eq!(
            urls[0],
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
        assert!(urls.contains(
            &"https://bmclapi2.bangbang93.com/maven/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
                .to_string()
        ));

        // 只给出 47.1.106（如整合包清单）时也能拼出旧坐标
        let urls = neoforge_installer_urls("1.20.1", "47.1.106");
        assert_eq!(
            urls[0],
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
    }

    #[test]
    fn installer_urls_use_neoforge_coordinate_for_modern_versions() {
        let urls = neoforge_installer_urls("1.21.1", "21.1.267");
        assert_eq!(
            urls[0],
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.267/neoforge-21.1.267-installer.jar"
        );
        assert!(urls.contains(
            &"https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge/21.1.267/neoforge-21.1.267-installer.jar"
                .to_string()
        ));
        assert!(urls.iter().all(|u| !u.contains("1.21.1-")));
    }
}
