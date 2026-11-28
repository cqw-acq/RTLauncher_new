use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

use crate::downloader::concurrent_download;
use crate::downloader::shared_utils::{
    parse_library_path_for_fs, sanitize_instance_name,
};
use crate::http_client::shared_client;

const BMCL_API_ROOT: &str = "https://bmclapi2.bangbang93.com";

const VANILLA_MAIN: &str = "net.minecraft.client.main.Main";
const LAUNCH_WRAPPER_MAIN: &str = "net.minecraft.launchwrapper.Launch";
const MOD_LAUNCHER_MAIN: &str = "cpw.mods.modlauncher.Launcher";
const BOOTSTRAP_LAUNCHER_MAIN: &str = "cpw.mods.bootstraplauncher.BootstrapLauncher";
const FORGE_BOOTSTRAP_MAIN: &str = "net.minecraftforge.bootstrap.ForgeBootstrap";
const NEO_FORGE_BOOTSTRAP_MAIN: &str = "net.neoforged.fml.startup.Client";

const FORGE_OPTIFINE_COMPATIBLE_MAINS: &[&str] = &[
    VANILLA_MAIN,
    LAUNCH_WRAPPER_MAIN,
    MOD_LAUNCHER_MAIN,
    BOOTSTRAP_LAUNCHER_MAIN,
    FORGE_BOOTSTRAP_MAIN,
    NEO_FORGE_BOOTSTRAP_MAIN,
];

