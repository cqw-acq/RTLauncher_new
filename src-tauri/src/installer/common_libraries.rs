use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use super::downloader_wrapper::download_file_blocking;

#[derive(Debug, Deserialize)]
pub struct LibraryItem {
    pub name: String,
    pub url: Option<String>,
    #[serde(default)]
    pub md5: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub natives: Option<serde_json::Value>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<Os>,
}

#[derive(Debug, Deserialize)]
pub struct Os {
    pub name: String,
}

pub fn parse_library_path_for_fs(name: &str) -> (PathBuf, String) {
    let parts: Vec<&str> = name.split(':').collect();
    let group_path = parts[0].replace('.', &std::path::MAIN_SEPARATOR.to_string());
    let artifact_id = parts[1];
    let version = parts[2];

    let mut path = PathBuf::new();
    path.push(group_path);
    path.push(artifact_id);
    path.push(version);

    let jar_name = format!("{}-{}.jar", artifact_id, version);
    (path, jar_name)
}

pub fn parse_library_path_for_url(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    let group_id = parts[0].replace('.', "/");
    let artifact_id = parts[1];
    let version = parts[2];
    format!(
        "{}/{}/{}/{}-{}.jar",
        group_id, artifact_id, version, artifact_id, version
    )
}

fn calc_md5_of_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut context = md5::Context::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        context.consume(&buffer[..n]);
    }
    let digest = context.compute();
    Ok(format!("{:x}", digest))
}

fn calc_sha1_of_file(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

fn download_sha1_from_url(url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("获取SHA1失败，HTTP状态码: {}", resp.status()));
    }
    let sha1_text = resp.text()?;
    Ok(sha1_text.trim().to_string())
}

pub fn download_libraries(
    libraries: &[LibraryItem],
    default_url: &str,
    check_md5: bool,
    mc_folder_path: &str,
) -> Result<()> {
    for lib in libraries {
        let base_url = lib.url.as_deref().unwrap_or(default_url).trim_end_matches('/');
        let name = &lib.name;
        let (sub_path, jar_name) = parse_library_path_for_fs(name);
        let library_dir = PathBuf::from(mc_folder_path).join("libraries").join(&sub_path);
        fs::create_dir_all(&library_dir)?;
        let local_file_path = library_dir.join(&jar_name);
        let url_sub_path = parse_library_path_for_url(name);
        let download_url = format!("{}/{}", base_url, url_sub_path);
        download_file_blocking(&download_url, &local_file_path, 4)?;

        // 如果没有sha1值，尝试从.sha1文件获取
        let sha1_value = if lib.sha1.is_empty() {
            let sha1_url = format!("{}.sha1", download_url);
            match download_sha1_from_url(&sha1_url) {
                Ok(sha1) => sha1,
                Err(e) => {
                    eprintln!("无法获取 {} 的SHA1值: {}", lib.name, e);
                    String::new()
                }
            }
        } else {
            lib.sha1.clone()
        };

        // 如果有sha1值，则验证文件
        if !sha1_value.is_empty() {
            let sha1_local = calc_sha1_of_file(&local_file_path)?;
            if sha1_local.to_lowercase() != sha1_value.to_lowercase() {
                return Err(anyhow!(
                    "文件 SHA1 校验失败: 本地({}) != 远程({})",
                    sha1_local,
                    sha1_value
                ));
            }
        }

        if check_md5 && !lib.md5.is_empty() {
            let md5_local = calc_md5_of_file(&local_file_path)?;
            if md5_local.to_lowercase() != lib.md5.to_lowercase() {
                return Err(anyhow!(
                    "文件 MD5 校验失败: 本地({}) != 远程({})",
                    md5_local,
                    lib.md5
                ));
            }
        }
    }
    Ok(())
}

/// 使用多线程下载单个文件（根据文件大小自动决定线程数）
pub fn download_file_with_progress(url: &str, path: &Path, max_threads: usize) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let head_resp = client.head(url).send()?;
    let file_size = head_resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

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
pub fn download_libraries_concurrent(
    libraries: &[LibraryItem],
    default_url: &str,
    mc_folder_path: &str,
    max_concurrent: usize,
) -> Result<()> {
    use std::sync::{Arc, Mutex};

    const MULTITHREAD_THRESHOLD: u64 = 1024 * 1024; // 1MB

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

    let completed = Arc::new(Mutex::new(0usize));
    let failed = Arc::new(Mutex::new(Vec::new()));

    let pool = threadpool::ThreadPool::new(max_concurrent);

    for (url, path, lib_name) in download_tasks {
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);

        pool.execute(move || {
            let client = reqwest::blocking::Client::new();
            let file_size = match client.head(&url).send() {
                Ok(resp) => resp
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0),
                Err(_) => 0,
            };

            let threads = if file_size > MULTITHREAD_THRESHOLD {
                max_concurrent.min(file_size as usize)
            } else {
                1
            };

            let result = download_file_blocking(&url, &path, threads);

            {
                let mut completed_count = completed.lock().unwrap();
                *completed_count += 1;
                let progress = (*completed_count as f64 / total_files as f64) * 100.0;
                println!("下载进度: {:.1}% ({}/{})", progress, *completed_count, total_files);
            }

            if let Err(e) = result {
                let mut failed_list = failed.lock().unwrap();
                failed_list.push((lib_name, e.to_string()));
            }
        });
    }

    pool.join();

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
