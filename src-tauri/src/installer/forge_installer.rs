use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{fs, io::Read, path::PathBuf, process::Command, sync::{Arc, Mutex}};
use zip::ZipArchive;
use scraper::{Html, Selector};
use super::downloader_wrapper::download_file_blocking;

/// 运行 Forge 安装器并等待完成
fn run_forge_installer(installer_path: &PathBuf, install_dir: &PathBuf) -> Result<()> {
    println!("运行 Forge 安装器: {:?} -> {:?}", installer_path, install_dir);

    let output = Command::new("java")
        .arg("-jar")
        .arg(installer_path)
        .arg("--installClient")
        .arg(install_dir)
        .output()
        .context("执行 Forge 安装器失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Forge 安装器执行失败: {}", stderr));
    }

    println!("Forge 安装器执行成功");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ForgeVersionInfo {
    #[serde(default)]
    build: i32,
    #[serde(default)]
    version: String,
    #[serde(default)]
    mcversion: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ForgeLibrary {
    name: String,
    url: Option<String>,
    checksums: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LegacyInstallProfile {
    #[serde(default)]
    libraries: Vec<VersionJsonLibrary>,
}

#[derive(Debug, Deserialize)]
struct ForgeInstallProfile {
    versionInfo: serde_json::Value,
    libraries: Option<Vec<ForgeLibrary>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ForgeVersionInfoSection {
    #[serde(default)]
    libraries: Vec<VersionJsonLibrary>,
}

#[derive(Debug, Deserialize)]
struct MinecraftVersionJson {
    libraries: Vec<VersionJsonLibrary>,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionJsonLibrary {
    name: String,
    #[serde(default)]
    downloads: Option<VersionJsonDownloads>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    checksums: Option<Vec<String>>,
    #[serde(default)]
    serverreq: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionJsonDownloads {
    artifact: VersionJsonArtifact,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionJsonArtifact {
    url: String,
    sha1: String,
}

pub fn get_forge_versions(mcversion: &str) -> Result<Vec<String>> {
    let url = format!(
        "https://bmclapi2.bangbang93.com/forge/minecraft/{}",
        mcversion
    );
    let resp = Client::new().get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let infos: Vec<ForgeVersionInfo> = resp.json()?;
    let versions = infos.into_iter().map(|info| info.version).collect();
    Ok(versions)
}

pub fn try_official_source(mcversion: &str) -> Result<Vec<String>> {
    let url = format!(
        "https://files.minecraftforge.net/net/minecraftforge/forge/index_{}.html",
        mcversion
    );
    println!("使用备选源: {}", url);
    let resp = Client::new().get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("备选源请求失败, HTTP 状态码: {}", resp.status()));
    }
    let html = resp.text()?;
    parse_versions_from_html(&html)
}

fn parse_versions_from_html(html: &str) -> Result<Vec<String>> {
    let document = Html::parse_document(html);
    let tbody_selector = Selector::parse("tbody").unwrap();
    let tr_selector = Selector::parse("tr").unwrap();
    let td_selector = Selector::parse("td").unwrap();
    let tbody = document.select(&tbody_selector).next()
        .ok_or_else(|| anyhow!("未找到 tbody 元素"))?;
    let mut versions = Vec::new();
    for tr in tbody.select(&tr_selector) {
        if let Some(td) = tr.select(&td_selector).next() {
            let version_text = td.text().collect::<String>();
            let cleaned = version_text
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            if cleaned.chars().all(|c| c.is_ascii_digit() || c == '.') && !cleaned.is_empty() {
                versions.push(cleaned);
            }
        }
    }
    if versions.is_empty() {
        return Err(anyhow!("在 HTML 中未找到任何版本"));
    }
    Ok(versions)
}

pub fn install_forge(mcversion: &str, forgeversion: &str, download_dir: &str) -> Result<()> {
    let download_dir = PathBuf::from(download_dir);
    let installerurl = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{}-{}/forge-{}-{}-installer.jar",
        mcversion, forgeversion, mcversion, forgeversion
    );
    let installername = format!("forge-{}-{}-installer.jar", mcversion, forgeversion);
    let installerpath = PathBuf::from(&installername);
    println!("Forge 安装器下载链接: {}", installerurl);

    // 尝试下载，如果失败则使用备用链接
    let download_result = download_file_with_progress(&installerurl, &installerpath, 16);
    if download_result.is_err() {
        println!("原链接下载失败，尝试备用链接...");
        let fallback_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{}-{}-{}/forge-{}-{}-{}-installer.jar",
            mcversion, forgeversion, mcversion, mcversion, forgeversion, mcversion
        );
        println!("备用 Forge 安装器下载链接: {}", fallback_url);
        download_file_with_progress(&fallback_url, &installerpath, 16)
            .with_context(|| format!("下载 Forge 安装器失败: {}", fallback_url))?;
    } else {
        download_result?;
    }
    println!("Forge 安装器下载完成: {:?}", installerpath);

    // 检查安装器中是否同时存在 version.json 和 install_profile.json
    let mut has_version_json = false;
    let mut has_install_profile = false;

    let mut zip = ZipArchive::new(fs::File::open(&installerpath)?)?;
    let mut install_profile: Option<ForgeInstallProfile> = None;
    let mut universal_jar = None;
    let mut fallback_universal_jar = None;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let entry_name = entry.name().to_string();

        if entry_name.ends_with("install_profile.json") {
            has_install_profile = true;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            println!("找到 install_profile.json");
            match serde_json::from_slice::<ForgeInstallProfile>(&buf) {
                Ok(profile) => install_profile = Some(profile),
                Err(_) => {
                    let _legacy: LegacyInstallProfile =
                        serde_json::from_slice(&buf)
                            .context("解析旧版 install_profile.json 失败")?;
                    println!("旧版 install_profile.json，已转换为 version.json");
                }
            }
        } else if entry_name.ends_with("version.json") {
            has_version_json = true;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            println!("找到 version.json");
        } else if entry_name.starts_with("maven/net/minecraftforge/forge/") {
            let parts: Vec<&str> = entry_name.split('/').collect();
            if parts.len() >= 5 {
                let folder = parts[4];
                let rel_path = if parts.len() > 5 { parts[5..].join("/") } else { String::new() };
                let dest_dir = download_dir
                    .join("libraries")
                    .join("net")
                    .join("minecraftforge")
                    .join("forge")
                    .join(folder);
                fs::create_dir_all(&dest_dir)?;
                if entry_name.ends_with('/') {
                    println!("创建目录: {:?}", dest_dir.join(&rel_path));
                } else {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    let dest_path = if rel_path.is_empty() {
                        dest_dir.clone()
                    } else {
                        dest_dir.join(&rel_path)
                    };
                    if let Some(p) = dest_path.parent() {
                        fs::create_dir_all(p)?;
                    }
                    fs::write(&dest_path, &buf)?;
                    println!("提取到: {:?}", dest_path);
                    if dest_path.extension().and_then(|s| s.to_str()) == Some("jar") {
                        universal_jar = Some((dest_path.to_string_lossy().to_string(), buf));
                        println!("找到可能的 universal.jar: {:?}", dest_path);
                    }
                }
            }
        } else if entry_name.contains("-universal.jar") {
            println!("发现 universal.jar: {}", entry_name);
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            universal_jar = Some((entry_name.clone(), buf));
            println!("保存为 universal.jar: {}", entry_name);
        } else if entry_name.ends_with(".jar") && entry_name.contains("minecraftforge-") {
            println!("发现候选 universal.jar: {}", entry_name);
            if fallback_universal_jar.is_none() {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                fallback_universal_jar = Some((entry_name.clone(), buf));
                println!("保存为备用 universal.jar: {}", entry_name);
            }
        }
    }

    if universal_jar.is_none() {
        universal_jar = fallback_universal_jar;
    }

    if has_version_json && has_install_profile {
        println!("检测到 version.json 和 install_profile.json，使用 Forge 安装器进行安装");
        run_forge_installer(&installerpath, &download_dir)?;
        println!("Forge 安装完成！版本ID：{}-forge{}", mcversion, forgeversion);
        fs::remove_file(&installerpath).context("删除 Forge 安装器失败")?;
        println!("已删除 Forge 安装器: {:?}", installerpath);
        return Ok(());
    }

    if let Some(profile) = install_profile {
        println!("处理 install_profile.json 安装流程");

        let versionid = format!("{}-forge{}", mcversion, forgeversion);
        let versionsdir = download_dir
            .join("versions")
            .join(&versionid);
        fs::create_dir_all(&versionsdir)?;

        let versionjsonbytes_value = serde_json::to_vec(&profile.versionInfo)
            .context("序列化 versionInfo 失败")?;
        if let Some(extra_libraries) = &profile.libraries {
            let mut libraries = Vec::new();
            for extra_lib in extra_libraries {
                libraries.push((
                    extra_lib.name.as_str(),
                    extra_lib.url.as_ref().map(|u| u.clone()),
                    extra_lib.checksums.as_ref().map(|c| c.clone()),
                    None
                ));
            }
            download_libraries_concurrent(libraries, &download_dir)?;
        }
        let versionjsonpath = versionsdir.join("version.json");
        fs::write(&versionjsonpath, &versionjsonbytes_value)?;
        println!("version.json 已保存到: {:?}", versionjsonpath);

        let profile_data: ForgeVersionInfoSection = serde_json::from_slice(&versionjsonbytes_value)
            .context("解析 version.json 失败")?;
        handle_universal_jar(&profile_data, &universal_jar, &download_dir)?;
        let mut libraries = Vec::new();
        for lib in profile_data.libraries.iter().skip(1) {
            if lib.name.contains("net.minecraftforge:forge") {
                println!("跳过库 {}（已忽略net.minecraftforge:forge）", lib.name);
                continue;
            }
            if let Some(url) = &lib.url {
                libraries.push((
                    lib.name.as_str(),
                    Some(url.clone()),
                    lib.checksums.as_ref().map(|c| c.clone()),
                    None
                ));
            } else {
                println!("下载库（无url）: {}", lib.name);
                libraries.push((
                    lib.name.as_str(),
                    None,
                    None,
                    Some("https://libraries.minecraft.net".to_string())
                ));
            }
        }
        download_libraries_concurrent(libraries, &download_dir)?;
    }
    fs::remove_file(&installerpath).context("删除 Forge 安装器失败")?;
    println!("已删除 Forge 安装器: {:?}", installerpath);
    println!("Forge 安装完成！版本ID：{}-{}", mcversion, forgeversion);
    Ok(())
}

fn handle_universal_jar(profile: &ForgeVersionInfoSection, universal_jar: &Option<(String, Vec<u8>)>, download_dir: &PathBuf) -> Result<()> {
    if let Some((_, content)) = universal_jar {
        if let Some(first_lib) = profile.libraries.first() {
            save_universal_jar(&first_lib.name, content, download_dir)?;
            return Ok(());
        }
    }
    if !profile.libraries.is_empty() {
        Err(anyhow!("在安装器中未找到 universal.jar"))
    } else {
        Ok(())
    }
}

fn save_universal_jar(lib_name: &str, content: &[u8], download_dir: &PathBuf) -> Result<()> {
    let parts: Vec<&str> = lib_name.split(':').collect();
    let group = parts[0];
    let artifact = parts[1];
    let version = parts[2];
    let grouppath = group.replace('.', "/");
    let librarydir = download_dir
        .join("libraries")
        .join(&grouppath)
        .join(artifact)
        .join(version);
    fs::create_dir_all(&librarydir)?;
    let jarname = format!("{}-{}.jar", artifact, version);
    let localjarpath = librarydir.join(&jarname);
    fs::write(&localjarpath, content)?;
    println!("使用安装器中的 universal.jar 作为第一个库: {:?}", localjarpath);
    Ok(())
}

/// 使用线程池并发下载多个库文件
fn download_libraries_concurrent(
    libraries: Vec<(&str, Option<String>, Option<Vec<String>>, Option<String>)>,
    download_dir: &PathBuf,
) -> Result<()> {
    use threadpool::ThreadPool;

    const MAX_CONCURRENT_DOWNLOADS: usize = 16;

    let mut download_tasks = Vec::new();
    for (name, url, checksums, baseurl) in libraries {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() != 3 {
            eprintln!("无效的 library name 格式: {}", name);
            continue;
        }
        let group = parts[0];
        let artifact = parts[1];
        let version = parts[2];
        let grouppath = group.replace('.', "/");
        let librarydir = download_dir
            .join("libraries")
            .join(&grouppath)
            .join(artifact)
            .join(version);
        fs::create_dir_all(&librarydir)?;
        let jarname = format!("{}-{}.jar", artifact, version);
        let localjarpath = librarydir.join(&jarname);

        if localjarpath.exists() {
            println!("库已存在，跳过下载: {:?}", localjarpath);
            continue;
        }

        let downloadurl = if let Some(u) = url {
            u.clone()
        } else if let Some(base) = baseurl {
            let remotepath = format!("{}/{}/{}/{}", grouppath, artifact, version, jarname);
            format!("{}/{}", base.trim_end_matches('/'), remotepath)
        } else {
            let remotepath = format!("{}/{}/{}/{}", grouppath, artifact, version, jarname);
            format!("https://libraries.minecraft.net/{}", remotepath)
        };

        download_tasks.push((downloadurl, localjarpath, name.to_string(), checksums.clone()));
    }

    let total_files = download_tasks.len();
    if total_files == 0 {
        return Ok(());
    }

    println!("准备并发下载 {} 个库文件...", total_files);

    let completed = Arc::new(Mutex::new(0usize));
    let failed = Arc::new(Mutex::new(Vec::new()));

    let pool = ThreadPool::new(MAX_CONCURRENT_DOWNLOADS);

    for (downloadurl, localjarpath, lib_name, checksums) in download_tasks {
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);

        pool.execute(move || {
            let result = if let Some(ref checksums) = checksums {
                download_and_verify_library_internal(&downloadurl, &localjarpath, checksums)
            } else {
                download_library_internal(&downloadurl, &localjarpath)
            };

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

fn download_and_verify_library_internal(url: &str, path: &PathBuf, expected_checksums: &[String]) -> Result<()> {
    download_file_with_progress(url, path, 16)?;

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0; 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let hash = hasher.finalize();
    let hex_hash = hex::encode(hash);
    if !expected_checksums.iter().any(|s| s == &hex_hash) {
        return Err(anyhow!(
            "SHA1 校验失败: {} (期望值: {:?})",
            hex_hash,
            expected_checksums
        ));
    }
    println!("校验成功: {}", hex_hash);
    Ok(())
}

fn download_library_internal(url: &str, path: &PathBuf) -> Result<()> {
    download_file_with_progress(url, path, 16)?;
    Ok(())
}

fn download_file_with_progress(url: &str, path: &PathBuf, max_threads: usize) -> Result<()> {
    let client = Client::new();
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