#[derive(Debug, Deserialize, Clone)]
struct OptiFineBmclEntry {
    #[serde(default)]
    pub dl: String,
    #[serde(default)]
    pub ver: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub mirror: String,
    #[serde(default)]
    pub mcversion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptiFineVersionInfo {
    pub game_version: String,
    pub self_version: String,
    pub full_version: String,
    pub download_url: String,
    pub official_url: String,
    pub is_pre: bool,
}

fn lookup_version_normalize(version: &str) -> String {
    match version {
        "1.8" => "1.8.0".to_string(),
        "1.9" => "1.9.0".to_string(),
        v => v.to_string(),
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<i32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i32>().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}

/// OptiFine 官网。官网的 adloadx 页面里带有 downloadx 直链（含 token），HMCL、PCL 也是这样解析的。
const OPTIFINE_OFFICIAL_ROOT: &str = "https://optifine.net";
/// 版本列表 / 官网页面请求的单次超时，避免源挂起时等满共享客户端的 600 秒总超时
const OPTIFINE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// 小于这个大小的"安装器"一定不完整（正常的 OptiFine 安装器在 1MB 以上）
const OPTIFINE_INSTALLER_MIN_SIZE: u64 = 100_000;

/// 一个 OptiFine 构建。mcversion / type / patch 的含义与 BMCLAPI 一致：
/// 正式版是 `HD_U` + `G5`，预览版是 `HD_U_J1` + `pre13`。
#[derive(Debug, Clone, PartialEq, Eq)]
struct OptiFineBuild {
    mcversion: String,
    type_: String,
    patch: String,
}

/// 只允许出现在版本号 / 文件名里的字符，防止拼进 URL 时混入 `&`、`/` 等
fn is_safe_optifine_token(s: &str) -> bool {
    !s.is_empty()
        && s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

impl OptiFineBuild {
    fn new_checked(mcversion: &str, type_: &str, patch: &str) -> Option<Self> {
        if [mcversion, type_, patch].iter().all(|s| is_safe_optifine_token(s)) {
            Some(Self {
                mcversion: mcversion.to_string(),
                type_: type_.to_string(),
                patch: patch.to_string(),
            })
        } else {
            None
        }
    }

    fn is_pre(&self) -> bool {
        self.patch.starts_with("pre") || self.patch.starts_with("alpha")
    }

    fn self_version(&self) -> String {
        format!("{}_{}", self.type_, self.patch)
    }

    fn bmcl_url(&self) -> String {
        format!(
            "{}/optifine/{}/{}/{}",
            BMCL_API_ROOT,
            lookup_version_normalize(&self.mcversion),
            self.type_,
            self.patch
        )
    }

    /// optifine.net 上的文件名：正式版 `OptiFine_1.12.2_HD_U_G5.jar`，
    /// 预览版 `preview_OptiFine_1.21.1_HD_U_J1_pre13.jar`（与 BMCLAPI 列表里的 filename 一致）
    fn official_filename(&self) -> String {
        format!(
            "{}OptiFine_{}_{}_{}.jar",
            if self.is_pre() { "preview_" } else { "" },
            lookup_version_normalize(&self.mcversion),
            self.type_,
            self.patch
        )
    }

    fn official_page_url(&self) -> String {
        format!(
            "{}/adloadx?f={}",
            OPTIFINE_OFFICIAL_ROOT,
            self.official_filename()
        )
    }

    /// 从 BMCLAPI 下载链接 `.../optifine/{mc}/{type}/{patch}` 解析
    fn from_bmcl_url(url: &str) -> Option<Self> {
        let path = url.split(|c| c == '?' || c == '#').next()?;
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let idx = segments.iter().position(|s| *s == "optifine")?;
        match &segments[idx + 1..] {
            [mc, type_, patch, ..] => Self::new_checked(mc, type_, patch),
            _ => None,
        }
    }

    /// 从官网文件名 `(preview_)OptiFine_{mc}_{type}_{patch}.jar` 解析
    fn from_official_filename(name: &str) -> Option<Self> {
        let stem = name.strip_prefix("preview_").unwrap_or(name);
        let stem = stem.strip_prefix("OptiFine_")?.strip_suffix(".jar")?;
        let (mc, rest) = stem.split_once('_')?;
        let (type_, patch) = rest.rsplit_once('_')?;
        if !mc.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        Self::new_checked(mc, type_, patch)
    }
}

/// OptiFine 版本排序键：`HD_U_G5` → (G, 5, 正式版, 0)，`HD_U_J1_pre13` → (J, 1, 预览版, 13)。
/// 只比较数字会把 `HD_U_C9` 排到 `HD_U_G5` 前面、把 `J1_pre15` 排到正式版 `J1` 前面。
fn optifine_release_key(self_version: &str) -> (char, u32, bool, u32) {
    let mut letter = '\0';
    let mut number = 0u32;
    let mut pre: Option<u32> = None;
    for token in self_version.split('_') {
        if let Some(n) = token.strip_prefix("pre").or_else(|| token.strip_prefix("alpha")) {
            pre = Some(n.parse().unwrap_or(0));
            continue;
        }
        let mut chars = token.chars();
        if let Some(first) = chars.next() {
            let rest = chars.as_str();
            if first.is_ascii_uppercase()
                && !rest.is_empty()
                && rest.chars().all(|c| c.is_ascii_digit())
            {
                letter = first;
                number = rest.parse().unwrap_or(0);
            }
        }
    }
    (letter, number, pre.is_none(), pre.unwrap_or(0))
}

fn compare_optifine_self_versions(a: &str, b: &str) -> std::cmp::Ordering {
    optifine_release_key(a)
        .cmp(&optifine_release_key(b))
        .then_with(|| compare_versions(a, b))
}

/// 把构建列表转换成指定 MC 版本的 OptiFineVersionInfo（新版本在前）
fn build_optifine_version_infos(
    builds: Vec<OptiFineBuild>,
    mc_version: &str,
) -> Vec<OptiFineVersionInfo> {
    // BMCLAPI 与官网都把 1.8 / 1.9 写成 1.8.0 / 1.9.0，两边都规范化后再比较
    let target_mc = lookup_version_normalize(mc_version);
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for build in builds {
        let game_version = lookup_version_normalize(&build.mcversion);
        if game_version != target_mc {
            continue;
        }
        let download_url = build.bmcl_url();
        if !seen.insert(download_url.clone()) {
            continue;
        }
        let self_version = build.self_version();
        results.push(OptiFineVersionInfo {
            full_version: format!("{}_{}", game_version, self_version),
            game_version,
            self_version,
            download_url,
            official_url: build.official_page_url(),
            is_pre: build.is_pre(),
        });
    }

    results.sort_by(|a, b| compare_optifine_self_versions(&b.self_version, &a.self_version));
    results
}

async fn fetch_optifine_text(url: &str) -> Result<String> {
    let client = shared_client().await;
    let resp = client
        .get(url)
        .timeout(OPTIFINE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| anyhow!("请求 {} 失败: {}", url, e))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("请求 {} 失败, HTTP 状态码: {}", url, status);
    }
    resp.text()
        .await
        .map_err(|e| anyhow!("读取 {} 失败: {}", url, e))
}

async fn fetch_bmcl_optifine_builds() -> Result<Vec<OptiFineBuild>> {
    let url = format!("{}/optifine/versionlist", BMCL_API_ROOT);
    let text = fetch_optifine_text(&url).await?;
    let list: Vec<OptiFineBmclEntry> = serde_json::from_str(&text).map_err(|e| {
        let preview: String = text.chars().take(200).collect();
        anyhow!("解析 BMCLAPI OptiFine 版本列表失败: {} (内容开头: {})", e, preview)
    })?;
    Ok(list
        .into_iter()
        .filter_map(|e| OptiFineBuild::new_checked(&e.mcversion, &e.type_, &e.patch))
        .collect())
}

/// 从 optifine.net/downloads 页面里的 `adloadx?f=<文件名>` 链接解析出所有构建
fn parse_official_downloads_page(html: &str) -> Vec<OptiFineBuild> {
    let re = regex::Regex::new(r"adloadx\?f=((?:preview_)?OptiFine_[A-Za-z0-9._\-]+?\.jar)")
        .expect("OptiFine 官网文件名正则无效");
    let mut seen = std::collections::HashSet::new();
    re.captures_iter(html)
        .filter_map(|cap| {
            let filename = cap.get(1)?.as_str();
            if !seen.insert(filename.to_string()) {
                return None;
            }
            OptiFineBuild::from_official_filename(filename)
        })
        .collect()
}

async fn fetch_official_optifine_builds() -> Result<Vec<OptiFineBuild>> {
    let url = format!("{}/downloads", OPTIFINE_OFFICIAL_ROOT);
    let html = fetch_optifine_text(&url).await?;
    let builds = parse_official_downloads_page(&html);
    if builds.is_empty() {
        bail!("未能从 OptiFine 官网下载页解析出任何版本");
    }
    Ok(builds)
}

/// 获取 OptiFine 版本列表：优先 BMCLAPI，BMCLAPI 不可用时回退到 optifine.net 下载页。
/// `download_url` 始终是 BMCLAPI 格式（安装时据此解析版本号），`official_url` 是官网下载页。
pub async fn get_optifine_versions(mc_version: &str) -> Result<Vec<OptiFineVersionInfo>> {
    let builds = match fetch_bmcl_optifine_builds().await {
        Ok(builds) => builds,
        Err(bmcl_err) => {
            eprintln!(
                "[OptiFine] BMCLAPI 版本列表不可用，回退到 OptiFine 官网: {}",
                bmcl_err
            );
            let builds = fetch_official_optifine_builds().await.map_err(|official_err| {
                anyhow!(
                    "BMCLAPI 镜像与 OptiFine 官网均不可用（BMCLAPI 镜像: {}；OptiFine 官网: {}）",
                    bmcl_err,
                    official_err
                )
            })?;
            println!("[OptiFine] 已从 OptiFine 官网获取 {} 个构建", builds.len());
            builds
        }
    };
    Ok(build_optifine_version_infos(builds, mc_version))
}

fn derive_filename_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if let Some(idx) = segments.iter().position(|s| *s == "optifine") {
        let after = &segments[idx + 1..];
        if after.len() >= 3 {
            return format!(
                "OptiFine_{}_{}_{}.jar",
                after[0], after[1], after[2]
            );
        }
    }

    if let Some(last) = segments.last() {
        if !last.is_empty() && last.contains('.') {
            return last.to_string();
        }
    }

    if let Some(query_part) = url.split('?').nth(1) {
        for param in query_part.split('&') {
            if let Some(val) = param.strip_prefix("f=") {
                let decoded = val.replace("%2F", "/").replace("%20", " ");
                if !decoded.is_empty() {
                    return decoded;
                }
            }
        }
    }

    format!("optifine-installer.jar")
}

pub async fn download_optifine_installer(
    download_url: &str,
    mc_dir: &Path,
) -> Result<PathBuf> {
    download_optifine_installer_with_fallback(download_url, download_url, mc_dir).await
}

/// OptiFine 安装器的一个候选下载源
#[derive(Debug, Clone, PartialEq, Eq)]
enum InstallerSource {
    /// 可以直接下载的链接（BMCLAPI 等）
    Direct(String),
    /// optifine.net 的 adloadx 页面，需要先从页面里解析出带 token 的 downloadx 直链
    OfficialPage { page_url: String, filename: String },
}

impl InstallerSource {
    fn from_url(url: &str) -> Option<Self> {
        let url = url.trim();
        if url.is_empty() {
            return None;
        }
        Some(match official_page_filename(url) {
            Some(filename) => InstallerSource::OfficialPage {
                page_url: url.to_string(),
                filename,
            },
            None => InstallerSource::Direct(url.to_string()),
        })
    }

