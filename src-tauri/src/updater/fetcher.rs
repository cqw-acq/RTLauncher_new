use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::config::{
    get_current_arch_keywords, get_current_os, get_current_os_keywords, get_update_config,
    get_update_endpoints, is_trusted_download_url, is_trusted_release_asset_url,
    is_valid_sha256_hex, matches_asset_name_for, save_update_config, should_check_update,
    UpdateStatus, HASH_TAG,
};

#[derive(Debug, Clone, Deserialize)]
struct WireAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum WireReleaseBody {
    Text(String),
    Structured {
        #[serde(default)]
        update_info: String,
        #[serde(default)]
        sum: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct WireRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<WireReleaseBody>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<WireAsset>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WireReleaseResponse {
    Single(WireRelease),
    Multiple(Vec<WireRelease>),
}

#[derive(Debug, Clone)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    platform: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseSource {
    LightingTeam,
    GitHub,
}

impl ReleaseSource {
    fn priority(self) -> u8 {
        match self {
            Self::LightingTeam => 1,
            Self::GitHub => 0,
        }
    }
}

#[derive(Debug, Clone)]
struct NormalizedRelease {
    version: Version,
    notes: String,
    prerelease: bool,
    draft: bool,
    assets: Vec<ReleaseAsset>,
    checksums: HashMap<String, String>,
    source: ReleaseSource,
}

fn parse_semver(input: &str) -> Option<Version> {
    let trimmed = input.trim().trim_start_matches(['v', 'V']);
    if let Ok(version) = Version::parse(trimmed) {
        return Some(version);
    }

    let re = regex::Regex::new(r"(?i)\bv?(\d+\.\d+\.\d+(?:-[0-9a-z.-]+)?)\b").ok()?;
    let value = re.captures(input)?.get(1)?.as_str();
    Version::parse(value).ok()
}

fn parse_release_response_for_source(
    json: &str,
    source: ReleaseSource,
) -> Result<Vec<NormalizedRelease>, String> {
    let response: WireReleaseResponse =
        serde_json::from_str(json).map_err(|e| format!("解析更新信息失败: {}", e))?;
    let releases = match response {
        WireReleaseResponse::Single(release) => vec![release],
        WireReleaseResponse::Multiple(releases) => releases,
    };

    Ok(releases
        .into_iter()
        .filter_map(|release| {
            let version = parse_semver(&release.tag_name)
                .or_else(|| release.name.as_deref().and_then(parse_semver))?;

            let (notes, checksums) = match release.body {
                Some(WireReleaseBody::Structured { update_info, sum }) => (
                    update_info,
                    sum.into_iter()
                        .map(|(name, hash)| (name.to_lowercase(), hash.to_lowercase()))
                        .collect(),
                ),
                Some(WireReleaseBody::Text(text)) => {
                    let checksums = UpdateFetcher::parse_checksums_from_body(&text, HASH_TAG)
                        .into_iter()
                        .collect();
                    (text, checksums)
                }
                None => (String::new(), HashMap::new()),
            };

            Some(NormalizedRelease {
                version,
                notes,
                prerelease: release.prerelease,
                draft: release.draft,
                assets: release
                    .assets
                    .into_iter()
                    .map(|asset| ReleaseAsset {
                        name: asset.name,
                        browser_download_url: asset.browser_download_url,
                        platform: asset.platform,
                        size: asset.size,
                    })
                    .collect(),
                checksums,
                source,
            })
        })
        .collect())
}

fn parse_release_response(json: &str) -> Result<Vec<NormalizedRelease>, String> {
    let source = if json.trim_start().starts_with('[') {
        ReleaseSource::GitHub
    } else {
        ReleaseSource::LightingTeam
    };
    parse_release_response_for_source(json, source)
}

fn is_stable_release(release: &NormalizedRelease) -> bool {
    !release.prerelease && !release.draft && release.version.pre.is_empty()
}

fn select_latest_release<'a>(
    releases: &'a [NormalizedRelease],
    current_version: &str,
) -> Option<&'a NormalizedRelease> {
    let current = parse_semver(current_version)?;
    releases
        .iter()
        .filter(|release| is_stable_release(release) && release.version > current)
        .max_by(|left, right| {
            left.version
                .cmp(&right.version)
                .then_with(|| left.source.priority().cmp(&right.source.priority()))
        })
}

fn select_latest_candidate<'a>(
    releases: &'a [NormalizedRelease],
    current_version: &str,
    current_os: &str,
) -> Option<(&'a NormalizedRelease, &'a ReleaseAsset)> {
    let latest_version = select_latest_release(releases, current_version)?
        .version
        .clone();
    releases
        .iter()
        .filter(|release| is_stable_release(release) && release.version == latest_version)
        .filter_map(|release| {
            UpdateFetcher::select_asset(release, current_os).map(|asset| (release, asset))
        })
        .max_by_key(|(release, _)| release.source.priority())
}

fn available_update_result(
    current_version: &str,
    release: &NormalizedRelease,
) -> UpdateCheckResult {
    let changelog = (!release.notes.trim().is_empty()).then(|| release.notes.clone());
    UpdateCheckResult {
        needs_check: true,
        update_available: true,
        current_version: current_version.to_string(),
        target_version: Some(release.version.to_string()),
        message: "发现新版本可用".to_string(),
        changelog,
    }
}

fn filename_from_download_url(download_url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(download_url).map_err(|e| format!("更新下载地址无效: {}", e))?;
    let encoded_name = parsed
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .ok_or_else(|| "更新下载地址缺少文件名".to_string())?;
    urlencoding::decode(encoded_name)
        .map(|name| name.into_owned())
        .map_err(|e| format!("更新文件名编码无效: {}", e))
}

#[derive(Clone)]
pub struct UpdateFetcher {
    cancel_flag: Arc<AtomicBool>,
    is_downloading: Arc<AtomicBool>,
}

/// RAII 守卫：构造时把 `is_downloading` 置为 true；`drop()` 时无论函数如何退出（panic、?、break），
/// 都一定会把该原子标志重置，防止下载失败后启动器永远卡在"下载中"无法重试。
///
/// 同时在 drop 时检查状态机：如果仍然停留在 `Downloading`，说明是异常退出，
/// 需要把状态写成 `Error(...)` 并删除半下载文件。
struct DownloadGuard {
    flag: Arc<AtomicBool>,
    active: bool,
    /// 已计算好的下载目标路径，异常退出时用于清理部分下载文件。
    cleanup_path: Option<PathBuf>,
}

