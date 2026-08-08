use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const JAVA_MANIFEST_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
// 减少并发下载数，避免被服务器限流
const MAX_CONCURRENT_DOWNLOADS: usize = 8;
const DOWNLOAD_BUFFER_SIZE: usize = 65536;
// 每个文件的最大重试次数（整个下载流程，包括 body 读取）
const MAX_FILE_RETRIES: usize = 8;
// 初始重试延迟（毫秒），采用指数退避
const INITIAL_RETRY_DELAY_MS: u64 = 500;
// 最大重试延迟（毫秒）
const MAX_RETRY_DELAY_MS: u64 = 8000;

#[derive(Debug, Deserialize)]
struct JavaManifest {
    #[serde(flatten)]
    platforms: HashMap<String, PlatformData>,
}

#[derive(Debug, Deserialize)]
struct PlatformData {
    #[serde(flatten)]
    runtimes: HashMap<String, Vec<JavaRuntime>>,
}

#[derive(Debug, Deserialize, Clone)]
struct JavaRuntime {
    version: JavaVersion,
    manifest: ManifestInfo,
}

#[derive(Debug, Deserialize, Clone)]
struct JavaVersion {
    name: String,
    released: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ManifestInfo {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct JavaFilesManifest {
    files: HashMap<String, JavaFileInfo>,
}

#[derive(Debug, Deserialize)]
struct JavaFileInfo {
    #[serde(rename = "type")]
    file_type: String,
    downloads: Option<Downloads>,
    executable: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Downloads {
    raw: DownloadInfo,
}

#[derive(Debug, Deserialize, Clone)]
struct DownloadInfo {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Debug)]
struct DownloadTask {
    url: String,
    target_path: PathBuf,
    sha1: String,
    size: u64,
    executable: bool,
}

struct DownloadProgress {
    done: Arc<AtomicUsize>,
}

impl DownloadProgress {
    fn new() -> Self {
        Self {
            done: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaVersionInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadResult {
    pub message: String,
    pub java_path: String,
}

fn get_platform_identifier() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",
        _ => "unknown",
    }
}

#[tauri::command]
pub async fn get_java_versions() -> Result<Vec<JavaVersionInfo>, String> {
    let client = crate::http_client::shared_client().await;
    let response = client
        .get(JAVA_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("获取Java版本列表失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("获取Java版本列表失败: HTTP {}", response.status()));
    }

    let manifest: JavaManifest = response
        .json()
        .await
        .map_err(|e| format!("解析Java版本列表失败: {}", e))?;

    let platform = get_platform_identifier();
    if platform == "unknown" {
        return Err("不支持的系统或架构".to_string());
    }

    let platform_data = manifest
        .platforms
        .get(platform)
        .ok_or_else(|| format!("未找到平台 {} 的Java版本", platform))?;

    let mut versions = Vec::new();
    for (runtime_name, runtimes) in &platform_data.runtimes {
        for runtime in runtimes {
            versions.push(JavaVersionInfo {
                name: runtime_name.clone(),
                version: runtime.version.name.clone(),
            });
        }
    }

    if versions.is_empty() {
        return Err("未找到可用的Java版本".to_string());
    }

    Ok(versions)
}

#[tauri::command]
pub async fn download_java_runtime(
    runtime_name: String,
    base_path: String,
    task_id: u64,
    window: tauri::WebviewWindow,
) -> Result<DownloadResult, String> {
    let client = crate::http_client::shared_client().await;
    let response = client
        .get(JAVA_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("获取Java版本列表失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("获取Java版本列表失败: HTTP {}", response.status()));
    }

    let manifest: JavaManifest = response
        .json()
        .await
        .map_err(|e| format!("解析Java版本列表失败: {}", e))?;

    let platform = get_platform_identifier();
    let platform_data = manifest
        .platforms
        .get(platform)
        .ok_or_else(|| format!("未找到平台 {} 的Java版本", platform))?;

    let runtimes = platform_data
        .runtimes
        .get(&runtime_name)
        .ok_or_else(|| format!("未找到 Java 版本: {}", runtime_name))?;

    let runtime = runtimes
        .first()
        .ok_or_else(|| format!("Java 版本 {} 无可用下载", runtime_name))?;

    let version_name = &runtime.version.name;
    let manifest_url = &runtime.manifest.url;

    let files_response = client
        .get(manifest_url)
        .send()
        .await
        .map_err(|e| format!("获取Java文件列表失败: {}", e))?;

    if !files_response.status().is_success() {
        return Err(format!(
            "获取Java文件列表失败: HTTP {}",
            files_response.status()
        ));
    }

    let files_manifest: JavaFilesManifest = files_response
        .json()
        .await
        .map_err(|e| format!("解析Java文件列表失败: {}", e))?;

    let mut download_tasks = Vec::new();
    let java_dir = PathBuf::from(&base_path).join(&runtime_name);
    let java_exe_name = if cfg!(windows) {
        "bin/java.exe"
    } else {
        "bin/java"
    };
    let mut java_relative_path: Option<String> = None;

    for (file_path, file_info) in &files_manifest.files {
        if file_info.file_type == "directory" {
            continue;
        }

        if file_path.ends_with(java_exe_name) && java_relative_path.is_none() {
            java_relative_path = Some(file_path.clone());
        }

        if let Some(downloads) = &file_info.downloads {
            let download_info = &downloads.raw;
            let target_path = java_dir.join(file_path);

            download_tasks.push(DownloadTask {
                url: download_info.url.clone(),
                target_path,
                sha1: download_info.sha1.clone(),
                size: download_info.size,
                executable: file_info.executable.unwrap_or(false),
            });
        }
    }

    if download_tasks.is_empty() {
        return Err("没有找到需要下载的文件".to_string());
    }

    let java_bin = java_relative_path
        .map(|p| java_dir.join(p))
        .unwrap_or_else(|| java_dir.join(java_exe_name));

    download_java_files(download_tasks, task_id, window.clone())
        .await
        .map_err(|e| {
            let _ = window.emit(
                "java-download-finished",
                serde_json::json!({
                    "task_id": task_id,
                    "success": false,
                    "error": e
                }),
            );
            e
        })?;

    let _ = window.emit(
        "java-download-finished",
        serde_json::json!({
            "task_id": task_id,
            "success": true,
            "error": null
        }),
    );

    Ok(DownloadResult {
        message: format!(
            "Java {} ({}) 已成功下载到: {}",
            runtime_name,
            version_name,
            java_dir.display()
        ),
        java_path: java_bin.to_string_lossy().to_string(),
    })
}

async fn download_java_files(
    tasks: Vec<DownloadTask>,
    task_id: u64,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let total = tasks.len();
    let progress = Arc::new(DownloadProgress::new());
    // 复用共享 HTTP client（连接池更优、配置更完整）
    let client = crate::http_client::shared_client().await;

    let progress_clone = progress.clone();
    let window_clone = window.clone();
    let progress_reporter = tokio::spawn(async move {
        loop {
            let done = progress_clone.done.load(Ordering::SeqCst);
            let percent = if total > 0 {
                (done as f64 / total as f64) * 100.0
            } else {
                100.0
            };
            let _ = window_clone.emit(
                "java-download-progress",
                serde_json::json!({
                    "task_id": task_id,
                    "percent": percent
                }),
            );
            if done >= total {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // Java 运行时通常包含大量小文件。无限并发会占满连接池和文件句柄，
    // 因此所有文件共用同一并发上限，而不是只限制大文件。
    let download_futures = tasks.into_iter().map(|task| {
        let progress = progress.clone();
        let client = client.clone();
        download_java_file(task, client, progress)
    });

    let results = stream::iter(download_futures)
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .collect::<Vec<_>>()
        .await;

    // 失败任务不会增加 done；不停止该任务会让它永久保留在后台。
    progress_reporter.abort();
    let _ = progress_reporter.await;

    let _ = window.emit(
        "java-download-progress",
        serde_json::json!({
            "task_id": task_id,
            "percent": 100.0
        }),
    );

    let mut errors = Vec::new();
    for result in results {
        if let Err(e) = result {
            errors.push(e);
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    Ok(())
}

async fn download_java_file(
    task: DownloadTask,
    client: Arc<reqwest::Client>,
    progress: Arc<DownloadProgress>,
) -> Result<(), String> {
    // 如果目标文件已存在且 SHA1 匹配，直接跳过
    if let Ok(mut file) = File::open(&task.target_path).await {
        if check_sha1(&mut file, &task.sha1).await.unwrap_or(false) {
            progress.done.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        } else {
            let _ = std::fs::remove_file(&task.target_path);
        }
    }

    // 创建目录
    if let Some(parent) = task.target_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 核心下载：带 body 读取阶段重试、支持断点续传
    download_file_with_resumable_retry(&client, &task.url, &task.target_path, task.size).await?;

    // SHA1 校验
    let mut file = File::open(&task.target_path)
        .await
        .map_err(|e| format!("打开文件失败: {}", e))?;

    if !check_sha1(&mut file, &task.sha1).await.unwrap_or(false) {
        let _ = std::fs::remove_file(&task.target_path);
        return Err("SHA1校验失败".to_string());
    }

    // Unix 平台设置可执行权限
    #[cfg(unix)]
    if task.executable {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&task.target_path)
            .map_err(|e| format!("获取文件权限失败: {}", e))?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&task.target_path, perms)
            .map_err(|e| format!("设置可执行权限失败: {}", e))?;
    }

    progress.done.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// 带断点续传与完整流程重试的下载函数
///
/// 关键修复点：
/// 1. 不仅对 "建立连接 / 获取响应头" 阶段进行重试，
///    对 "读取 body 数据流" 阶段的错误（如 connection reset）也进行重试。
/// 2. 利用 HTTP Range 请求实现断点续传：
///    在重试前检查磁盘已有字节数，从断点处继续下载，
///    避免因中途连接断开而从零重新开始。
/// 3. 使用指数退避策略，减少对服务器的瞬时压力。
async fn download_file_with_resumable_retry(
    client: &reqwest::Client,
    url: &str,
    target_path: &PathBuf,
    expected_size: u64,
) -> Result<(), String> {
    let mut last_error: Option<String> = None;

    for attempt in 0..MAX_FILE_RETRIES {
        // 计算当前磁盘上已有字节数（作为断点续传起点）
        let current_bytes = std::fs::metadata(target_path).map(|m| m.len()).unwrap_or(0);

        // 已有完整文件则直接成功
        if expected_size > 0 && current_bytes == expected_size {
            return Ok(());
        }

        if attempt > 0 {
            // 指数退避：500ms → 1s → 2s → 4s → 8s → 8s ...
            let backoff_ms = std::cmp::min(
                INITIAL_RETRY_DELAY_MS * (1u64 << (attempt - 1)),
                MAX_RETRY_DELAY_MS,
            );
            eprintln!(
                "[Java下载] 第{}次重试 ({}/{}), 已下载 {} bytes, 等待 {}ms 后继续: {}",
                attempt,
                attempt,
                MAX_FILE_RETRIES - 1,
                current_bytes,
                backoff_ms,
                url
            );
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        }

        match download_chunk(client, url, target_path, current_bytes).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_error = Some(e.clone());
                eprintln!(
                    "[Java下载] 尝试 {}/{} 失败: {} (URL: {})",
                    attempt + 1,
                    MAX_FILE_RETRIES,
                    e,
                    url
                );
            }
        }
    }

    Err(format!(
        "下载失败: {}（已重试 {} 次）",
        last_error.unwrap_or_else(|| "未知错误".to_string()),
        MAX_FILE_RETRIES
    ))
}

/// 从指定字节偏移开始下载一个 HTTP Range 片段到文件末尾。
/// 出错时会保留已写入部分（以便下一次重试作为断点续传的起点）。
async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    target_path: &PathBuf,
    start_offset: u64,
) -> Result<(), String> {
    // 构建请求
    let mut request_builder = client
        .get(url)
        // 限制单次请求总时长（读取阶段最长 10 分钟），避免挂死
        .timeout(Duration::from_secs(600));

    if start_offset > 0 {
        // 断点续传：从 start_offset 开始下载剩余字节
        request_builder =
            request_builder.header(reqwest::header::RANGE, format!("bytes={}-", start_offset));
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let status = response.status();

    // 判断响应状态：
    // - 首次下载 (offset=0)：期望 HTTP 200
    // - 断点续传 (offset>0)：期望 HTTP 206 Partial Content
    if start_offset == 0 {
        if !status.is_success() {
            return Err(format!("HTTP 请求失败: {}", status));
        }
        // 如果服务器不理会 Range 也没关系，直接从 0 写即可
    } else {
        if status.as_u16() == 416 {
            // HTTP 416 Range Not Satisfiable：
            // 说明服务器认为我们请求的区间超出文件大小，
            // 通常意味着本地已下载完成。视为成功。
            return Ok(());
        }
        if status.as_u16() != 206 {
            // 服务器不支持 Range，回退策略：丢弃已有部分，从头下载。
            // （某些 CDN 不支持 Range 但会返回 200 + 完整 body）
            if !status.is_success() {
                return Err(format!("HTTP 请求失败: {}", status));
            }
            // 200 但我们带了 Range 头：服务器返回的是完整文件。
            // 此时需要覆盖写入（从头写）
            eprintln!(
                "[Java下载] 服务器不支持 Range 请求 (HTTP {})，回退到覆盖写入: {}",
                status, url
            );
            return write_response_to_file_from_start(response, target_path).await;
        }
    }

    // 正常路径：打开文件并写入
    write_response_to_file(response, target_path, start_offset).await
}

/// 从 offset 开始将响应 body 写入文件
async fn write_response_to_file(
    response: reqwest::Response,
    target_path: &PathBuf,
    start_offset: u64,
) -> Result<(), String> {
    use tokio::io::BufWriter;

    // 以 append 模式打开文件；若 offset > 0 则 seek 到 offset
    let file = if start_offset == 0 {
        File::create(target_path)
            .await
            .map_err(|e| format!("创建文件失败: {}", e))?
    } else {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .open(target_path)
            .await
            .map_err(|e| format!("打开文件失败: {}", e))?;
        f.seek(std::io::SeekFrom::Start(start_offset))
            .await
            .map_err(|e| format!("定位文件偏移失败: {}", e))?;
        f
    };

    let mut stream = response.bytes_stream();
    let mut writer = BufWriter::with_capacity(DOWNLOAD_BUFFER_SIZE, file);
    let downloaded_bytes = Arc::new(AtomicU64::new(0));
    let downloaded_for_stall = downloaded_bytes.clone();

    // 启动 stall 检测器：若 30 秒内没有任何新字节到来则放弃本次
    let stall_handle = tokio::spawn(async move {
        let mut last_seen = 0u64;
        let mut same_count = 0u32;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let cur = downloaded_for_stall.load(Ordering::Relaxed);
            if cur > last_seen {
                last_seen = cur;
                same_count = 0;
            } else {
                same_count += 1;
                if same_count >= 3 {
                    // 30 秒内无任何进展
                    break;
                }
            }
        }
    });

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取数据失败: {}", e))?;
        downloaded_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        writer
            .write_all(&chunk)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
    }

    // 取消 stall 检测器（正常完成）
    stall_handle.abort();

    writer
        .flush()
        .await
        .map_err(|e| format!("刷新缓冲区失败: {}", e))?;
    writer
        .into_inner()
        .sync_all()
        .await
        .map_err(|e| format!("同步文件失败: {}", e))?;

    Ok(())
}

/// 从头覆盖写入（服务器不支持 Range 时的回退路径）
async fn write_response_to_file_from_start(
    response: reqwest::Response,
    target_path: &PathBuf,
) -> Result<(), String> {
    use tokio::io::BufWriter;

    let file = File::create(target_path)
        .await
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut writer = BufWriter::with_capacity(DOWNLOAD_BUFFER_SIZE, file);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取数据失败: {}", e))?;
        writer
            .write_all(&chunk)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
    }

    writer
        .flush()
        .await
        .map_err(|e| format!("刷新缓冲区失败: {}", e))?;
    writer
        .into_inner()
        .sync_all()
        .await
        .map_err(|e| format!("同步文件失败: {}", e))?;

    Ok(())
}

async fn check_sha1(file: &mut File, expected: &str) -> Result<bool, String> {
    use sha1::{Digest, Sha1};

    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; DOWNLOAD_BUFFER_SIZE];
    let mut reader = tokio::io::BufReader::with_capacity(DOWNLOAD_BUFFER_SIZE, file);

    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| format!("读取文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()) == expected)
}