    fn official_for(build: &OptiFineBuild) -> Self {
        InstallerSource::OfficialPage {
            page_url: build.official_page_url(),
            filename: build.official_filename(),
        }
    }

    fn label(&self) -> String {
        match self {
            InstallerSource::Direct(url) if url.contains("bmclapi") => {
                format!("BMCLAPI 镜像 ({})", url)
            }
            InstallerSource::Direct(url) => format!("备用源 ({})", url),
            InstallerSource::OfficialPage { page_url, .. } => {
                format!("OptiFine 官网 ({})", page_url)
            }
        }
    }
}

/// 若 `url` 是 optifine.net 的 adloadx 页面，返回其中的安装器文件名
fn official_page_filename(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "optifine.net" && host != "www.optifine.net" {
        return None;
    }
    if parsed.path() != "/adloadx" {
        return None;
    }
    let filename = parsed
        .query_pairs()
        .find(|(k, _)| k == "f")
        .map(|(_, v)| v.into_owned())?;
    OptiFineBuild::from_official_filename(&filename)?;
    Some(filename)
}

/// 安装器下载源的尝试顺序：调用方给的主链接 → 调用方给的备用链接（为空或与主链接相同则忽略）
/// → 由 BMCLAPI 链接推导出的 optifine.net 官网下载页。
fn installer_sources(download_url: &str, fallback_url: &str) -> Vec<InstallerSource> {
    let derived_official =
        OptiFineBuild::from_bmcl_url(download_url).map(|b| InstallerSource::official_for(&b));
    let mut sources: Vec<InstallerSource> = Vec::new();
    for source in [
        InstallerSource::from_url(download_url),
        InstallerSource::from_url(fallback_url),
        derived_official,
    ]
    .into_iter()
    .flatten()
    {
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    sources
}

/// 从 adloadx 页面里找出指定文件的 downloadx 直链
fn extract_official_download_link(html: &str, filename: &str) -> Option<String> {
    let re = regex::Regex::new(r"downloadx\?f=([A-Za-z0-9._\-]+)(?:&amp;|&)x=([0-9A-Za-z]+)")
        .expect("OptiFine downloadx 正则无效");
    let link = re
        .captures_iter(html)
        .find(|cap| &cap[1] == filename)
        .map(|cap| {
            format!(
                "{}/downloadx?f={}&x={}",
                OPTIFINE_OFFICIAL_ROOT, &cap[1], &cap[2]
            )
        });
    link
}

async fn resolve_official_download_url(page_url: &str, filename: &str) -> Result<String> {
    let html = fetch_optifine_text(page_url).await?;
    extract_official_download_link(&html, filename)
        .ok_or_else(|| anyhow!("OptiFine 官网页面中没有找到 {} 的下载链接", filename))
}

/// 下载内容是否像一个完整的 OptiFine 安装器（能作为 zip 打开）。
/// optifine.net 对不存在的文件会返回 200 + "File not found." 的 HTML，必须靠这一步识别。
fn is_valid_installer_jar(path: &Path) -> bool {
    let big_enough = fs::metadata(path)
        .map(|m| m.len() >= OPTIFINE_INSTALLER_MIN_SIZE)
        .unwrap_or(false);
    big_enough
        && fs::File::open(path)
            .ok()
            .and_then(|f| ZipArchive::new(f).ok())
            .map(|zip| zip.len() > 0)
            .unwrap_or(false)
}

async fn download_installer_from(
    source: &InstallerSource,
    target: &Path,
) -> Result<PathBuf> {
    let url = match source {
        InstallerSource::Direct(url) => url.clone(),
        InstallerSource::OfficialPage { page_url, filename } => {
            resolve_official_download_url(page_url, filename).await?
        }
    };
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| anyhow!("无效的安装器缓存路径: {}", target.display()))?;
    let target_dir = target
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("无效的安装器缓存路径: {}", target.display()))?;
    let task = concurrent_download::DownloadTask {
        file_name,
        target_dir,
        urls: vec![url],
        sha1: None,
    };
    let path = concurrent_download::download_one(task).await?;
    if !is_valid_installer_jar(&path) {
        let _ = fs::remove_file(&path);
        bail!("下载到的文件不是有效的 OptiFine 安装器（可能是错误页面）");
    }
    Ok(path)
}

/// 下载 OptiFine 安装器。依次尝试 `download_url`、`fallback_url`（可为空，或与主链接相同）
/// 和 optifine.net 官网，每个源下载完都会校验是不是有效的 jar。
pub async fn download_optifine_installer_with_fallback(
    download_url: &str,
    fallback_url: &str,
    mc_dir: &Path,
) -> Result<PathBuf> {
    let cache_dir = mc_dir.join("cache").join("optifine_installer");
    fs::create_dir_all(&cache_dir).ok();

    let filename = derive_filename_from_url(download_url);
    let target = cache_dir.join(&filename);

    if target.exists() {
        if is_valid_installer_jar(&target) {
            println!("[OptiFine] 使用缓存安装器: {}", target.display());
            return Ok(target);
        }
        // 下载器看到目标文件已存在会直接当作成功，残缺的缓存必须先删掉
        println!(
            "[OptiFine] 缓存的安装器不完整或已损坏，删除后重新下载: {}",
            target.display()
        );
        let _ = fs::remove_file(&target);
    }

    let sources = installer_sources(download_url, fallback_url);
    let mut errors: Vec<String> = Vec::new();
    for source in &sources {
        let label = source.label();
        println!("[OptiFine] 下载安装器: {}", label);
        match download_installer_from(source, &target).await {
            Ok(path) => {
                if !errors.is_empty() {
                    println!("[OptiFine] 已回退到 {} 并下载成功", label);
                }
                return Ok(path);
            }
            Err(e) => {
                eprintln!("[OptiFine] {} 下载失败: {}", label, e);
                errors.push(format!("{}: {}", label, e));
                let _ = fs::remove_file(&target);
            }
        }
    }

    if errors.is_empty() {
        bail!("下载 OptiFine 安装器失败: 无效的下载链接 {}", download_url);
    }
    bail!(
        "下载 OptiFine 安装器失败，所有下载源均不可用（{}）。请检查网络连接后重试。",
        errors.join("；")
    )
}

#[cfg(test)]
mod download_source_tests {
    // 放在下载相关函数旁边，而不是文件末尾，避免和同文件其他部分的改动冲突
    use super::*;

    fn build(mc: &str, type_: &str, patch: &str) -> OptiFineBuild {
        OptiFineBuild::new_checked(mc, type_, patch).unwrap()
    }