impl DownloadGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::SeqCst);
        Self {
            flag,
            active: true,
            cleanup_path: None,
        }
    }

    /// 设置下载路径（在文件名合法检查通过后调用），guard 异常 drop 时会尝试删除它。
    fn set_cleanup_path(&mut self, p: PathBuf) {
        self.cleanup_path = Some(p);
    }

    /// 成功结束时调用：取消 guard 的副作用。
    fn defuse(mut self) {
        self.active = false;
        self.cleanup_path = None;
    }

    /// 调用方已经自己处理了状态，只想立刻重置标志/取消清理路径。
    fn manual_reset(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
        if !self.active {
            return;
        }
        // active=true 代表函数没有走成功路径，fallback 地把状态写成 Error（只在仍为 Downloading 时）
        let mut cfg = get_update_config();
        if matches!(cfg.status, UpdateStatus::Downloading) {
            cfg.status = UpdateStatus::Error("下载过程中出现异常，已中止".to_string());
            cfg.download_progress = None;
            let _ = save_update_config(cfg);
        }
        if let Some(p) = self.cleanup_path.take() {
            let _ = fs::remove_file(&p);
        }
    }
}

impl UpdateFetcher {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            is_downloading: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub fn is_downloading(&self) -> bool {
        self.is_downloading.load(Ordering::SeqCst)
    }

    fn score_asset_match(asset_name: &str) -> i32 {
        let name_lower = asset_name.to_lowercase();
        let mut score = 0;

        let os_keywords = get_current_os_keywords();
        let arch_keywords = get_current_arch_keywords();

        let mut has_os = false;
        let mut has_arch = false;

        for ok in &os_keywords {
            if name_lower.contains(ok) {
                has_os = true;
                score += 10;
                break;
            }
        }

        for ak in &arch_keywords {
            if name_lower.contains(ak) {
                has_arch = true;
                score += 20;
                break;
            }
        }

        if has_os && has_arch {
            score += 50;
        }

        let os = get_current_os();
        let os_underscore = os.replace('-', "_");
        if name_lower.contains(&os) || name_lower.contains(&os_underscore) {
            score += 30;
        }

        for ok in &os_keywords {
            for ak in &arch_keywords {
                let p1 = format!("{}_{}", ok, ak);
                let p2 = format!("{}-{}", ok, ak);
                let p3 = format!("{}_{}", ak, ok);
                let p4 = format!("{}-{}", ak, ok);
                if name_lower.contains(&p1)
                    || name_lower.contains(&p2)
                    || name_lower.contains(&p3)
                    || name_lower.contains(&p4)
                {
                    score += 100;
                    break;
                }
            }
        }

        let exclude_os_keywords: Vec<&str> = match os.as_str() {
            s if s.starts_with("windows") => {
                vec!["macos", "darwin", "linux", "appimage", "deb", "dmg"]
            }
            s if s.starts_with("macos") => vec![
                "windows", "win", "linux", "appimage", "deb", "msi", "nsis", ".exe",
            ],
            s if s.starts_with("linux") => vec![
                "windows", "win", "macos", "darwin", "dmg", "msi", "nsis", ".exe",
            ],
            _ => vec![],
        };

        let mut has_exclude = false;
        for ek in &exclude_os_keywords {
            if name_lower.contains(ek) {
                has_exclude = true;
                score -= 50;
            }
        }

        if !has_exclude
            && !name_lower.contains(".zip")
            && !name_lower.contains(".tar")
            && !name_lower.contains(".png")
            && !name_lower.contains(".json")
        {
            score += 5;
        }

        if name_lower.contains("setup") || name_lower.contains("installer") {
            score += 8;
        }

        score
    }

    /// 从 Release body 里解析出 SHA256SUMS 列表。支持以下格式（每行一条）：
    ///   - `SHA256 (RTLauncher-windows-x86_64.zip) = a5c2d9...`
    ///   - `a5c2d9f...  RTLauncher-windows-x86_64.zip`
    ///   - `a5c2d9f... *RTLauncher-windows-x86_64.zip`
    ///   - 代码块 ```SHA256SUMS ... ``` 包裹的块
    /// 返回 `(asset_filename_lowercase, lowercase_hex_hash_64)` 列表。
    fn parse_checksums_from_body(body: &str, tag: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();

        // 1) 优先提取 tag 对应的代码块
        let mut search_space: String = body.to_string();
        let fence_start = format!("```{}", tag);
        if let Some(idx) = body.to_lowercase().find(&fence_start.to_lowercase()) {
            let after = &body[idx + fence_start.len()..];
            if let Some(end) = after.find("```") {
                search_space = after[..end].to_string();
            }
        }

        for raw_line in search_space.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            // 形式 A: SHA256 (name) = hash
            let mut it = line.split('=');
            if let (Some(left), Some(right)) = (it.next(), it.next()) {
                let left_s = left.trim();
                let hash = right.trim().to_lowercase();
                if let (Some(lp), Some(rp)) = (left_s.rfind('('), left_s.rfind(')')) {
                    if rp > lp && is_valid_sha256_hex(&hash) {
                        let name = left_s[lp + 1..rp].trim().to_lowercase();
                        if !name.is_empty() {
                            out.push((name, hash));
                            continue;
                        }
                    }
                }
            }

            // 形式 B/C: hash  name / hash *name (两列)
            let mut parts = line.splitn(2, char::is_whitespace);
            let Some(hash_piece) = parts.next() else {
                continue;
            };
            let Some(name_piece) = parts.next() else {
                continue;
            };
            let hash = hash_piece.trim().to_lowercase();
            let name = name_piece
                .trim_start_matches([' ', '\t', '*'])
                .trim()
                .to_lowercase();
            if is_valid_sha256_hex(&hash) && !name.is_empty() {
                out.push((name, hash));
            }
        }

