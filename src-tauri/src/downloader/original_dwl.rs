use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Semaphore};

const MOJANG_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest.json";
const MIRROR_URL: &str = "https://bmclapi2.bangbang93.com";
const MAX_CONCURRENT_DOWNLOADS: usize = 32;

#[derive(Debug)]
struct DownloadTask {
    urls: Vec<String>,
    target_path: PathBuf,
    sha1: String,
    size: u64,
}

struct DownloadProgress {
    total: Arc<AtomicUsize>,
    done: Arc<AtomicUsize>, // success + failed
}

impl DownloadProgress {
    fn new(total: usize) -> Self {
        Self {
            total: Arc::new(AtomicUsize::new(total)),
            done: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionJson {
    downloads: Downloads,
    #[serde(default)]
    logging: Option<Logging>,
    #[serde(rename = "assetIndex")]
    asset_index: AssetIndex,
    libraries: Vec<Library>,
    #[serde(flatten)]
    other: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetIndex {
    url: String,
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetsJson {
    objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetObject {
    hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Downloads {
    client: ClientDownload,
}

#[derive(Debug, Deserialize, Serialize)]
struct ClientDownload {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Logging {
    client: LoggingClient,
}

#[derive(Debug, Deserialize, Serialize)]
struct LoggingClient {
    file: LogFile,
}

#[derive(Debug, Deserialize, Serialize)]
struct LogFile {
    url: String,
    sha1: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Library {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LibraryDownloads {
    artifact: Option<Artifact>,
    #[serde(default)]
    classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Artifact {
    path: String,
    sha1: String,
    size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Rule {
    action: String,
    #[serde(default)]
    os: Option<OsRule>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OsRule {
    name: Option<String>,
}


async fn download_task(
    task: DownloadTask,
    client: Arc<reqwest::Client>,
    semaphore: Arc<Semaphore>,
    progress: Option<Arc<DownloadProgress>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _permit = semaphore.acquire().await?;
    const MAX_RETRIES: u8 = 5;
    let mut retry_count = 0;
    let mut used_urls: Vec<String> = Vec::new();

    // 检查文件是否已存在且完整
    if let Ok(mut file) = File::open(&task.target_path).await {
        match check_sha1(&mut file, &task.sha1).await {
            Ok(true) => {
                if let Some(p) = &progress {
                    p.done.fetch_add(1, Ordering::SeqCst);
                }
                return Ok(());
            }
            _ => {
                let _ = fs::remove_file(&task.target_path);
            }
        }
    }

    // 确保目录存在
    if let Some(parent) = task.target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // 尝试所有URL（官方源优先，失败后自动回退到镜像源）
    for url in &task.urls {
        if used_urls.contains(url) {
            continue;
        }

        let result = download_with_url(url, &task, &client).await;
        used_urls.push(url.clone());

        match result {
            Ok(_) => {
                let mut file = File::open(&task.target_path).await?;
                if check_sha1(&mut file, &task.sha1).await? {
                    if let Some(p) = &progress {
                        p.done.fetch_add(1, Ordering::SeqCst);
                    }
                    return Ok(());
                } else {
                    let _ = fs::remove_file(&task.target_path);
                }
            }
            Err(e) => {
                eprintln!("下载失败 [{}]: {}", url, e);
            }
        }

        retry_count += 1;
        if retry_count >= MAX_RETRIES {
            break;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // 下载彻底失败也计入 done，保证百分比最终能到 100%
    if let Some(p) = &progress {
        p.done.fetch_add(1, Ordering::SeqCst);
    }

    Err(format!("文件 {} 下载失败，已尝试所有源", task.target_path.display()).into())
}


async fn download_with_url(
    url: &str,
    task: &DownloadTask,
    client: &reqwest::Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = client.get(url)
        .send()
        .await?
        .error_for_status()?;

    let content_length = response.content_length().unwrap_or(0);
    if task.size > 0 && content_length != task.size {
        return Err(format!("文件大小不匹配: 预期{} 实际{}", task.size, content_length).into());
    }

    let mut file = tokio::fs::File::create(&task.target_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }

    file.sync_all().await?;

    Ok(())
}


async fn check_sha1(
    file: &mut File,
    expected: &str
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 8192];
    let mut reader = tokio::io::BufReader::new(file);

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()) == expected)
}

pub async fn process_version(
    version: &str,
    minecraft_path: &Path,
    progress_tx: mpsc::Sender<f64>,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let version_dir = minecraft_path.join("versions").join(version);
    fs::create_dir_all(&version_dir)?;
    fs::create_dir_all(minecraft_path.join("instance").join(version).join("mods"))?;
    let natives_dir = &version_dir.join(format!("{}-natives", version));
    fs::create_dir_all(&natives_dir)?;

    // 获取版本 JSON（优先官方源）
    let json_url = fetch_version_url(version).await?;
    let json_content = reqwest::get(&json_url).await?.text().await?;
    fs::write(version_dir.join(format!("{}.json", version)), &json_content)?;
    let version_data: VersionJson = serde_json::from_str(&json_content)?;

    // 保存 asset index（直接使用 version json 中的官方 URL）
    let assets_content = reqwest::get(&version_data.asset_index.url).await?.text().await?;
    let indexes_dir = minecraft_path.join("assets").join("indexes");
    fs::create_dir_all(&indexes_dir)?;
    let index_path = indexes_dir.join(format!("{}.json", version_data.asset_index.id));
    fs::write(index_path, &assets_content)?;

    let assets_data: AssetsJson = serde_json::from_str(&assets_content)?;

    let mut tasks: Vec<DownloadTask> = Vec::new();

    // 客户端 JAR：官方源优先，bmcl 兜底
    let client_download = &version_data.downloads.client;
    tasks.push(DownloadTask {
        urls: vec![
            client_download.url.clone(),
            format!("{}/version/{}/client", MIRROR_URL, version),
        ],
        target_path: version_dir.join(format!("{}.jar", version)),
        sha1: client_download.sha1.clone(),
        size: client_download.size,
    });

    // 日志配置文件（直接来自官方 JSON，无需镜像）
    if let Some(logging) = &version_data.logging {
        tasks.push(DownloadTask {
            urls: vec![logging.client.file.url.clone()],
            target_path: version_dir.join(&logging.client.file.url
                .split('/')
                .last()
                .unwrap_or("log_config.xml")
                .to_string()),
            sha1: logging.client.file.sha1.clone(),
            size: 0,
        });
    }

    // 依赖库：官方源优先，bmcl 兜底
    let os_type = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    for lib in version_data.libraries {
        // artifact 下载遵循 rules 过滤
        if check_rules(&lib.rules, os_type) {
            if let Some(artifact) = lib.downloads.artifact {
                let official_url = format!("https://libraries.minecraft.net/{}", artifact.path);
                let mirror_url = format!("https://bmclapi2.bangbang93.com/maven/{}", artifact.path);
                tasks.push(DownloadTask {
                    urls: vec![official_url, mirror_url],
                    target_path: minecraft_path.join("libraries").join(&artifact.path),
                    sha1: artifact.sha1,
                    size: artifact.size,
                });
            }
        }

        // classifiers（natives）下载：不受 rules 约束，直接通过 path 中的系统/架构信息判断
        // 这样可以正确处理 rules 中 osx-only 但实际 classifiers 含 natives-windows 的库
        for native in lib.downloads.classifiers.values() {
            if should_download_native(&native.path) {
                let official_url = format!("https://libraries.minecraft.net/{}", native.path);
                let mirror_url = format!("https://bmclapi2.bangbang93.com/maven/{}", native.path);
                tasks.push(DownloadTask {
                    urls: vec![official_url, mirror_url],
                    target_path: minecraft_path.join("libraries").join(&native.path),
                    sha1: native.sha1.clone(),
                    size: native.size,
                });
            }
        }
    }

    // 资源文件：官方源优先，bmcl 兜底
    for (_, obj) in assets_data.objects {
        let prefix = &obj.hash[0..2];
        let official_url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            prefix, obj.hash
        );
        let mirror_url = format!("{}/assets/{}/{}", MIRROR_URL, prefix, obj.hash);

        tasks.push(DownloadTask {
            urls: vec![official_url, mirror_url],
            target_path: minecraft_path
                .join("assets")
                .join("objects")
                .join(prefix)
                .join(&obj.hash),
            sha1: obj.hash,
            size: 0,
        });
    }

    let total = tasks.len();
    let progress = Arc::new(DownloadProgress::new(total));
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    // 整个下载过程复用同一个 client，避免每个任务新建 client 导致收尾时大量析构
    let client = Arc::new(
        reqwest::Client::builder()
            .pool_max_idle_per_host(MAX_CONCURRENT_DOWNLOADS)
            .timeout(Duration::from_secs(100))
            .build()?
    );

    // 启动定时进度推送任务：每隔 1 秒计算并发送百分比
    let progress_reporter = {
        let done_counter = progress.done.clone();
        let tx = progress_tx.clone();
        tokio::spawn(async move {
            loop {
                let done = done_counter.load(Ordering::SeqCst);
                let percent = if total > 0 {
                    (done as f64 / total as f64) * 100.0
                } else {
                    100.0
                };
                if tx.send(percent).await.is_err() {
                    break;
                }
                if done >= total {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    };

    let mut futures = Vec::new();
    for task in tasks {
        let semaphore = semaphore.clone();
        let progress = progress.clone();
        let client = client.clone();
        futures.push(download_task(task, client, semaphore, Some(progress)));
    }

    let results = stream::iter(futures)
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .collect::<Vec<_>>()
        .await;

    // 下载全部完成后立即中止 reporter（不等它睡完1秒），再发送最终 100%
    progress_reporter.abort();
    let _ = progress_tx.send(100.0).await;
    drop(progress_tx);

    let mut errors = Vec::new();
    for result in results {
        if let Err(e) = result {
            errors.push(e.to_string());
        }
    }

    let done = progress.done.load(Ordering::SeqCst);
    println!("下载完成: {}/{} ({:.1}%)", done, total, (done as f64 / total as f64) * 100.0);

    if !errors.is_empty() {
        return Err(errors.join("\n").into());
    }

    Ok(vec![])
}

fn check_rules(rules: &[Rule], os_type: &str) -> bool {
    let mut allowed = true;
    for rule in rules {
        let os_match = rule.os.as_ref().map_or(true, |os|
            os.name.as_deref() == Some(os_type)
        );
        match rule.action.as_str() {
            "allow" => allowed = os_match,
            "disallow" => allowed = !os_match,
            _ => ()
        }
    }
    allowed
}

/// 获取当前系统对应的 natives path 标签列表（借鉴 decompression.rs）
fn get_system_tags() -> Vec<&'static str> {
    match env::consts::OS {
        "windows" => vec!["-windows"],
        "linux"   => vec!["-linux"],
        "macos"   => vec!["-macos", "-osx"],
        _         => vec![],
    }
}

/// 检查 path 中 -natives 后的部分是否属于当前系统（借鉴 decompression.rs）
fn match_native_system(path: &str) -> bool {
    let system_tags = get_system_tags();
    if system_tags.is_empty() {
        return false;
    }
    let parts: Vec<&str> = path.split("-natives").collect();
    if parts.len() < 2 {
        return false;
    }
    let post_native = parts[1];
    system_tags.iter().any(|&tag| post_native.starts_with(tag))
}

/// 从 path 末尾提取架构段（借鉴 decompression.rs extract_arch_info）
/// 返回 (arch_opt, is_implicit)：is_implicit=true 表示无架构后缀（即通用/x86_64）
fn extract_arch_info(path: &str) -> (Option<String>, bool) {
    const OS_NAMES: [&str; 4] = ["windows", "linux", "macos", "osx"];
    let segments: Vec<&str> = path.split('-').collect();
    let (arch_part, has_arch) = segments
        .iter()
        .rev()
        .find(|s| !s.is_empty())
        .and_then(|s| s.split('.').next())
        .map(|s| {
            let is_os = OS_NAMES.contains(&s);
            (s, !is_os)
        })
        .unwrap_or(("", false));
    (
        if has_arch { Some(arch_part.to_lowercase()) } else { None },
        !has_arch,
    )
}

/// 综合判断某个 classifier artifact 的 path 是否应在当前平台下载
fn should_download_native(path: &str) -> bool {
    if !path.contains("-natives") {
        return false;
    }

    let is_windows = env::consts::OS == "windows";
    // 特殊处理 natives-windows-64（某些旧版库会用这个 key）
    let is_win64_case = is_windows && path.contains("natives-windows-64");

    if !match_native_system(path) && !is_win64_case {
        return false;
    }

    let current_arch = env::consts::ARCH;
    let (arch_opt, is_implicit) = extract_arch_info(path);

    is_win64_case || match (arch_opt.as_deref(), is_implicit) {
        (_, true) => current_arch == "x86_64",
        (Some(arch), false) => match current_arch {
            "x86_64"  => arch == "x86_64",
            "x86"     => arch == "x86",
            "aarch64" => arch == "arm64" || arch == "aarch_64",
            _         => false,
        },
        _ => false,
    }
}

async fn fetch_version_url(version: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = match client.get(MOJANG_MANIFEST).send().await {
        Ok(res) if res.status().is_success() => res,
        _ => {
            // 官方 manifest 不可达时才回退到镜像
            return Ok(format!("{}/mc/game/version_manifest.json", MIRROR_URL));
        }
    };

    let manifest = response.json::<VersionManifest>().await
        .map_err(|_| "Failed to parse version manifest")?;

    manifest.versions
        .iter()
        .find(|v| v.id == version)
        .map(|e| e.url.clone())
        .or_else(|| manifest.versions.iter()
            .find(|v| v.id.replace(".", "") == version.replace(".", ""))
            .map(|e| e.url.clone()))
        .map(Ok)
        .unwrap_or_else(|| Ok(format!("{}/version/{}/json", MIRROR_URL, version)))
}