    #[test]
    fn official_filename_matches_bmcl_naming() {
        assert_eq!(
            build("1.12.2", "HD_U", "G5").official_filename(),
            "OptiFine_1.12.2_HD_U_G5.jar"
        );
        assert_eq!(
            build("1.21.1", "HD_U_J1", "pre13").official_filename(),
            "preview_OptiFine_1.21.1_HD_U_J1_pre13.jar"
        );
        assert_eq!(
            build("1.8", "HD_U", "I7").official_filename(),
            "OptiFine_1.8.0_HD_U_I7.jar"
        );
    }

    #[test]
    fn official_filename_round_trips() {
        for name in [
            "OptiFine_1.12.2_HD_U_G5.jar",
            "preview_OptiFine_1.21.1_HD_U_J1_pre13.jar",
            "OptiFine_1.9.0_HD_U_E7.jar",
            "preview_OptiFine_1.12.2_HD_U_G6_pre1.jar",
        ] {
            let b = OptiFineBuild::from_official_filename(name).unwrap();
            assert_eq!(b.official_filename(), name);
        }
        assert!(OptiFineBuild::from_official_filename("OptiFine_x.jar").is_none());
        assert!(OptiFineBuild::from_official_filename("readme.txt").is_none());
    }

    #[test]
    fn bmcl_url_round_trips() {
        let url = "https://bmclapi2.bangbang93.com/optifine/1.20.1/HD_U/I5";
        let b = OptiFineBuild::from_bmcl_url(url).unwrap();
        assert_eq!(b, build("1.20.1", "HD_U", "I5"));
        assert_eq!(b.bmcl_url(), url);
        assert!(OptiFineBuild::from_bmcl_url("https://bmclapi2.bangbang93.com/optifine/1.20.1").is_none());
        assert!(OptiFineBuild::from_bmcl_url("https://example.com/x.jar").is_none());
    }

    #[test]
    fn sources_do_not_repeat_the_same_url_and_end_with_official_site() {
        let url = "https://bmclapi2.bangbang93.com/optifine/1.20.1/HD_U/I5";
        let official = InstallerSource::OfficialPage {
            page_url: "https://optifine.net/adloadx?f=OptiFine_1.20.1_HD_U_I5.jar".to_string(),
            filename: "OptiFine_1.20.1_HD_U_I5.jar".to_string(),
        };
        let expected = vec![InstallerSource::Direct(url.to_string()), official.clone()];

        // 以前组合安装把主链接当"备用源"再传一遍，等于没有备用源
        assert_eq!(installer_sources(url, url), expected);
        assert_eq!(installer_sources(url, ""), expected);
        // 前端传 get_optifine_versions 返回的 official_url 时也不会重复
        assert_eq!(
            installer_sources(url, "https://optifine.net/adloadx?f=OptiFine_1.20.1_HD_U_I5.jar"),
            expected
        );
        let custom = "https://example.com/OptiFine_1.20.1_HD_U_I5.jar";
        assert_eq!(
            installer_sources(url, custom),
            vec![
                InstallerSource::Direct(url.to_string()),
                InstallerSource::Direct(custom.to_string()),
                official,
            ]
        );
    }

    #[test]
    fn official_page_filename_only_accepts_optifine_net() {
        assert_eq!(
            official_page_filename("https://optifine.net/adloadx?f=OptiFine_1.12.2_HD_U_G5.jar"),
            Some("OptiFine_1.12.2_HD_U_G5.jar".to_string())
        );
        assert_eq!(
            official_page_filename("https://evil.example/adloadx?f=OptiFine_1.12.2_HD_U_G5.jar"),
            None
        );
        assert_eq!(
            official_page_filename("https://optifine.net/adloadx?f=OptiFine_1.12.2_HD_U_G5.jar%26x%3D1"),
            None
        );
    }

    #[test]
    fn extracts_downloadx_link_for_the_requested_file() {
        let html = r#"
            <a href='downloadx?f=OptiFine_1.12.2_HD_U_G5.jar&x=568d98ea1e45db75bca79de45b65a0b4' onclick='onDownload()'>OptiFine 1.12.2 HD U G5</a>
            <a href="downloadx?f=OptiFine_1.12.1_HD_U_G5.jar&amp;x=abc123">other</a>
        "#;
        assert_eq!(
            extract_official_download_link(html, "OptiFine_1.12.2_HD_U_G5.jar").as_deref(),
            Some("https://optifine.net/downloadx?f=OptiFine_1.12.2_HD_U_G5.jar&x=568d98ea1e45db75bca79de45b65a0b4")
        );
        assert_eq!(
            extract_official_download_link(html, "OptiFine_1.12.1_HD_U_G5.jar").as_deref(),
            Some("https://optifine.net/downloadx?f=OptiFine_1.12.1_HD_U_G5.jar&x=abc123")
        );
        assert_eq!(extract_official_download_link(html, "OptiFine_1.20.1_HD_U_I5.jar"), None);
    }