        out
    }

    /// 计算磁盘文件的 SHA-256，返回小写十六进制 64 字符串。
    fn sha256_of_file(path: &PathBuf) -> Result<String, String> {
        let mut f = fs::File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = f
                .read(&mut buf)
                .map_err(|e| format!("读取文件失败: {}", e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let digest = hasher.finalize();
        Ok(hex_encode_lower(&digest))
    }

    async fn fetch_source_releases(
        client: &reqwest::Client,
        endpoint: &str,
        source: ReleaseSource,
    ) -> Result<Vec<NormalizedRelease>, String> {
        let response = client
            .get(endpoint)
            .header(
                "User-Agent",
                "RTLauncher updater (+https://github.com/cqw-acq/RTLauncher_new)",
            )
            .header("Accept", "application/vnd.github+json, application/json")
            .send()
            .await
            .map_err(|e| format!("请求 {} 失败: {}", endpoint, e))?;

        if !response.status().is_success() {
            return Err(format!("{} 返回 HTTP {}", endpoint, response.status()));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("读取 {} 响应失败: {}", endpoint, e))?;
        parse_release_response_for_source(&body, source).map_err(|e| format!("{}: {}", endpoint, e))
    }

    fn platform_matches_current(platform: &str, current_os: &str) -> bool {
        matches!(
            (platform.to_lowercase().as_str(), current_os),
            ("windows-x64", "windows-x86_64")
                | ("windows-x86_64", "windows-x86_64")
                | ("windows-amd64", "windows-x86_64")
                | ("windows-arm64", "windows-aarch64")
                | ("windows-aarch64", "windows-aarch64")
                | ("macos-x64", "macos-x86_64")
                | ("macos-x86_64", "macos-x86_64")
                | ("macos-arm64", "macos-aarch64")
                | ("macos-aarch64", "macos-aarch64")
                | ("linux-x64", "linux-x86_64")
                | ("linux-x86_64", "linux-x86_64")
                | ("linux-amd64", "linux-x86_64")
                | ("linux-arm64", "linux-aarch64")
                | ("linux-aarch64", "linux-aarch64")
        )
    }

    fn select_asset<'a>(
        release: &'a NormalizedRelease,
        current_os: &str,
    ) -> Option<&'a ReleaseAsset> {
        let mut assets: Vec<(i32, &ReleaseAsset)> = release
            .assets
            .iter()
            .filter(|asset| {
                let platform_matches = asset
                    .platform
                    .as_deref()
                    .map(|platform| Self::platform_matches_current(platform, current_os))
                    .unwrap_or_else(|| matches_asset_name_for(&asset.name, current_os));
                platform_matches && is_trusted_release_asset_url(&asset.browser_download_url)
            })
            .map(|asset| {
                let platform_bonus = if asset.platform.is_some() { 1_000 } else { 0 };
                (platform_bonus + Self::score_asset_match(&asset.name), asset)
            })
            .collect();
        assets.sort_by(|left, right| right.0.cmp(&left.0));
        assets.first().map(|(_, asset)| *asset)
    }

    fn checksum_for(release: &NormalizedRelease, asset_name: &str) -> Option<String> {
        let needle = asset_name.to_lowercase();
        release
            .checksums
            .iter()
            .find(|(name, _)| {
                *name == &needle
                    || name.ends_with(&format!("/{}", needle))
                    || needle.ends_with(name.as_str())
            })
            .map(|(_, hash)| hash.clone())
    }

    pub async fn check_for_update(
        &self,
        force: bool,
        current_version: &str,
    ) -> Result<UpdateCheckResult, String> {
        if !force && !should_check_update() {
            let cfg = get_update_config();
            let cached_update_available = cfg
                .target_version
                .as_deref()
                .and_then(parse_semver)
                .zip(parse_semver(current_version))
                .map(|(target, current)| target > current)
                .unwrap_or(false);
            return Ok(UpdateCheckResult {
                needs_check: false,
                update_available: cached_update_available,
                current_version: current_version.to_string(),
                target_version: cfg.target_version,
                message: "距上次检查不足5秒，请稍后再试".to_string(),
                changelog: cfg.changelog,
            });
        }

        self.cancel_flag.store(false, Ordering::SeqCst);

        let mut cfg = get_update_config();
        cfg.current_version = current_version.to_string();
        cfg.status = UpdateStatus::Checking;
        cfg.last_check_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );
        let _ = save_update_config(cfg.clone());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
        let [lighting_team_endpoint, github_endpoint] = get_update_endpoints();
        let (lighting_team_result, github_result) = tokio::join!(
            Self::fetch_source_releases(
                &client,
                lighting_team_endpoint,
                ReleaseSource::LightingTeam
            ),
            Self::fetch_source_releases(&client, github_endpoint, ReleaseSource::GitHub)
        );

        let mut releases = Vec::new();
        let mut errors = Vec::new();
        for result in [lighting_team_result, github_result] {
            match result {
                Ok(mut source_releases) => releases.append(&mut source_releases),
                Err(error) => errors.push(error),
            }
        }

        if releases.is_empty() && errors.len() == 2 {
            let message = format!("所有更新源均不可用: {}", errors.join("；"));
            cfg.status = UpdateStatus::Error(message.clone());
            let _ = save_update_config(cfg);
            return Err(message);
        }
        for error in &errors {
            eprintln!("[更新器] 更新源不可用，已使用其他来源: {}", error);
        }

        let current_os = get_current_os();
        let selected_release = select_latest_release(&releases, current_version);
        let selected_candidate = select_latest_candidate(&releases, current_version, &current_os);

        let Some(release) = selected_release else {
            cfg.target_version = None;
            cfg.changelog = None;
            cfg.target_os = None;
            cfg.download_url = None;
            cfg.file_size = None;
            cfg.expected_sha256 = None;
            cfg.status = UpdateStatus::Idle;
            let _ = save_update_config(cfg);
            return Ok(UpdateCheckResult {
                needs_check: true,
                update_available: false,
                current_version: current_version.to_string(),
                target_version: None,
                message: if releases.is_empty() {
                    "当前没有可用的发布版本".to_string()
                } else {
                    "当前已是最新版本".to_string()
                },
                changelog: None,
            });
        };

        let Some((release, asset)) = selected_candidate else {
            let message = format!(
                "已找到最新版本 {}，但未找到适配当前系统 ({}) 的可信安装包",
                release.version, current_os
            );
            cfg.target_version = None;
            cfg.changelog = None;
            cfg.target_os = None;
            cfg.download_url = None;
            cfg.file_size = None;
            cfg.expected_sha256 = None;
            cfg.status = UpdateStatus::Error(message.clone());
            let _ = save_update_config(cfg);
            return Ok(UpdateCheckResult {
                needs_check: true,
                update_available: false,
                current_version: current_version.to_string(),
                target_version: None,
                message,
                changelog: None,
            });
        };

        let release_version = release.version.to_string();
        cfg.target_version = Some(release_version.clone());
        cfg.changelog = (!release.notes.trim().is_empty()).then(|| release.notes.clone());
        cfg.target_os = Some(current_os);
        cfg.download_url = Some(asset.browser_download_url.clone());
        cfg.file_size = asset.size;
        cfg.expected_sha256 = Self::checksum_for(release, &asset.name);
        cfg.status = UpdateStatus::Available;
        let _ = save_update_config(cfg);

        Ok(available_update_result(current_version, release))
    }

    pub async fn download_update(&self) -> Result<DownloadResult, String> {
        if self.is_downloading.load(Ordering::SeqCst) {
            return Err("已有更新正在下载中".to_string());
        }

        let cfg_before = get_update_config();
        let download_url = match &cfg_before.download_url {
            Some(url) => url.clone(),
            None => return Err("没有可下载的更新".to_string()),
        };

        // 进入下载前再次核对 URL 白名单，防止配置被篡改（此时还没置 is_downloading=true）
        if !is_trusted_release_asset_url(&download_url) {
            return Err(format!(
                "拒绝下载：URL 不在受信任域名单中: {}",
                download_url
            ));
        }

        // ========= 从这里开始所有提前退出都必须保证 is_downloading 被重置 =========
        // 使用 RAII guard 处理 finally 式清理；active=true 的异常 drop 会：
        //   1) 把 is_downloading=false（无论何种路径一定发生）
        //   2) 如果状态仍是 Downloading → 写成 Error(...)，避免 UI 卡住
        //   3) 如果已 set_cleanup_path → 删除部分下载文件
        let mut guard = DownloadGuard::new(Arc::clone(&self.is_downloading));

        self.cancel_flag.store(false, Ordering::SeqCst);

        let mut cfg = cfg_before;
        cfg.status = UpdateStatus::Downloading;
        cfg.download_progress = Some(0.0);
        let _ = save_update_config(cfg.clone());

        // ------ 下面的所有 ? 都会触发 guard drop 做清理 ------

        let config_dir = super::config::config_dir();
        let download_dir = PathBuf::from(&config_dir).join("updates");
        fs::create_dir_all(&download_dir).map_err(|e| format!("创建下载目录失败: {}", e))?;

        let target_filename = filename_from_download_url(&download_url)?;
        let download_path = download_dir.join(&target_filename);

        // 拒绝跨目录文件名：下载文件名里只能有正常字符，防止路径穿越
        if target_filename.contains("..")
            || target_filename.contains('/')
            || target_filename.contains('\\')
            || target_filename.is_empty()
        {
            let msg = format!("非法的更新文件名: {}", target_filename);
            guard.active = false; // 自己下面会写 Error 状态，避免 on_error 覆盖
            drop(guard);
            let mut cfg = get_update_config();
            cfg.status = UpdateStatus::Error(msg.clone());
            cfg.download_progress = None;
            let _ = save_update_config(cfg);
            return Err(msg);
        }

        // 告诉 guard 如果异常退出就删除这个半下载文件
        guard.set_cleanup_path(download_path.clone());

        // 重定向策略：只能重定向到白名单域名，拒绝跳转到其他站点（fail closed）
        let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
            if is_trusted_download_url(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .redirect(redirect_policy)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let response = client
            .get(&download_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "*/*")
            .send()
            .await
            .map_err(|e| {
                let mut cfg = get_update_config();
                let msg = format!("下载更新失败: {}", e);
                cfg.status = UpdateStatus::Error(msg.clone());
                cfg.download_progress = None;
                let _ = save_update_config(cfg);
                msg
            })?;

        if !response.status().is_success() {
            let msg = format!("下载失败: HTTP {}", response.status());
            guard.active = false;
            drop(guard);
            let mut cfg = get_update_config();
            cfg.status = UpdateStatus::Error(msg.clone());
            cfg.download_progress = None;
            let _ = fs::remove_file(&download_path);
            let _ = save_update_config(cfg);
            return Err(msg);
        }

        let total_size = response.content_length();

        let mut file =
            fs::File::create(&download_path).map_err(|e| format!("创建临时文件失败: {}", e))?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        while let Some(chunk) = stream.next().await {
            if self.cancel_flag.load(Ordering::SeqCst) {
                guard.manual_reset(); // 立刻解锁，避免 UI 还要等下面 drop
                guard.active = false;
                let _ = fs::remove_file(&download_path);
                let mut cfg = get_update_config();
                cfg.status = UpdateStatus::Idle;
                cfg.download_progress = None;
                let _ = save_update_config(cfg);
                return Err("下载已取消".to_string());
            }

            let chunk = chunk.map_err(|e| format!("下载数据错误: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;

            downloaded += chunk.len() as u64;

            if let Some(total) = total_size {
                let progress = (downloaded as f64 / total as f64) * 100.0;
                let mut cfg = get_update_config();
                cfg.download_progress = Some(progress);
                let _ = save_update_config(cfg);
            }
        }

        drop(file);
        guard.manual_reset(); // is_downloading=false 立即生效（后续不涉及并发下载了）

        // ==== 完整性校验（fail closed） ====
        if let Some(expected) = cfg.expected_sha256.as_ref() {
            if !is_valid_sha256_hex(expected) {
                let msg = format!("更新配置中的期望哈希无效，拒绝安装: {}", expected);
                guard.active = false;
                drop(guard);
                let mut cfg = get_update_config();
                cfg.status = UpdateStatus::Error(msg.clone());
                cfg.download_progress = None;
                let _ = fs::remove_file(&download_path);
                let _ = save_update_config(cfg);
                return Err(msg);
            }

            let actual = Self::sha256_of_file(&download_path).map_err(|e| {
                let msg = format!("无法计算文件哈希: {}", e);
                let mut cfg = get_update_config();
                cfg.status = UpdateStatus::Error(msg.clone());
                let _ = save_update_config(cfg);
                msg
            })?;

            if &actual != expected {
                let msg = format!(
                    "更新文件 SHA-256 校验失败！\n期望: {}\n实际: {}",
                    expected, actual
                );
                guard.active = false;
                drop(guard);
                let mut cfg = get_update_config();
                cfg.status = UpdateStatus::Error(msg.clone());
                cfg.download_progress = None;
                let _ = fs::remove_file(&download_path);
                let _ = save_update_config(cfg);
                return Err(msg);
            }

            eprintln!(
                "[更新器] SHA-256 校验通过: {} (期望={}, 实际={})",
                download_path.display(),
                expected,
                actual
            );
        }

        let mut cfg = get_update_config();
        cfg.status = UpdateStatus::Downloaded;
        cfg.download_progress = Some(100.0);
        cfg.download_path = Some(download_path.to_string_lossy().to_string());
        let _ = save_update_config(cfg);

        // ========== 所有步骤成功，取消 guard 的 on_error 副作用 ==========
        guard.defuse();

        Ok(DownloadResult {
            success: true,
            path: download_path.to_string_lossy().to_string(),
            size: downloaded,
        })
    }

    pub async fn install_update(&self) -> Result<InstallResult, String> {
        let cfg = get_update_config();
        let download_path_str = match &cfg.download_path {
            Some(path) => path.clone(),
            None => return Err("No downloaded update available".to_string()),
        };

        let download_path = PathBuf::from(&download_path_str);
        if !download_path.exists() {
            return Err("Update file does not exist".to_string());
        }

        // 安装前最后一道防线：如果 expected_sha256 有值就必须校验通过（fail closed）
        // 即便 download_update 已校验过，这里再检查一次可防止下载后到安装前的 TOCTOU 修改
        if let Some(expected) = cfg.expected_sha256.as_ref() {
            if !is_valid_sha256_hex(expected) {
                return Err(format!("期望哈希无效，拒绝安装: {}", expected));
            }
            match Self::sha256_of_file(&download_path) {
                Ok(ref actual) if actual == expected => {
                    eprintln!("[更新器-安装前] 再次校验 SHA-256 通过: {}", expected);
                }
                Ok(actual) => {
                    return Err(format!(
                        "安装前 SHA-256 校验失败！\n期望: {}\n实际: {}",
                        expected, actual
                    ));
                }
                Err(e) => {
                    return Err(format!("安装前无法读取更新文件: {}", e));
                }
            }
        }

        let current_exe =
            std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

        let exe_dir = current_exe
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .to_path_buf();

        let temp_dir = exe_dir.join("update_temp");
        let _ = fs::create_dir_all(&temp_dir);

        let extracted = self.extract_if_needed(&download_path, &temp_dir)?;

        let backup_dir = exe_dir.join("backup");
        let _ = fs::create_dir_all(&backup_dir);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let current_os = get_current_os();

        if current_os.starts_with("windows") {
            self.install_windows(
                &download_path,
                &temp_dir,
                &backup_dir,
                timestamp,
                extracted,
                &exe_dir,
            )?;
        } else if current_os.starts_with("macos") {
            self.install_macos(
                &download_path_str,
                &temp_dir,
                &backup_dir,
                timestamp,
                extracted,
                &exe_dir,
            )?;
        } else if current_os.starts_with("linux") {
            self.install_linux(
                &download_path,
                &temp_dir,
                &backup_dir,
                timestamp,
                extracted,
                &exe_dir,
            )?;
        } else {
            return Err("Unsupported OS for update".to_string());
        }

        Ok(InstallResult {
            success: true,
            message: "Installer started".to_string(),
        })
    }

    /// 当前运行中的可执行文件名（portable 为小写 `rtlauncher`，安装版为大写）。
    /// 拿不到时按平台回退到历史硬编码名字。
    fn current_exe_file_name() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| match get_current_os() {
                os if os.starts_with("windows") => "RTLauncher.exe".to_string(),
                os if os.starts_with("macos") => "RTLauncher".to_string(),
                _ => "RTLauncher".to_string(),
            })
    }

    fn install_windows(
        &self,
        download_path: &PathBuf,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        let target_exe = exe_dir.join(Self::current_exe_file_name());
        if target_exe.exists() {
            let backup_path = backup_dir.join(format!("RTLauncher_backup_{}.exe", timestamp));
            let _ = fs::copy(&target_exe, &backup_path);
        }

        let source_file = if extracted {
            self.find_executable_in_dir(temp_dir, "windows")?
        } else {
            download_path.clone()
        };

        let script = format!(
            "@echo off\n\
            chcp 65001 >nul\n\
            echo Installing update...\n\
            timeout /t 3 /nobreak >nul\n\
            copy /y \"{source}\" \"{target}\"\n\
            if %errorlevel% neq 0 (\n\
                echo Install failed\n\
                pause\n\
                exit /b 1\n\
            )\n\
            echo Install complete, restarting...\n\
            start \"\" \"{target}\"\n\
            timeout /t 2 /nobreak >nul\n\
            rmdir /s /q \"{temp}\"\n\
            del \"%~f0\"",
            source = source_file.to_string_lossy(),
            target = target_exe.to_string_lossy(),
            temp = temp_dir.to_string_lossy()
        );

        let script_path = exe_dir.join("update_installer.bat");
        fs::write(&script_path, &script)
            .map_err(|e| format!("Write install script failed: {}", e))?;

        std::process::Command::new("cmd")
            .args(["/C", script_path.to_string_lossy().as_ref()])
            .spawn()
            .map_err(|e| format!("Start installer failed: {}", e))?;

        std::process::exit(0);
    }

    fn install_macos(
        &self,
        download_path_str: &str,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        if extracted {
            let target_app = self.find_app_bundle_in_dir(temp_dir)?;
            let target_path = exe_dir.join("RTLauncher.app");

            if target_path.exists() {
                let backup_path = backup_dir.join(format!("RTLauncher_backup_{}.app", timestamp));
                let _ = self.copy_dir_recursive(&target_path, &backup_path);
            }

            let script = format!(
                "#!/bin/bash\n\
                set -e\n\
                echo \"Installing update...\"\n\
                sleep 3\n\
                rm -rf \"{target}\"\n\
                cp -R \"{source}\" \"{target}\"\n\
                echo \"Install complete, restarting...\"\n\
                open \"{target}\"\n\
                rm -rf \"{temp}\"\n\
                rm -- \"$0\"",
                source = target_app.to_string_lossy(),
                target = target_path.to_string_lossy(),
                temp = temp_dir.to_string_lossy()
            );

            let script_path = exe_dir.join("update_installer.sh");
            fs::write(&script_path, &script)
                .map_err(|e| format!("Write install script failed: {}", e))?;

            self.make_executable(&script_path)?;

            // 注意：不要用 `open <script.sh>` 来执行脚本！`open` 会走 LaunchServices，
            // 可能把 .sh 扔到文本编辑器里，不保证执行。必须显式交给 /bin/bash 运行，
            // 并用 nohup + setsid 让它在父进程 exit(0) 后继续存活完成复制/重启。
            let detached = std::process::Command::new("/bin/bash")
                .args([
                    "-c",
                    "nohup /bin/bash \"$0\" </dev/null >/dev/null 2>&1 & disown",
                    script_path.to_string_lossy().as_ref(),
                ])
                .spawn()
                .map_err(|e| format!("Start installer failed: {}", e))?;
            let _ = detached;

            std::process::exit(0);
        } else {
            let target_binary = exe_dir.join(Self::current_exe_file_name());
            if target_binary.exists() {
                let backup_path = backup_dir.join(format!("RTLauncher_backup_{}", timestamp));
                let _ = fs::copy(&target_binary, &backup_path);
            }

            let script = format!(
                "#!/bin/bash\n\
                set -e\n\
                echo \"Installing update...\"\n\
                sleep 3\n\
                cp \"{source}\" \"{target}\"\n\
                chmod +x \"{target}\"\n\
                echo \"Install complete, restarting...\"\n\
                nohup \"{target}\" </dev/null >/dev/null 2>&1 & disown\n\
                rm -rf \"{temp}\"\n\
                rm -- \"$0\"",
                source = download_path_str,
                target = target_binary.to_string_lossy(),
                temp = temp_dir.to_string_lossy()
            );

            let script_path = exe_dir.join("update_installer.sh");
            fs::write(&script_path, &script)
                .map_err(|e| format!("Write install script failed: {}", e))?;

            self.make_executable(&script_path)?;

            // 同上：显式 /bin/bash + nohup/setsid/detach，父进程退出后脚本继续跑
            let detached = std::process::Command::new("/bin/bash")
                .args([
                    "-c",
                    "nohup /bin/bash \"$0\" </dev/null >/dev/null 2>&1 & disown",
                    script_path.to_string_lossy().as_ref(),
                ])
                .spawn()
                .map_err(|e| format!("Start installer failed: {}", e))?;
            let _ = detached;

            std::process::exit(0);
        }
    }

    fn install_linux(
        &self,
        download_path: &PathBuf,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        let target_binary = exe_dir.join(Self::current_exe_file_name());
        if target_binary.exists() {
            let backup_path = backup_dir.join(format!("RTLauncher_backup_{}", timestamp));
            let _ = fs::copy(&target_binary, &backup_path);
        }

        let source_file = if extracted {
            self.find_executable_in_dir(temp_dir, "linux")?
        } else {
            download_path.clone()
        };

        let script = format!(
            "#!/bin/bash\n\
            set -e\n\
            echo \"Installing update...\"\n\
            sleep 3\n\
            cp \"{source}\" \"{target}\"\n\
            chmod +x \"{target}\"\n\
            echo \"Install complete, restarting...\"\n\
            nohup \"{target}\" &\n\
            rm -rf \"{temp}\"\n\
            rm -- \"$0\"",
            source = source_file.to_string_lossy(),
            target = target_binary.to_string_lossy(),
            temp = temp_dir.to_string_lossy()
        );

        let script_path = exe_dir.join("update_installer.sh");
        fs::write(&script_path, &script)
            .map_err(|e| format!("Write install script failed: {}", e))?;

        self.make_executable(&script_path)?;

        // 让安装脚本在父进程退出后继续存活：nohup + setsid + 重定向到 /dev/null
        let detached = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "nohup setsid /bin/sh \"$0\" </dev/null >/dev/null 2>&1 &",
                script_path.to_string_lossy().as_ref(),
            ])
            .spawn()
            .map_err(|e| format!("Start installer failed: {}", e))?;
        let _ = detached;

        std::process::exit(0);
    }

    #[allow(unused_variables)]
    fn make_executable(&self, path: &PathBuf) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(path, perms)
                .map_err(|e| format!("Set script permissions failed: {}", e))?;
        }
        Ok(())
    }

    fn extract_if_needed(&self, source: &PathBuf, dest: &PathBuf) -> Result<bool, String> {
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "zip" => {
                let file = fs::File::open(source).map_err(|e| e.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

                let canonical_dest = dest
                    .canonicalize()
                    .or_else(|_| {
                        fs::create_dir_all(dest).ok();
                        dest.canonicalize()
                    })
                    .map_err(|e| format!("无法解析目标目录: {}", e))?;

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).map_err(|e| e.to_string())?;

                    let safe_rel_path = file.enclosed_name().ok_or_else(|| {
                        format!("非法的 zip 条目路径（可能包含路径遍历）: {}", file.name())
                    })?;

                    if safe_rel_path.is_absolute() {
                        return Err(format!("拒绝解压绝对路径条目: {}", file.name()));
                    }

                    if safe_rel_path
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir))
                    {
                        return Err(format!("拒绝解压包含上级目录引用的条目: {}", file.name()));
                    }

                    let outpath = canonical_dest.join(&safe_rel_path);

                    if file.is_dir() {
                        fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            fs::create_dir_all(p).map_err(|e| e.to_string())?;
                        }

                        if let Ok(real_parent) = outpath.parent().unwrap().canonicalize() {
                            let real_final = real_parent.join(outpath.file_name().unwrap());
                            if !real_final.starts_with(&canonical_dest) {
                                return Err(format!(
                                    "解压后路径越界（目录包含符号链接？）: {}",
                                    file.name()
                                ));
                            }
                        }

                        let mut outfile = fs::File::create(&outpath).map_err(|e| e.to_string())?;
                        std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;

                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Some(mode) = file.unix_mode() {
                                let _ =
                                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                            }
                        }
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn find_executable_in_dir(&self, dir: &PathBuf, os: &str) -> Result<PathBuf, String> {
        let expected_name = match os {
            "windows" => "RTLauncher.exe",
            "macos" => "RTLauncher",
            "linux" => "RTLauncher",
            _ => "RTLauncher",
        };

        // portable 构建的可执行文件名是小写 `rtlauncher`（Cargo 包名），必须一起找
        let expected_name_lower = expected_name.to_lowercase();

        let search_paths = vec![
            dir.join(expected_name),
            dir.join(&expected_name_lower),
            dir.join("RTLauncher"),
            dir.join("rtlauncher"),
            dir.join("bin").join(expected_name),
            dir.join("bin").join(&expected_name_lower),
            dir.join("bin").join("RTLauncher"),
            dir.join("bin").join("rtlauncher"),
        ];

        for path in &search_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        self.find_file_recursive(dir, expected_name)
    }

    fn find_file_recursive(&self, dir: &PathBuf, name: &str) -> Result<PathBuf, String> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(found) = self.find_file_recursive(&path, name) {
                        return Ok(found);
                    }
                } else if path
                    .file_name()
                    .map(|f| f == name || f.to_string_lossy().eq_ignore_ascii_case(name))
                    .unwrap_or(false)
                {
                    return Ok(path);
                }
            }
        }
        Err(format!("找不到可执行文件: {}", name))
    }

    fn find_app_bundle_in_dir(&self, dir: &PathBuf) -> Result<PathBuf, String> {
        let search_paths = vec![
            dir.join("RTLauncher.app"),
            dir.join("RTLauncher-macos").join("RTLauncher.app"),
            dir.join("RTLauncher-aarch64").join("RTLauncher.app"),
        ];

        for path in &search_paths {
            if path.exists() && path.is_dir() {
                return Ok(path.clone());
            }
        }

        self.find_app_bundle_recursive(dir)
    }

    fn find_app_bundle_recursive(&self, dir: &PathBuf) -> Result<PathBuf, String> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_dir() {
                    if path.extension().map(|e| e == "app").unwrap_or(false) {
                        return Ok(path);
                    }
                    if let Ok(found) = self.find_app_bundle_recursive(&path) {
                        return Ok(found);
                    }
                }
            }
        }
        Err("找不到 .app 包".to_string())
    }

    fn copy_dir_recursive(&self, src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
        if !dst.exists() {
            fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        }
        for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let target_path = dst.join(entry.file_name());
            if path.is_dir() {
                self.copy_dir_recursive(&path, &target_path)?;
            } else {
                fs::copy(&path, &target_path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub needs_check: bool,
    pub update_available: bool,
    pub current_version: String,
    pub target_version: Option<String>,
    pub message: String,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub success: bool,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
}

fn hex_encode_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn parses_lighting_team_release_body_and_asset_metadata() {
        let json = format!(
            r#"{{
                "tag_name":"1.2.0",
                "name":"Stable 1.2.0",
                "body":{{
                    "update_info":"release notes",
                    "sum":{{"RTLauncher_1.2.0_x64-setup.exe":"{HASH}"}}
                }},
                "prerelease":false,
                "assets":[{{
                    "name":"RTLauncher_1.2.0_x64-setup.exe",
                    "platform":"windows-x64",
                    "size":1234,
                    "browser_download_url":"https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/RTLauncher_1.2.0_x64-setup.exe"
                }}]
            }}"#
        );

        let releases = parse_release_response(&json).expect("Lighting-Team response must parse");

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version.to_string(), "1.2.0");
        assert_eq!(releases[0].notes, "release notes");
        assert_eq!(
            releases[0]
                .checksums
                .get("rtlauncher_1.2.0_x64-setup.exe")
                .map(String::as_str),
            Some(HASH)
        );
        assert_eq!(
            releases[0].assets[0].platform.as_deref(),
            Some("windows-x64")
        );
    }

    #[test]
    fn parses_github_release_array_and_text_body() {
        let json = r#"[{
            "tag_name":"v1.1.9",
            "name":"Release 1.1.9",
            "body":"GitHub release notes",
            "prerelease":false,
            "assets":[{
                "name":"RTLauncher_1.1.9_x64-setup.exe",
                "size":4321,
                "browser_download_url":"https://github.com/cqw-acq/RTLauncher_new/releases/download/v1.1.9/RTLauncher_1.1.9_x64-setup.exe"
            }]
        }]"#;

        let releases = parse_release_response(json).expect("GitHub response must parse");

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version.to_string(), "1.1.9");
        assert_eq!(releases[0].notes, "GitHub release notes");
        assert!(releases[0].checksums.is_empty());
        assert_eq!(releases[0].assets[0].platform, None);
    }

    #[test]
    fn selects_only_the_highest_stable_version_above_current() {
        let json = r#"[
            {"tag_name":"1.1.9","name":"1.1.9","body":"","prerelease":false,"assets":[]},
            {"tag_name":"1.2.0","name":"1.2.0","body":"","prerelease":false,"assets":[]},
            {"tag_name":"1.3.0","name":"1.3.0 beta","body":"","prerelease":true,"assets":[]},
            {"tag_name":"1.4.0-beta.1","name":"1.4.0 beta","body":"","prerelease":false,"assets":[]}
        ]"#;
        let releases = parse_release_response(json).expect("fixture must parse");

        assert_eq!(
            select_latest_release(&releases, "1.1.0").map(|release| release.version.to_string()),
            Some("1.2.0".to_string())
        );
        assert!(select_latest_release(&releases, "1.2.0").is_none());
        assert!(select_latest_release(&releases, "2.0.0").is_none());
    }

    #[test]
    fn prefers_lighting_team_metadata_when_versions_are_equal() {
        let primary = r#"{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":{"update_info":"primary notes","sum":{}},
            "prerelease":false,"assets":[]
        }"#;
        let github = r#"[{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":"GitHub notes","prerelease":false,"assets":[]
        }]"#;
        let mut releases = parse_release_response(primary).expect("primary fixture must parse");
        releases.extend(parse_release_response(github).expect("GitHub fixture must parse"));

        assert_eq!(
            select_latest_release(&releases, "1.1.0").map(|release| release.notes.as_str()),
            Some("primary notes")
        );
    }

    #[test]
    fn available_update_returns_release_notes_as_changelog() {
        let json = r#"{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":{"update_info":"Added startup notifications","sum":{}},
            "prerelease":false,"assets":[]
        }"#;
        let releases = parse_release_response(json).expect("fixture must parse");
        let release =
            select_latest_release(&releases, "1.1.0").expect("fixture must contain an update");

        let result = available_update_result("1.1.0", release);

        assert_eq!(result.target_version.as_deref(), Some("1.2.0"));
        assert_eq!(
            result.changelog.as_deref(),
            Some("Added startup notifications")
        );
        assert_eq!(result.message, "发现新版本可用");
    }

    #[test]
    fn explicit_platform_selects_the_correct_macos_architecture() {
        let json = r#"{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":{"update_info":"","sum":{}},"prerelease":false,
            "assets":[
                {"name":"RTLauncher_1.2.0_aarch64.dmg","platform":"macos-aarch64","size":1,"browser_download_url":"https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/arm.dmg"},
                {"name":"RTLauncher_1.2.0_x64.dmg","platform":"macos-x64","size":1,"browser_download_url":"https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/x64.dmg"}
            ]
        }"#;
        let releases = parse_release_response(json).expect("fixture must parse");

        assert_eq!(
            UpdateFetcher::select_asset(&releases[0], "macos-x86_64")
                .map(|asset| asset.name.as_str()),
            Some("RTLauncher_1.2.0_x64.dmg")
        );
    }

    #[test]
    fn signed_download_url_produces_a_plain_filename() {
        let url = "https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/RTLauncher_1.2.0_x64-setup.exe?sign=abc&t=123";

        assert_eq!(
            filename_from_download_url(url).as_deref(),
            Ok("RTLauncher_1.2.0_x64-setup.exe")
        );
    }

    #[test]
    fn filename_fallback_rejects_the_wrong_architecture() {
        let json = r#"[{
            "tag_name":"1.2.0","name":"1.2.0","body":"","prerelease":false,
            "assets":[{
                "name":"RTLauncher_1.2.0_arm64.dmg","size":1,
                "browser_download_url":"https://github.com/cqw-acq/RTLauncher_new/releases/download/1.2.0/RTLauncher_1.2.0_arm64.dmg"
            }]
        }]"#;
        let releases = parse_release_response(json).expect("fixture must parse");

        assert!(UpdateFetcher::select_asset(&releases[0], "macos-x86_64").is_none());
    }

    #[test]
    fn falls_back_to_github_asset_for_the_same_latest_version() {
        let primary = r#"{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":"primary notes","prerelease":false,"assets":[]
        }"#;
        let github = r#"[{
            "tag_name":"1.2.0","name":"1.2.0",
            "body":"GitHub notes","prerelease":false,
            "assets":[{
                "name":"RTLauncher_1.2.0_x64-setup.exe","size":1,
                "browser_download_url":"https://github.com/cqw-acq/RTLauncher_new/releases/download/1.2.0/RTLauncher_1.2.0_x64-setup.exe"
            }]
        }]"#;
        let mut releases = parse_release_response_for_source(primary, ReleaseSource::LightingTeam)
            .expect("primary fixture must parse");
        releases.extend(
            parse_release_response_for_source(github, ReleaseSource::GitHub)
                .expect("GitHub fixture must parse"),
        );

        let (release, asset) = select_latest_candidate(&releases, "1.1.0", "windows-x86_64")
            .expect("GitHub must supply the usable asset");
        assert_eq!(release.notes, "GitHub notes");
        assert_eq!(asset.name, "RTLauncher_1.2.0_x64-setup.exe");
    }

    #[test]
    fn same_version_candidate_never_uses_prerelease_metadata() {
        let primary = r#"{
            "tag_name":"1.2.0","name":"1.2.0 beta","body":{"update_info":"beta","sum":{}},
            "prerelease":true,
            "assets":[{
                "name":"RTLauncher_1.2.0_x64-setup.exe","platform":"windows-x64","size":1,
                "browser_download_url":"https://7463-tcb-charcaius-d0gpaxdu6e2408df8-1306022435.tcb.qcloud.la/RTL/releases/1.2.0/RTLauncher_1.2.0_x64-setup.exe"
            }]
        }"#;
        let github = r#"[{
            "tag_name":"1.2.0","name":"1.2.0","body":"stable","prerelease":false,
            "assets":[{
                "name":"RTLauncher_1.2.0_x64-setup.exe","size":1,
                "browser_download_url":"https://github.com/cqw-acq/RTLauncher_new/releases/download/1.2.0/RTLauncher_1.2.0_x64-setup.exe"
            }]
        }]"#;
        let mut releases = parse_release_response_for_source(primary, ReleaseSource::LightingTeam)
            .expect("primary fixture must parse");
        releases.extend(
            parse_release_response_for_source(github, ReleaseSource::GitHub)
                .expect("GitHub fixture must parse"),
        );

        let (release, _) = select_latest_candidate(&releases, "1.1.0", "windows-x86_64")
            .expect("stable GitHub release must be selected");
        assert_eq!(release.notes, "stable");
    }

    #[test]
    fn parses_github_sha256_text_block() {
        let json = format!(
            r#"[{{
                "tag_name":"1.2.0","name":"1.2.0",
                "body":"```SHA256SUMS\n{HASH}  RTLauncher_1.2.0_x64-setup.exe\n```",
                "prerelease":false,"assets":[]
            }}]"#
        );
        let releases = parse_release_response(&json).expect("fixture must parse");

        assert_eq!(
            releases[0]
                .checksums
                .get("rtlauncher_1.2.0_x64-setup.exe")
                .map(String::as_str),
            Some(HASH)
        );
    }
}
