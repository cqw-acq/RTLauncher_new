use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

use super::downloader_wrapper::download_file_blocking;
use super::common_libraries::{LibraryItem, Rule};

/// 根据规则判断是否应该下载
fn should_download_by_rules(rules: &[Rule]) -> bool {
    let os_name = std::env::consts::OS;

    for rule in rules {
        if rule.action == "disallow" {
            if let Some(os) = &rule.os {
                let target_os = match os.name.as_str() {
                    "osx" => "macos",
                    _ => &os.name
                };
                if target_os == os_name {
                    return false;
                }
            }
        } else if rule.action == "allow" {
            if let Some(os) = &rule.os {
                let target_os = match os.name.as_str() {
                    "osx" => "macos",
                    _ => &os.name
                };
                if target_os != os_name {
                    return false;
                }
            }
        }
    }
    true
}

/// 获取当前系统的natives后缀
fn get_native_suffix(natives: &serde_json::Value) -> Result<String> {
    let os_name = std::env::consts::OS;

    if let Some(natives_map) = natives.as_object() {
        let suffix_key = if os_name == "macos" { "osx" } else { os_name };

        if let Some(suffix) = natives_map.get(suffix_key) {
            if let Some(suffix_str) = suffix.as_str() {
                if suffix_str.contains("${arch}") && os_name == "windows" {
                    let arch = if std::mem::size_of::<usize>() == 8 { "64" } else { "32" };
                    return Ok(suffix_str.replace("${arch}", arch));
                }
                return Ok(suffix_str.to_string());
            }
        }
    }
    Err(anyhow!("无法获取natives后缀"))
}

/// 针对单个库，根据是否有url字段选择下载源并下载
fn download_library(lib: &LibraryItem, minecraft_dir: &Path, installer_jar_path: Option<&Path>) -> Result<()> {
    if let Some(rules) = &lib.rules {
        if !should_download_by_rules(rules) {
            println!("跳过 {} (根据规则不适用于当前系统)", lib.name);
            return Ok(());
        }
    }

    let (sub_path, jar_name) = super::common_libraries::parse_library_path_for_fs(&lib.name);
    let library_dir = minecraft_dir.join("libraries").join(&sub_path);
    fs::create_dir_all(&library_dir)?;

    let (jar_name, download_url) = if let Some(natives) = &lib.natives {
        let native_suffix = get_native_suffix(natives)?;
        let base_jar_name = jar_name.trim_end_matches(".jar");
        let native_jar_name = format!("{}-{}.jar", base_jar_name, native_suffix);

        let download_url = if let Some(url) = &lib.url {
            let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
            let url_sub_path = url_sub_path.replace(".jar", &format!("-{}.jar", native_suffix));
            format!("{}/{}", url.trim_end_matches('/'), url_sub_path)
        } else {
            let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
            let url_sub_path = url_sub_path.replace(".jar", &format!("-{}.jar", native_suffix));
            format!("https://libraries.minecraft.net/{}", url_sub_path)
        };
        (native_jar_name, download_url)
    } else {
        let download_url = if let Some(url) = &lib.url {
            let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
            format!("{}/{}", url.trim_end_matches('/'), url_sub_path)
        } else {
            let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
            format!("https://libraries.minecraft.net/{}", url_sub_path)
        };
        (jar_name, download_url)
    };

    let local_file_path = library_dir.join(&jar_name);
    println!("尝试下载 {} 从 {}", lib.name, download_url);
    match download_file_blocking(&download_url, &local_file_path, 4) {
        Ok(()) => {
            println!("成功下载 {} 到 {:?}", lib.name, local_file_path);
            Ok(())
        }
        Err(e) => {
            println!("下载 {} 失败: {}", lib.name, e);

            if lib.name.starts_with("com.mumfrey:liteloader:") && installer_jar_path.is_some() {
                let installer_path = installer_jar_path.unwrap();
                println!("尝试从安装器 jar 中提取 {}...", lib.name);

                let jar_file = fs::File::open(installer_path)
                    .context("无法打开安装器 jar 文件")?;
                let mut zip = ZipArchive::new(jar_file)
                    .context("无法打开压缩包")?;

                let mut jar_entry_name = None;
                for i in 0..zip.len() {
                    let entry = zip.by_index(i)?;
                    let entry_name = entry.name();

                    if entry_name.ends_with(".jar") && !entry_name.contains('/') {
                        jar_entry_name = Some(entry_name.to_string());
                        break;
                    }
                }

                if let Some(entry_name) = jar_entry_name {
                    let mut zip = ZipArchive::new(fs::File::open(installer_path)?)?;
                    let mut entry = zip.by_name(&entry_name)?;
                    let mut buffer = Vec::new();
                    entry.read_to_end(&mut buffer)?;

                    fs::write(&local_file_path, &buffer)
                        .context("写入提取的 jar 文件失败")?;

                    println!("成功从安装器中提取 {} 到 {:?}", lib.name, local_file_path);
                    return Ok(());
                } else {
                    println!("在安装器中未找到 jar 文件");
                }
            }

            Err(e)
        }
    }
}