    #[test]
    fn rejects_error_pages_as_installer() {
        let dir = std::env::temp_dir().join(format!("rtl-of-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let html = dir.join("not-found.jar");
        fs::write(&html, b"File not found.\n").unwrap();
        assert!(!is_valid_installer_jar(&html));
        let big_html = dir.join("big.jar");
        fs::write(&big_html, vec![b'<'; (OPTIFINE_INSTALLER_MIN_SIZE + 1) as usize]).unwrap();
        assert!(!is_valid_installer_jar(&big_html));
        assert!(!is_valid_installer_jar(&dir.join("missing.jar")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_official_downloads_page() {
        let html = r#"
            <td class='colMirror'><a href="http://optifine.net/adloadx?f=OptiFine_1.21.1_HD_U_J1.jar&x=ed92">(Mirror)</a></td>
            <td class='colDownload'><a href="https://optifine.net/adloadx?f=OptiFine_1.21.1_HD_U_J1.jar">Download</a></td>
            <a href="adloadx?f=preview_OptiFine_1.12.2_HD_U_G6_pre1.jar&x=ed92">Mirror</a>
            <a href="adloadx?f=OptiFine_1.12.2_HD_U_G5.jar">Download</a>
        "#;
        assert_eq!(
            parse_official_downloads_page(html),
            vec![
                build("1.21.1", "HD_U", "J1"),
                build("1.12.2", "HD_U_G6", "pre1"),
                build("1.12.2", "HD_U", "G5"),
            ]
        );
    }

    #[test]
    fn version_infos_are_sorted_newest_first_and_link_to_official_page() {
        let builds = vec![
            build("1.12.2", "HD_U", "C9"),
            build("1.12.2", "HD_U", "E3"),
            build("1.12.2", "HD_U", "G5"),
            build("1.12.2", "HD_U_G5", "pre1"),
            build("1.12.2", "HD_U_F6", "pre1"),
            build("1.12.2", "HD_U", "F5"),
            build("1.20.1", "HD_U", "I5"),
            build("1.12.2", "HD_U", "G5"),
        ];
        let infos = build_optifine_version_infos(builds, "1.12.2");
        let order: Vec<&str> = infos.iter().map(|i| i.self_version.as_str()).collect();
        assert_eq!(
            order,
            vec!["HD_U_G5", "HD_U_G5_pre1", "HD_U_F6_pre1", "HD_U_F5", "HD_U_E3", "HD_U_C9"]
        );
        let top = &infos[0];
        assert_eq!(top.download_url, "https://bmclapi2.bangbang93.com/optifine/1.12.2/HD_U/G5");
        assert_eq!(top.official_url, "https://optifine.net/adloadx?f=OptiFine_1.12.2_HD_U_G5.jar");
        assert_eq!(top.full_version, "1.12.2_HD_U_G5");
        assert!(!top.is_pre);
        assert!(infos[1].is_pre);
    }

    #[test]
    fn release_sorts_above_its_previews() {
        let builds = vec![
            build("1.21.1", "HD_U_J1", "pre9"),
            build("1.21.1", "HD_U_J1", "pre15"),
            build("1.21.1", "HD_U", "J1"),
            build("1.21.1", "HD_U_J1", "pre10"),
        ];
        let order: Vec<String> = build_optifine_version_infos(builds, "1.21.1")
            .into_iter()
            .map(|i| i.self_version)
            .collect();
        assert_eq!(order, vec!["HD_U_J1", "HD_U_J1_pre15", "HD_U_J1_pre10", "HD_U_J1_pre9"]);
    }

    #[test]
    fn legacy_mc_versions_match_normalized_names() {
        let infos = build_optifine_version_infos(vec![build("1.8.0", "HD_U", "I7")], "1.8");
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].download_url, "https://bmclapi2.bangbang93.com/optifine/1.8.0/HD_U/I7");
    }
}

fn get_library_file(mc_dir: &Path, library_name: &str) -> Result<PathBuf> {
    let (parent_dir, jar_name) = parse_library_path_for_fs(library_name)?;
    let lib_dir = mc_dir.join("libraries").join(parent_dir);
    fs::create_dir_all(&lib_dir).ok();
    Ok(lib_dir.join(jar_name))
}

fn zip_entry_exists<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, path: &str) -> bool {
    for i in 0..zip.len() {
        if let Ok(file) = zip.by_index(i) {
            let name = file.name().trim_start_matches('/');
            if name == path.trim_start_matches('/') {
                return true;
            }
        }
    }
    false
}

fn read_zip_entry_to_vec<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    path: &str,
) -> Option<Vec<u8>> {
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).ok()?;
        let name = file.name().trim_start_matches('/').to_string();
        if name == path.trim_start_matches('/') {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

fn extract_zip_entry_to_file<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    entry_path: &str,
    dest: &Path,
) -> Result<bool> {
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).with_context(|| format!("读取 zip 索引 {} 失败", i))?;
        let name = file.name().trim_start_matches('/').to_string();
        if name == entry_path.trim_start_matches('/') {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut out = fs::File::create(dest)
                .with_context(|| format!("创建文件失败: {}", dest.display()))?;
            std::io::copy(&mut file, &mut out).ok();
            return Ok(true);
        }
    }
    Ok(false)
}

