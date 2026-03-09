use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use futures::stream::{self, StreamExt};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use std::time::Duration;
use tauri::Emitter;

const JAVA_MANIFEST_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const MAX_CONCURRENT_DOWNLOADS: usize = 64;
const DOWNLOAD_BUFFER_SIZE: usize = 65536;

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
    total: Arc<AtomicUsize>,
    done: Arc<AtomicUsize>,
}

impl DownloadProgress {
    fn new(total: usize) -> Self {
        Self {
            total: Arc::new(AtomicUsize::new(total)),
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
    let client = reqwest::Client::new();
    let response = client.get(JAVA_MANIFEST_URL).send().await
        .map_err(|e| format!("获取Java版本列表失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("获取Java版本列表失败: HTTP {}", response.status()));
    }

    let manifest: JavaManifest = response.json().await
        .map_err(|e| format!("解析Java版本列表失败: {}", e))?;

    let platform = get_platform_identifier();
    if platform == "unknown" {
        return Err("不支持的系统或架构".to_string());
    }

    let platform_data = manifest.platforms.get(platform)
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
    target_version: i32,
    base_path: String,
    window: tauri::Window,
) -> Result<DownloadResult, String> {
    let client = reqwest::Client::new();
    let response = client.get(JAVA_MANIFEST_URL).send().await
        .map_err(|e| format!("获取Java版本列表失败: {}", e))?;

    let manifest: JavaManifest = response.json().await
        .map_err(|e| format!("解析Java版本列表失败: {}", e))?;

    let platform = get_platform_identifier();
    let platform_data = manifest.platforms.get(platform)
        .ok_or_else(|| format!("未找到平台 {} 的Java版本", platform))?;

    let mut candidates: Vec<(String, String, String, i32, String)> = Vec::new();

    for (runtime_name, runtimes) in &platform_data.runtimes {
        for runtime in runtimes {
            let version_num: i32 = runtime.version.name
                .split(|c: char| !c.is_numeric())
                .filter(|s| !s.is_empty())
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            candidates.push((
                runtime_name.clone(),
                runtime.version.name.clone(),
                runtime.manifest.url.clone(),
                version_num,
                runtime.version.released.clone(),
            ));
        }
    }

    if candidates.is_empty() {
        return Err("未找到可用的Java版本".to_string());
    }

    candidates.sort_by(|a, b| {
        let diff_a = (a.3 - target_version).abs();
        let diff_b = (b.3 - target_version).abs();
        match diff_a.cmp(&diff_b) {
            std::cmp::Ordering::Equal => b.4.cmp(&a.4),
            other => other,
        }
    });

    let (runtime_name, version_name, manifest_url, _, _) = &candidates[0];

    let files_response = client.get(manifest_url).send().await
        .map_err(|e| format!("获取Java文件列表失败: {}", e))?;

    if !files_response.status().is_success() {
        return Err(format!("获取Java文件列表失败: HTTP {}", files_response.status()));
    }

    let files_manifest: JavaFilesManifest = files_response.json().await
        .map_err(|e| format!("解析Java文件列表失败: {}", e))?;

    let mut download_tasks = Vec::new();
    let java_dir = PathBuf::from(&base_path).join(runtime_name);

    // 从 manifest 中找 java 可执行文件的相对路径
    let java_exe_name = if cfg!(windows) { "bin/java.exe" } else { "bin/java" };
    let mut java_relative_path: Option<String> = None;

    for (file_path, file_info) in &files_manifest.files {
        if file_info.file_type == "directory" {
            continue;
        }

        // 记录 java 可执行文件的相对路径（以 bin/java 或 bin/java.exe 结尾）
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

    download_java_files(download_tasks, window.clone()).await
        .map_err(|e| format!("下载失败: {}", e))?;

    Ok(DownloadResult {
        message: format!("Java {} ({}) 已成功下载到: {}",
                         runtime_name, version_name, java_dir.display()),
        java_path: java_bin.to_string_lossy().to_string(),
    })
}

async fn download_java_files(
    tasks: Vec<DownloadTask>,
    window: tauri::Window,
) -> Result<(), String> {
    let total = tasks.len();
    let progress = Arc::new(DownloadProgress::new(total));
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    let client = Arc::new(
        reqwest::Client::builder()
            .pool_max_idle_per_host(MAX_CONCURRENT_DOWNLOADS)
            .pool_idle_timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(120))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .build()
            .map_err(|e| format!("创建HTTP客户端失败: {}", e))?
    );

    let progress_clone = progress.clone();
    let window_clone = window.clone();
    tokio::spawn(async move {
        loop {
            let done = progress_clone.done.load(Ordering::SeqCst);
            let percent = if total > 0 {
                (done as f64 / total as f64) * 100.0
            } else {
                100.0
            };
            let _ = window_clone.emit("java-download-progress", percent);
            if done >= total {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let (large_tasks, small_tasks): (Vec<_>, Vec<_>) = tasks.into_iter()
        .partition(|task| task.size > 5 * 1024 * 1024);

    let small_futures: Vec<_> = small_tasks.into_iter().map(|task| {
        let progress = progress.clone();
        let client = client.clone();
        download_java_file(task, client, None, progress)
    }).collect();

    let small_results = stream::iter(small_futures)
        .buffer_unordered(usize::MAX)
        .collect::<Vec<_>>()
        .await;

    let mut large_futures = Vec::new();
    for task in large_tasks {
        let semaphore = Some(semaphore.clone());
        let progress = progress.clone();
        let client = client.clone();
        large_futures.push(download_java_file(task, client, semaphore, progress));
    }

    let large_results = stream::iter(large_futures)
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .collect::<Vec<_>>()
        .await;

    let results = small_results.into_iter().chain(large_results.into_iter()).collect::<Vec<_>>();

    let _ = window.emit("java-download-progress", 100.0);

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
    semaphore: Option<Arc<Semaphore>>,
    progress: Arc<DownloadProgress>,
) -> Result<(), String> {
    let _permit = if let Some(ref sem) = semaphore {
        Some(sem.acquire().await.map_err(|e| format!("获取信号量失败: {}", e))?)
    } else {
        None
    };

    if let Ok(mut file) = File::open(&task.target_path).await {
        if check_sha1(&mut file, &task.sha1).await.unwrap_or(false) {
            progress.done.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        } else {
            let _ = std::fs::remove_file(&task.target_path);
        }
    }

    if let Some(parent) = task.target_path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let response = client.get(&task.url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("HTTP错误: {}", e))?;

    let file = File::create(&task.target_path).await
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    use tokio::io::BufWriter;
    let mut writer = BufWriter::with_capacity(DOWNLOAD_BUFFER_SIZE, file);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取数据失败: {}", e))?;
        writer.write_all(&chunk).await
            .map_err(|e| format!("写入文件失败: {}", e))?;
    }

    writer.flush().await.map_err(|e| format!("刷新缓冲区失败: {}", e))?;
    writer.into_inner().sync_all().await
        .map_err(|e| format!("同步文件失败: {}", e))?;

    let mut file = File::open(&task.target_path).await
        .map_err(|e| format!("打开文件失败: {}", e))?;

    if !check_sha1(&mut file, &task.sha1).await.unwrap_or(false) {
        let _ = std::fs::remove_file(&task.target_path);
        return Err("SHA1校验失败".to_string());
    }

    // 设置可执行权限（macOS / Linux）
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

async fn check_sha1(file: &mut File, expected: &str) -> Result<bool, String> {
    use sha1::{Digest, Sha1};

    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; DOWNLOAD_BUFFER_SIZE];
    let mut reader = tokio::io::BufReader::with_capacity(DOWNLOAD_BUFFER_SIZE, file);

    loop {
        let n = reader.read(&mut buf).await
            .map_err(|e| format!("读取文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()) == expected)
}
