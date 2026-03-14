use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use super::common_libraries::LibraryItem;

#[derive(Debug, Deserialize)]
struct GameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct LoaderVersion {
    separator: String,
    build: u64,
    maven: String,
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricMetaResponse {
    game: Vec<GameVersion>,
    loader: Vec<LoaderVersion>,
}

/// 获取 Fabric Loader 版本列表
pub fn get_fabric_loader_versions(mc_version: &str, use_mirror: bool) -> Result<Vec<String>> {
    let url = if use_mirror {
        "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions"
    } else {
        "https://meta.fabricmc.net/v2/versions"
    };
    let client = Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let meta: FabricMetaResponse = resp.json()?;
    if !meta.game.iter().any(|g| g.version == mc_version) {
        return Err(anyhow!("MC版本 {} 不存在", mc_version));
    }
    let loader_versions = meta.loader.into_iter().map(|l| l.version).collect();
    Ok(loader_versions)
}

/// 获取 Fabric API 版本列表
pub fn get_fabric_api_versions(mc_version: &str) -> Result<Vec<String>> {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    use quick_xml::name::QName;

    let url = "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/maven-metadata.xml";
    let client = Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let xml_text = resp.text()?;
    let mut reader = Reader::from_str(&xml_text);
    let mut versions = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name() == QName(b"version") => {
                let text = reader.read_text(QName(b"version"))?;
                if text.contains(mc_version) {
                    versions.push(text.into_owned());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(e) => return Err(anyhow!("XML解析错误: {}", e)),
        }
    }
    Ok(versions)
}

/// 安装 Fabric Loader
pub fn install_fabric_loader(mc_version: &str, loader_version: &str, mc_folder_path: &str) -> Result<()> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        mc_version, loader_version
    );
    let client = Client::new();
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let profile_json_text = resp.text()?;

    let version_id = format!("{}-{}-fabric", mc_version, loader_version);
    let versions_dir = PathBuf::from(mc_folder_path).join("versions").join(&version_id);
    fs::create_dir_all(&versions_dir)?;
    let profile_json_path = versions_dir.join(format!("{}.json", version_id));
    fs::write(&profile_json_path, &profile_json_text)?;

    #[derive(Debug, Deserialize)]
    struct ProfileJson {
        #[serde(default)]
        libraries: Vec<LibraryItem>,
    }
    let profile: ProfileJson = serde_json::from_str(&profile_json_text)?;

    // 使用线程池并发下载库文件
    download_libraries_concurrent(
        &profile.libraries,
        "https://maven.fabricmc.net",
        false,
        mc_folder_path
    )?;

    println!("Fabric Loader安装完成！版本ID：{}", version_id);
    Ok(())
}

/// 安装 Fabric API
pub fn install_fabric_api(mc_version: &str, fabric_api_version: &str, mc_folder_path: &str) -> Result<()> {
    let fabric_api_url = format!(
        "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/{0}/fabric-api-{0}.jar",
        fabric_api_version
    );
    let mods_dir = PathBuf::from(mc_folder_path).join("instance").join(mc_version).join("mods");
    fs::create_dir_all(&mods_dir)?;
    let fabric_api_jar = mods_dir.join(format!("fabric-api-{}.jar", fabric_api_version));

    // 使用多线程下载API文件
    download_file_with_progress(&fabric_api_url, &fabric_api_jar, 24)?;

    println!("Fabric API安装完成！版本：{}", fabric_api_version);
    Ok(())
}

/// 使用多线程下载文件并显示进度
fn download_file_with_progress(url: &str, path: &Path, max_threads: usize) -> Result<()> {
    use super::downloader_wrapper::download_file_blocking;

    // 获取文件大小
    let client = Client::new();
    let head_resp = client.head(url).send()?;
    let file_size = head_resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    // 如果文件大于1MB，使用多线程下载
    let threads = if file_size > 1024 * 1024 {
        max_threads.min(file_size as usize)
    } else {
        1
    };

    println!("正在下载: {}", path.file_name().unwrap_or_default().to_string_lossy());
    if file_size > 0 {
        let size_mb = file_size as f64 / (1024.0 * 1024.0);
        println!("文件大小: {:.2} MB", size_mb);
        println!("使用 {} 个线程下载", threads);
    }

    download_file_blocking(url, path, threads)?;

    Ok(())
}

/// 使用线程池并发下载多个库文件
fn download_libraries_concurrent(
    libraries: &[LibraryItem],
    default_url: &str,
    _check_md5: bool,
    mc_folder_path: &str,
) -> Result<()> {
    use super::common_libraries::{parse_library_path_for_fs, parse_library_path_for_url};
    use super::downloader_wrapper::download_file_blocking;
    use std::sync::{Arc, Mutex};

    const MAX_CONCURRENT_DOWNLOADS: usize = 8;
    const MULTITHREAD_THRESHOLD: u64 = 1024 * 1024; // 1MB

    // 准备下载任务
    let mut download_tasks = Vec::new();
    for lib in libraries {
        let base_url = lib.url.as_deref().unwrap_or(default_url).trim_end_matches('/');
        let name = lib.name.clone();
        let (sub_path, jar_name) = parse_library_path_for_fs(&name);
        let library_dir = PathBuf::from(mc_folder_path).join("libraries").join(&sub_path);
        fs::create_dir_all(&library_dir)?;
        let local_file_path = library_dir.join(&jar_name);
        let url_sub_path = parse_library_path_for_url(&name);
        let download_url = format!("{}/{}", base_url, url_sub_path);

        download_tasks.push((download_url, local_file_path, name));
    }

    let total_files = download_tasks.len();
    if total_files == 0 {
        return Ok(());
    }

    println!("准备下载 {} 个库文件...", total_files);

    // 创建进度跟踪器
    let completed = Arc::new(Mutex::new(0usize));
    let failed = Arc::new(Mutex::new(Vec::new()));

    // 使用线程池并发下载
    let pool = threadpool::ThreadPool::new(MAX_CONCURRENT_DOWNLOADS);

    for (url, path, lib_name) in download_tasks {
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);

        pool.execute(move || {
            // 获取文件大小以决定是否使用多线程下载
            let client = Client::new();
            let file_size = match client.head(&url).send() {
                Ok(resp) => resp
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0),
                Err(_) => 0,
            };

            // 如果文件大于1MB，使用多线程下载
            let threads = if file_size > MULTITHREAD_THRESHOLD {
                MAX_CONCURRENT_DOWNLOADS.min(file_size as usize)
            } else {
                1
            };

            // 下载文件
            let result = download_file_blocking(&url, &path, threads);

            // 更新进度
            {
                let mut completed_count = completed.lock().unwrap();
                *completed_count += 1;
                let progress = (*completed_count as f64 / total_files as f64) * 100.0;
                println!("下载进度: {:.1}% ({}/{})", progress, *completed_count, total_files);
            }

            // 处理下载失败的情况
            if let Err(e) = result {
                let mut failed_list = failed.lock().unwrap();
                failed_list.push((lib_name, e.to_string()));
            }
        });
    }

    // 等待所有下载任务完成
    pool.join();

    // 检查是否有下载失败的文件
    let failed_list = failed.lock().unwrap();
    if !failed_list.is_empty() {
        eprintln!("\n以下文件下载失败:");
        for (name, error) in failed_list.iter() {
            eprintln!("  - {}: {}", name, error);
        }
        return Err(anyhow!("部分文件下载失败"));
    }

    Ok(())
}