fn remove_mods_toml_from_jar(jar_path: &Path) -> Result<()> {
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let bytes = fs::read(jar_path)
        .with_context(|| format!("读取 jar 失败: {}", jar_path.display()))?;
    let cursor = Cursor::new(bytes);
    let mut src_zip = ZipArchive::new(cursor).context("解析 jar 失败")?;

    let tmp_path = jar_path.with_extension("jar.tmp");
    let tmp_file = fs::File::create(&tmp_path)
        .with_context(|| format!("创建临时 jar 失败: {}", tmp_path.display()))?;
    let mut writer = ZipWriter::new(tmp_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..src_zip.len() {
        let mut file = src_zip.by_index(i).with_context(|| format!("读取 zip 项 {} 失败", i))?;
        let name = file.name().to_string();
        let trimmed = name.trim_start_matches('/');
        if trimmed.eq_ignore_ascii_case("META-INF/mods.toml") {
            continue;
        }
        writer.start_file(name, options).ok();
        std::io::copy(&mut file, &mut writer).ok();
    }
    writer.finish().ok();
    drop(src_zip);

    fs::rename(&tmp_path, jar_path)
        .with_context(|| format!("重命名临时 jar 失败: {} -> {}", tmp_path.display(), jar_path.display()))?;
    Ok(())
}

fn read_buildof(installer_zip: &mut ZipArchive<Cursor<Vec<u8>>>) -> Option<String> {
    let bytes = read_zip_entry_to_vec(installer_zip, "buildof.txt")?;
    String::from_utf8(bytes).ok().map(|s| s.trim().to_string())
}

fn parse_optifine_version_from_url(url: &str) -> Result<(String, String)> {
    let path = url.split('?').next().ok_or_else(|| anyhow!("无效的下载链接"))?;
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let optifine_idx = segments
        .iter()
        .position(|s| *s == "optifine")
        .ok_or_else(|| anyhow!("URL 中找不到 optifine 段: {}", url))?;

    let after_optifine = &segments[optifine_idx + 1..];
    if after_optifine.len() < 3 {
        bail!("URL 路径段数不足，无法解析版本: {}", url);
    }

    let _mcversion = after_optifine[0].to_string();
    let type_ = after_optifine[1].to_string();
    let patch = after_optifine[2].to_string();

    let self_version = format!("{}_{}", type_, patch);
    Ok((self_version, _mcversion))
}

fn parse_optifine_version_from_installer(
    installer_bytes: &[u8],
) -> Result<(String, String, String)> {
    let cursor = Cursor::new(installer_bytes.to_vec());
    let mut zip = ZipArchive::new(cursor).context("解析 OptiFine installer jar 失败")?;

    if let Some(buildof) = read_buildof(&mut zip) {
        println!("[OptiFine] 从 buildof.txt 解析版本: {}", buildof);
        if let Some(result) = try_parse_buildof_version(&buildof) {
            return Ok(result);
        }
    }

    let config_paths = [
        "Config.class",
        "net/optifine/Config.class",
        "notch/net/optifine/Config.class",
    ];
    let mut config_bytes = None;
    for p in &config_paths {
        if let Some(b) = read_zip_entry_to_vec(&mut zip, p) {
            config_bytes = Some(b);
            break;
        }
    }
    let config_bytes = config_bytes
        .ok_or_else(|| anyhow!("无法在安装器中找到 Config.class，文件可能损坏"))?;

    match parse_version_from_config_class(&config_bytes) {
        Ok(result) => Ok(result),
        Err(e) => {
            println!(
                "[OptiFine] Config.class 解析失败: {}，尝试从安装器 JAR 文件名推断版本",
                e
            );
            Err(anyhow!(
                "无法从安装器中解析版本信息，请确认安装器文件完整且版本受支持"
            ))
        }
    }
}

fn try_parse_buildof_version(buildof: &str) -> Option<(String, String, String)> {
    let mut mc_version: Option<String> = None;
    let mut of_edition: Option<String> = None;
    let mut of_release: Option<String> = None;

    for line in buildof.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(val) = line.strip_prefix("MC_VERSION=") {
            mc_version = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("MC_VERSION:") {
            mc_version = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_EDITION=") {
            of_edition = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_EDITION:") {
            of_edition = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_RELEASE=") {
            of_release = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_RELEASE:") {
            of_release = Some(val.trim().to_string());
        }
    }

    match (mc_version, of_edition, of_release) {
        (Some(mc), Some(ed), Some(rel)) => {
            let self_version = format!("{}_{}", ed, rel);
            let full_version = format!("{}_{}", mc, self_version);
            Some((mc, self_version, full_version))
        }
        _ => None,
    }
}

fn parse_version_from_config_class(config_bytes: &[u8]) -> Result<(String, String, String)> {
    let mut utf8_strings: Vec<String> = Vec::new();
    let mut i = 0;
    while i + 3 <= config_bytes.len() {
        if config_bytes[i] == 0x01 {
            let length =
                ((config_bytes[i + 1] as usize) << 8) | (config_bytes[i + 2] as usize);
            let start = i + 3;
            if start + length <= config_bytes.len() {
                if let Ok(s) = std::str::from_utf8(&config_bytes[start..start + length]) {
                    utf8_strings.push(s.to_string());
                }
            }
            i = start + length;
        } else {
            i += 1;
        }
    }

    let get_next = |needle: &str| -> Option<String> {
        let idx = utf8_strings.iter().position(|s| s == needle)?;
        utf8_strings.get(idx + 1).cloned()
    };

    let mc_version = get_next("MC_VERSION")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 MC_VERSION"))?;
    let of_edition = get_next("OF_EDITION")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 OF_EDITION"))?;
    let of_release = get_next("OF_RELEASE")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 OF_RELEASE"))?;

    let self_version = format!("{}_{}", of_edition, of_release);
    let full_version = format!("{}_{}", mc_version, self_version);
    Ok((mc_version, self_version, full_version))
}

struct ExtraLibraries {
    libraries: Vec<Value>,
    version_folder_name: String,
    main_class: String,
    game_args_tweaks: Vec<String>,
    /// 以 mod 形式安装到已装 Forge/NeoForge 加载器的 mods 目录（而非作为库加入 classpath）
    install_as_mod: bool,
}

fn run_patcher(
    installer_path: &Path,
    minecraft_jar: &Path,
    output_optifine_lib: &Path,
) -> Result<()> {
    println!("[OptiFine] 调用 optifine.Patcher 修补原版 jar");
    let status = Command::new("java")
        .arg("-cp")
        .arg(installer_path)
        .arg("optifine.Patcher")
        .arg(minecraft_jar)
        .arg(installer_path)
        .arg(output_optifine_lib)
        .status()
        .context("启动 java (optifine.Patcher) 失败，请确认已安装 Java")?;

    if !status.success() {
        bail!(
            "optifine.Patcher 执行失败，退出码: {:?}。\n\
             可能的原因: Java 版本不兼容，或 Minecraft jar 不完整。",
            status.code()
        );
    }
    Ok(())
}

/// 检测该 MC 版本是否已安装 Forge/NeoForge 加载器版本目录（用于 OptiFine 叠加安装）。
///
/// 优先选择 version.json 带 `inheritsFrom: {mc_version}` 的原始加载器版本目录，
/// 其次选择名称含 forge/neoforge 且带 mainClass 的目录（如 HMCL 合并实例）。
pub fn detect_installed_loader_version(mc_dir: &Path, mc_version: &str) -> Option<String> {
    let versions_root = mc_dir.join("versions");
    let mut candidates: Vec<(u8, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&versions_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let lower = dir_name.to_lowercase();
            if !dir_name.starts_with(mc_version)
                || dir_name == mc_version
                || lower.contains("optifine")
                || !(lower.contains("forge") || lower.contains("neoforge"))
            {
                continue;
            }
            let json_path = path.join(format!("{}.json", dir_name));
            let Ok(text) = fs::read_to_string(&json_path) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if v.get("mainClass").and_then(|m| m.as_str()).is_none() {
                continue;
            }
            let score = if v.get("inheritsFrom").and_then(|x| x.as_str()) == Some(mc_version) {
                1
            } else {
                0
            };
            candidates.push((score, dir_name));
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, name)| name)
}