/// 下载所有库
fn download_all_libraries(libraries: &[LibraryItem], minecraft_dir: &Path, installer_jar_path: Option<&Path>) -> Result<()> {
    const MAX_CONCURRENT_DOWNLOADS: usize = 8;
    const MULTITHREAD_THRESHOLD: u64 = 1024 * 1024;

    let mut download_tasks = Vec::new();
    for lib in libraries {
        if let Some(rules) = &lib.rules {
            if !should_download_by_rules(rules) {
                println!("跳过 {} (根据规则不适用于当前系统)", lib.name);
                continue;
            }
        }

        let (sub_path, jar_name) = super::common_libraries::parse_library_path_for_fs(&lib.name);
        let library_dir = minecraft_dir.join("libraries").join(&sub_path);
        fs::create_dir_all(&library_dir)?;

        let (jar_name, download_url) = if let Some(natives) = &lib.natives {
            let native_suffix = get_native_suffix(natives)?;
            let base_jar_name = jar_name.trim_end_matches(".jar");
            let native_jar_name = format!("{}-{}.jar", base_jar_name, native_suffix);

            let download_url = if let Some(url) = &lib.url {
                let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
                let url_sub_path = url_sub_path.replace(".jar", &format!("-{}.jar", native_suffix));
                format!("{}/{}", url.trim_end_matches('/'), url_sub_path)
            } else {
                let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
                let url_sub_path = url_sub_path.replace(".jar", &format!("-{}.jar", native_suffix));
                format!("https://libraries.minecraft.net/{}", url_sub_path)
            };
            (native_jar_name, download_url)
        } else {
            let download_url = if let Some(url) = &lib.url {
                let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
                format!("{}/{}", url.trim_end_matches('/'), url_sub_path)
            } else {
                let url_sub_path = super::common_libraries::parse_library_path_for_url(&lib.name);
                format!("https://libraries.minecraft.net/{}", url_sub_path)
            };
            (jar_name, download_url)
        };

        let local_file_path = library_dir.join(&jar_name);
        let lib_name = lib.name.clone();
        let needs_fallback = lib_name.starts_with("com.mumfrey:liteloader:") && installer_jar_path.is_some();
        download_tasks.push((download_url, local_file_path, lib_name, needs_fallback));
    }

    let total_files = download_tasks.len();
    if total_files == 0 {
        return Ok(());
    }

    println!("准备下载 {} 个库文件...", total_files);

    let completed = Arc::new(Mutex::new(0usize));
    let failed = Arc::new(Mutex::new(Vec::new()));

    let pool = threadpool::ThreadPool::new(MAX_CONCURRENT_DOWNLOADS);

    for (url, path, lib_name, needs_fallback) in download_tasks {
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);
        let installer_jar_path = installer_jar_path.map(|p| p.to_path_buf());

        pool.execute(move || {
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

            let threads = if file_size > MULTITHREAD_THRESHOLD {
                MAX_CONCURRENT_DOWNLOADS.min(file_size as usize)
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
                if needs_fallback {
                    if let Some(ref installer_path) = installer_jar_path {
                        println!("尝试从安装器 jar 中提取 {}...", lib_name);

                        match extract_from_installer(installer_path, &path) {
                            Ok(()) => {
                                println!("成功从安装器中提取 {} 到 {:?}", lib_name, path);
                                return;
                            }
                            Err(e) => {
                                println!("从安装器中提取失败: {}", e);
                            }
                        }
                    }
                }

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

fn download_file_with_progress(url: &str, path: &Path, max_threads: usize) -> Result<()> {
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

fn extract_from_installer(installer_path: &Path, target_path: &Path) -> Result<()> {
    let jar_file = fs::File::open(installer_path)
        .context("无法打开安装器 jar 文件")?;
    let mut zip = ZipArchive::new(jar_file)
        .context("无法打开压缩包")?;

    let mut jar_entry_name = None;
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let entry_name = entry.name();

        if entry_name.ends_with(".jar") && !entry_name.contains('/') {
            jar_entry_name = Some(entry_name.to_string());
            break;
        }
    }

    if let Some(entry_name) = jar_entry_name {
        let mut zip = ZipArchive::new(fs::File::open(installer_path)?)?;
        let mut entry = zip.by_name(&entry_name)?;
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)?;

        fs::write(target_path, &buffer)
            .context("写入提取的 jar 文件失败")?;

        Ok(())
    } else {
        Err(anyhow!("在安装器中未找到 jar 文件"))
    }
}

/// 从 LiteLoader 官方 JSON 获取可用子版本列表
pub fn get_liteloader_versions(mc_version: &str) -> Result<Vec<String>> {
    let url = "https://dl.liteloader.com/versions/versions.json";
    let client = Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let text = resp.text()?;
    let root: Value = serde_json::from_str(&text)
        .context("解析 versions.json 失败")?;

    let versions_obj = root.get("versions")
        .ok_or_else(|| anyhow!("JSON 中未找到 'versions' 字段"))?;
    let sub_obj = versions_obj.get(mc_version)
        .ok_or_else(|| anyhow!("未找到大版本 {} 的配置", mc_version))?;

    let mut results = Vec::new();

    if let Some(artefacts_val) = sub_obj.get("artefacts") {
        if let Some(artefacts_map) = artefacts_val.as_object() {
            for (_artifact_key, artifact_val) in artefacts_map.iter() {
                if let Some(item_map) = artifact_val.as_object() {
                    for (_key, item_val) in item_map.iter() {
                        if let Some(obj) = item_val.as_object() {
                            if let Some(ver_val) = obj.get("version") {
                                if let Some(ver_str) = ver_val.as_str() {
                                    results.push(ver_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(snap_val) = sub_obj.get("snapshots") {
        if let Some(snap_obj) = snap_val.as_object() {
            if let Some(lite_obj) = snap_obj.get("com.mumfrey:liteloader") {
                if let Some(lite_map) = lite_obj.as_object() {
                    for (_k, v) in lite_map.iter() {
                        if let Some(item) = v.as_object() {
                            if let Some(ver_val) = item.get("version") {
                                if let Some(ver_str) = ver_val.as_str() {
                                    results.push(ver_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if results.is_empty() {
        return Err(anyhow!("未在 {} 中找到任何可用子版本 (artefacts + snapshots)", mc_version));
    }
    Ok(results)
}

/// 安装 LiteLoader
pub fn install_liteloader(mc_version: &str, lite_version: &str, minecraft_dir: &Path) -> Result<()> {
    let version_id = format!("{}-{}-liteloader", mc_version, lite_version);

    let (download_url, _temp_name) = if lite_version.contains('-') {
        let prefix = parse_mc_prefix(lite_version);
        let url = format!(
            "https://jenkins.liteloader.com/job/LiteLoaderInstaller%20{}/lastSuccessfulBuild/artifact/build/libs/liteloader-installer-{}-00-SNAPSHOT.jar",
            prefix, prefix
        );
        println!("下载链接: {}", url);
        (url, "liteloader-installer-temp.jar".to_string())
    } else {
        let replaced = lite_version.replace('_', "-");
        let url = format!(
            "https://dl.liteloader.com/redist/{}/liteloader-installer-{}.jar",
            mc_version, replaced
        );
        println!("下载链接: {}", url);
        (url, "liteloader-installer-temp.jar".to_string())
    };

    let jar_path = PathBuf::from("liteloader-installer-temp.jar");
    download_file_with_progress(&download_url, &jar_path, 16)?;
    println!("LiteLoader 安装器下载完成: {:?}", jar_path);

    fs::create_dir_all(minecraft_dir)?;

    let install_profile_data = {
        let jar_file = fs::File::open(&jar_path)?;
        let mut zip = ZipArchive::new(jar_file).context("无法打开压缩包")?;
        let mut install_profile_bytes = None;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.name().ends_with("install_profile.json") {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                install_profile_bytes = Some(buf);
                break;
            }
        }
        if install_profile_bytes.is_none() {
            return Err(anyhow!("未找到 install_profile.json"));
        }
        install_profile_bytes.unwrap()
    };
    println!("找到 install_profile.json");

    let json_str = String::from_utf8_lossy(&install_profile_data).into_owned();
    let re = Regex::new(r",\s*([\]}])").unwrap();
    let fixed_json_str = re.replace_all(&json_str, "$1");
    let value: Value = serde_json::from_str(&fixed_json_str)
        .with_context(|| format!("解析 install_profile.json 失败，内容:\n{}", fixed_json_str))?;

    let version_info = value.get("versionInfo")
        .ok_or_else(|| anyhow!("install_profile.json 中未找到 'versionInfo' 字段"))?;

    let versions_dir = minecraft_dir.join("versions").join(&version_id);
    fs::create_dir_all(&versions_dir)?;
    let profile_json_path = versions_dir.join(format!("{}.json", version_id));
    let version_info_json = serde_json::to_string_pretty(version_info)?;
    fs::write(&profile_json_path, &version_info_json)
        .context("写入 versionInfo.json 失败")?;

    let json_str = String::from_utf8_lossy(&install_profile_data).into_owned();
    let re = Regex::new(r",\s*([\]}])").unwrap();
    let fixed_json_str = re.replace_all(&json_str, "$1");
    let value: Value = serde_json::from_str(&fixed_json_str)
        .with_context(|| format!("解析 install_profile.json 失败，内容:\n{}", fixed_json_str))?;

    let version_info = value.get("versionInfo")
        .ok_or_else(|| anyhow!("install_profile.json 中未找到 'versionInfo' 字段"))?;
    let libraries_val = version_info.get("libraries")
        .ok_or_else(|| anyhow!("versionInfo 中未找到 'libraries' 字段"))?;
    let libraries: Vec<LibraryItem> = serde_json::from_value(libraries_val.clone())
        .context("解析 libraries 字段失败")?;

    println!("将要下载的库:");
    for lib in &libraries {
        println!("  - {} (URL: {:?})", lib.name, lib.url);
    }

    download_all_libraries(&libraries, minecraft_dir, Some(&jar_path))?;

    fs::remove_file(&jar_path).context("删除临时 Jar 文件失败")?;
    println!("删除临时 Jar 文件完成");

    println!("正在获取同版本中最新的Forge版本...");
    let forge_versions = match super::forge_installer::get_forge_versions(mc_version) {
        Ok(versions) => versions,
        Err(e) => {
            println!("获取Forge版本失败: {}，尝试备用源...", e);
            match super::forge_installer::try_official_source(mc_version) {
                Ok(versions) => versions,
                Err(e) => {
                    println!("备用源也失败: {}，跳过forge-patch创建", e);
                    println!("LiteLoader 安装完成！版本ID: {}", version_id);
                    return Ok(());
                }
            }
        }
    };

    if let Some(latest_forge_version) = forge_versions.first() {
        println!("选择最新的Forge版本: {}", latest_forge_version);

        let forge_download_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{}-{}/forge-{}-{}-installer.jar",
            mc_version, latest_forge_version, mc_version, latest_forge_version
        );
        let forge_installer_name = format!("forge-{}-{}-installer.jar", mc_version, latest_forge_version);
        let forge_installer_path = PathBuf::from(&forge_installer_name);
        println!("Forge 安装器下载链接: {}", forge_download_url);

        let forge_download_result = download_file_with_progress(&forge_download_url, &forge_installer_path, 16);
        if forge_download_result.is_err() {
            println!("下载Forge安装器失败，跳过forge-patch创建");
        } else {
            println!("Forge 安装器下载完成: {:?}", forge_installer_path);

            let forge_file = fs::File::open(&forge_installer_path)?;
            let mut forge_zip = ZipArchive::new(forge_file)?;
            let mut forge_version_json_bytes = None;

            for i in 0..forge_zip.len() {
                let mut entry = forge_zip.by_index(i)?;
                let entry_name = entry.name().to_string();

                if entry_name.ends_with("install_profile.json") {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    if let Ok(profile) = serde_json::from_slice::<serde_json::Value>(&buf) {
                        if let Some(version_info) = profile.get("versionInfo") {
                            forge_version_json_bytes = Some(serde_json::to_vec_pretty(version_info)?);
                            println!("从install_profile.json中提取versionInfo");
                            break;
                        }
                    }
                } else if entry_name.ends_with("version.json") {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    forge_version_json_bytes = Some(buf);
                    println!("找到version.json");
                    break;
                }
            }

            if let Some(bytes) = forge_version_json_bytes {
                let forge_patch_dir = versions_dir.join("forge-patch");
                fs::create_dir_all(&forge_patch_dir)?;
                let version_patch_path = forge_patch_dir.join("versionPatch.json");
                fs::write(&version_patch_path, &bytes)?;
                println!("versionPatch.json已保存到: {:?}", version_patch_path);
            }

            fs::remove_file(&forge_installer_path).context("删除Forge安装器失败")?;
            println!("删除Forge安装器完成");
        }
    }

    println!("LiteLoader 安装完成！版本ID: {}", version_id);
    Ok(())
}

fn parse_mc_prefix(ver: &str) -> String {
    let mut prefix = String::new();
    for c in ver.chars() {
        if c == '-' || c == '_' {
            break;
        }
        prefix.push(c);
    }
    prefix
}