fn process_installer_and_build_json(
    installer_path: &Path,
    mc_dir: &Path,
    mc_version: &str,
    optifine_full_ver: &str,
    optifine_self_ver: &str,
    base_loader: Option<&str>,
) -> Result<ExtraLibraries> {
    let installer_bytes = fs::read(installer_path)
        .with_context(|| format!("读取 OptiFine 安装器失败: {}", installer_path.display()))?;
    let cursor = Cursor::new(installer_bytes.clone());
    let mut installer_zip = ZipArchive::new(cursor).context("解析 OptiFine installer zip 失败")?;

    let maven_version = optifine_full_ver;
    let optifine_lib_name = format!("optifine:OptiFine:{}", maven_version);
    let optifine_installer_lib_name = format!("optifine:OptiFine:{}:installer", maven_version);

    let optifine_lib_path = get_library_file(mc_dir, &optifine_lib_name)?;
    let optifine_installer_lib_path = get_library_file(mc_dir, &optifine_installer_lib_name)?;

    if let Some(parent) = optifine_installer_lib_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::copy(installer_path, &optifine_installer_lib_path)
        .with_context(|| format!("复制 installer 到 libraries 失败: {}", optifine_installer_lib_path.display()))?;
    let _ = remove_mods_toml_from_jar(&optifine_installer_lib_path);

    let has_patcher = zip_entry_exists(&mut installer_zip, "optifine/Patcher.class");
    if has_patcher {
        let versions_root = mc_dir.join("versions");
        let vanilla_jar = versions_root.join(mc_version).join(format!("{}.jar", mc_version));
        if !vanilla_jar.exists() {
            bail!(
                "找不到原版 Minecraft jar: {}\n请先安装原版 Minecraft {}",
                vanilla_jar.display(),
                mc_version
            );
        }
        if let Some(parent) = optifine_lib_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        run_patcher(installer_path, &vanilla_jar, &optifine_lib_path)?;
    } else {
        if let Some(parent) = optifine_lib_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::copy(installer_path, &optifine_lib_path)
            .with_context(|| format!("复制 OptiFine 到 libraries 失败: {}", optifine_lib_path.display()))?;
    }
    let _ = remove_mods_toml_from_jar(&optifine_lib_path);

    let mut libraries: Vec<Value> = Vec::new();
    let mut has_launch_wrapper = false;

    // 以 mod 形式叠加到已装 Forge/NeoForge 时，不加入 launchwrapper/OptiFine 库，
    // 安装完成的 jar 会放入加载器版本的 mods/ 目录。
    if base_loader.is_none() {
        if zip_entry_exists(&mut installer_zip, "launchwrapper-2.0.jar") {
            let lw_lib_name = "optifine:launchwrapper:2.0".to_string();
            let lw_path = get_library_file(mc_dir, &lw_lib_name)?;
            if extract_zip_entry_to_file(&mut installer_zip, "launchwrapper-2.0.jar", &lw_path)? {
                libraries.push(json!({ "name": lw_lib_name }));
                has_launch_wrapper = true;
            }
        }

        if !has_launch_wrapper {
            if let Some(launchwrapper_of_bytes) = read_zip_entry_to_vec(&mut installer_zip, "launchwrapper-of.txt") {
                if let Ok(ver) = String::from_utf8(launchwrapper_of_bytes) {
                    let ver = ver.trim().to_string();
                    let entry_name = format!("launchwrapper-of-{}.jar", ver);
                    let lw_lib_name = format!("optifine:launchwrapper-of:{}", ver);
                    let lw_path = get_library_file(mc_dir, &lw_lib_name)?;
                    if extract_zip_entry_to_file(&mut installer_zip, &entry_name, &lw_path)? {
                        libraries.push(json!({ "name": lw_lib_name }));
                        has_launch_wrapper = true;
                    }
                }
            }
        }

        if !has_launch_wrapper {
            libraries.push(json!({ "name": "net.minecraft:launchwrapper:1.12" }));
        }

        libraries.insert(0, json!({ "name": optifine_lib_name }));
    }

    let buildof = read_buildof(&mut installer_zip);
    if let Some(buildof_str) = buildof.as_ref() {
        println!("[OptiFine] buildof: {}", buildof_str);
    }

    let mc_ver_parts: Vec<&str> = mc_version.split('.').collect();
    let mc_major: i32 = mc_ver_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let (main_class, game_args_tweaks) = if mc_major >= 17 {
        // 优先读取已装加载器版本的 json（PCL/HMCL 行为：OptiFine 叠加 Forge/NeoForge）
        let base_json_path = if let Some(base) = base_loader {
            mc_dir
                .join("versions")
                .join(base)
                .join(format!("{}.json", base))
        } else {
            mc_dir
                .join("versions")
                .join(mc_version)
                .join(format!("{}.json", mc_version))
        };
        let original_main = if let Ok(text) = fs::read_to_string(&base_json_path) {
            serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("mainClass")?.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| VANILLA_MAIN.to_string())
        } else {
            VANILLA_MAIN.to_string()
        };

        if base_loader.is_some()
            || (FORGE_OPTIFINE_COMPATIBLE_MAINS.contains(&original_main.as_str())
                && (original_main == MOD_LAUNCHER_MAIN
                    || original_main == BOOTSTRAP_LAUNCHER_MAIN
                    || original_main == FORGE_BOOTSTRAP_MAIN
                    || original_main == NEO_FORGE_BOOTSTRAP_MAIN))
        {
            println!("[OptiFine] Forge/NeoForge 检测到，OptiFine 将作为 mod 加载（不替换 mainClass）");
            (original_main, Vec::new())
        } else {
            (LAUNCH_WRAPPER_MAIN.to_string(), vec!["--tweakClass".to_string(), "optifine.OptiFineTweaker".to_string()])
        }
    } else {
        (LAUNCH_WRAPPER_MAIN.to_string(), vec!["--tweakClass".to_string(), "optifine.OptiFineTweaker".to_string()])
    };

    let version_folder_name = match base_loader {
        Some(base) => format!("{}-OptiFine-{}", base, optifine_self_ver),
        None => format!("{}-OptiFine-{}", mc_version, optifine_self_ver),
    };

    Ok(ExtraLibraries {
        libraries,
        version_folder_name,
        main_class,
        game_args_tweaks,
        install_as_mod: base_loader.is_some(),
    })
}

fn build_optifine_version_json(
    mc_dir: &Path,
    mc_version: &str,
    extra: &ExtraLibraries,
    optifine_self_ver: &str,
    optifine_full_ver: &str,
    base_loader: Option<&str>,
) -> Result<PathBuf> {
    let versions_root = mc_dir.join("versions");

    // 叠加到已装加载器时以加载器 json 为基础，否则以原版 json 为基础
    let base_json_path = if let Some(base) = base_loader {
        versions_root
            .join(base)
            .join(format!("{}.json", base))
    } else {
        versions_root
            .join(mc_version)
            .join(format!("{}.json", mc_version))
    };
    if !base_json_path.exists() {
        bail!(
            "找不到基础版本 version.json: {:?}",
            base_json_path
        );
    }

    let base_text = fs::read_to_string(&base_json_path)?;
    let mut base: Value = serde_json::from_str(&base_text)
        .context("解析基础版本 version.json 失败")?;

    let instance_id = extra.version_folder_name.clone();
    let base_name = base_loader.unwrap_or(mc_version);

    let obj = base
        .as_object_mut()
        .ok_or_else(|| anyhow!("基础版本 version.json 不是对象"))?;

    obj.insert("id".to_string(), Value::String(instance_id.clone()));
    obj.insert("inheritsFrom".to_string(), Value::String(base_name.to_string()));
    obj.insert("mainClass".to_string(), Value::String(extra.main_class.clone()));
    obj.insert("releaseTime".to_string(), Value::String(iso_now()));
    obj.insert("time".to_string(), Value::String(iso_now()));
    obj.insert("type".to_string(), Value::String("release".to_string()));

    if extra.install_as_mod {
        // Forge/NeoForge 叠加安装：OptiFine 作为 mod 放入加载器版本的 mods/ 目录，
        // 不向 json 注入 OptiFine 库（基础 json 自带加载器全部库）。
        let of_lib_path = get_library_file(mc_dir, &format!("optifine:OptiFine:{}", optifine_full_ver))?;
        let mods_dir = versions_root.join(base_name).join("mods");
        fs::create_dir_all(&mods_dir).ok();
        let mod_jar = mods_dir.join(format!("optifine-{}.jar", optifine_self_ver));
        if of_lib_path.exists() && !mod_jar.exists() {
            fs::copy(&of_lib_path, &mod_jar)
                .with_context(|| format!("复制 OptiFine mod jar 失败: {}", mod_jar.display()))?;
            println!("[OptiFine] 已复制到 mods 目录: {}", mod_jar.display());
        }
    } else if !extra.game_args_tweaks.is_empty() {
        let args_obj = obj
            .get_mut("arguments")
            .and_then(|v| v.as_object_mut());
        if let Some(args) = args_obj {
            let game_arr = args
                .get_mut("game")
                .and_then(|v| v.as_array_mut());
            if let Some(game) = game_arr {
                let mut tweaks: Vec<Value> = extra
                    .game_args_tweaks
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                game.append(&mut tweaks);
            } else {
                args.insert(
                    "game".to_string(),
                    Value::Array(
                        extra
                            .game_args_tweaks
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
        } else {
            let existing = obj
                .get("minecraftArguments")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let appended = format!(
                "{} {} {}",
                existing,
                extra.game_args_tweaks[0],
                extra.game_args_tweaks[1]
            );
            obj.insert("minecraftArguments".to_string(), Value::String(appended));
        }
    }

    let existing_libs = obj
        .get("libraries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // 叠加安装时基础 json 已含加载器全部库，不重复注入 OptiFine 库
    let mut merged = if extra.install_as_mod {
        existing_libs.clone()
    } else {
        let mut libs = extra.libraries.clone();
        libs.extend(existing_libs);
        libs
    };
    obj.insert("libraries".to_string(), Value::Array(merged));

    let target_dir = versions_root.join(&instance_id);
    fs::create_dir_all(&target_dir).ok();

    let target_json = target_dir.join(format!("{}.json", instance_id));
    fs::write(
        &target_json,
        serde_json::to_string_pretty(&base)?,
    )
    .with_context(|| format!("写入 OptiFine version.json 失败: {}", target_json.display()))?;

    println!(
        "[OptiFine] 版本 JSON 已生成: {} (OptiFine {})",
        target_json.display(),
        optifine_full_ver
    );
    let _ = optifine_self_ver;

    if let Err(e) = ensure_options_lang(&target_dir) {
        println!("[OptiFine] 警告: 创建/更新 options.txt 失败: {}", e);
    } else {
        println!("[OptiFine] options.txt 已更新");
    }

    Ok(target_dir)
}

fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let years = 1970 + secs / 31536000;
    format!("{}-06-15T00:00:00+00:00", years)
}

fn ensure_options_lang(versions_dir: &Path) -> Result<()> {
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
    Ok(())
}

pub async fn install_optifine_from_bmcl(
    mc_version: &str,
    optifine_download_url: &str,
    mc_dir: &Path,
) -> Result<String> {
    install_optifine_from_bmcl_with_fallback(
        mc_version,
        optifine_download_url,
        optifine_download_url,
        mc_dir,
    )
    .await
}

pub async fn install_optifine_from_bmcl_with_fallback(
    mc_version: &str,
    optifine_download_url: &str,
    optifine_fallback_url: &str,
    mc_dir: &Path,
) -> Result<String> {
    let installer_path =
        download_optifine_installer_with_fallback(optifine_download_url, optifine_fallback_url, mc_dir)
            .await?;

    let (self_ver, _url_mc_ver) = parse_optifine_version_from_url(optifine_download_url)?;
    let full_ver = format!("{}_{}", mc_version, self_ver);

    println!(
        "[OptiFine] 从 URL 解析版本: MC={}, self={}, full={}",
        mc_version, self_ver, full_ver
    );

    // 若已装 Forge/NeoForge 则叠加安装（PCL/HMCL 行为）
    let base_loader = detect_installed_loader_version(mc_dir, mc_version);
    if let Some(base) = &base_loader {
        println!("[OptiFine] 检测到已安装加载器 {}, OptiFine 将叠加安装", base);
    }

    let extra = process_installer_and_build_json(
        &installer_path,
        mc_dir,
        mc_version,
        &full_ver,
        &self_ver,
        base_loader.as_deref(),
    )?;

    let _ = build_optifine_version_json(
        mc_dir,
        mc_version,
        &extra,
        &self_ver,
        &full_ver,
        base_loader.as_deref(),
    )?;

    Ok(extra.version_folder_name)
}

pub async fn install_optifine_from_local(
    installer_path: &Path,
    mc_dir: &Path,
    mc_version: &str,
) -> Result<String> {
    if !installer_path.exists() {
        bail!("找不到本地 OptiFine 安装器: {}", installer_path.display());
    }
    let installer_bytes = fs::read(installer_path)?;
    let (parsed_mc, self_ver, full_ver) =
        parse_optifine_version_from_installer(&installer_bytes)?;

    if parsed_mc != mc_version {
        bail!(
            "OptiFine 安装器对应的 Minecraft 版本为 {}，期望版本 {}",
            parsed_mc,
            mc_version
        );
    }

    println!(
        "[OptiFine] 本地安装器版本: MC={}, self={}, full={}",
        parsed_mc, self_ver, full_ver
    );

    // 若已装 Forge/NeoForge 则叠加安装（PCL/HMCL 行为）
    let base_loader = detect_installed_loader_version(mc_dir, mc_version);
    if let Some(base) = &base_loader {
        println!("[OptiFine] 检测到已安装加载器 {}, OptiFine 将叠加安装", base);
    }

    let extra = process_installer_and_build_json(
        installer_path,
        mc_dir,
        mc_version,
        &full_ver,
        &self_ver,
        base_loader.as_deref(),
    )?;

    let _ = build_optifine_version_json(
        mc_dir,
        mc_version,
        &extra,
        &self_ver,
        &full_ver,
        base_loader.as_deref(),
    )?;

    Ok(extra.version_folder_name)
}

pub fn create_instance_from_optifine_version(
    mc_dir: &Path,
    mc_version: &str,
    optifine_loader_version: &str,
    instance_name: &str,
) -> Result<()> {
    use crate::downloader::shared_utils::merge_version_jsons_to_instance;

    let clean = sanitize_instance_name(instance_name);
    let default = format!("{}-optifine-{}", mc_version, optifine_loader_version);
    let final_name = if clean.trim().is_empty() {
        sanitize_instance_name(&default)
    } else {
        clean
    };
    merge_version_jsons_to_instance(
        &final_name,
        mc_version,
        optifine_loader_version,
        "optifine",
        mc_dir,
    )?;
    Ok(())
}