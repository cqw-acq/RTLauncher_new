use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use super::downloader_wrapper::download_file_blocking;

/// 运行 NeoForge 安装器并等待完成
fn run_neoforge_installer(installer_path: &PathBuf, install_dir: &PathBuf) -> Result<()> {
    println!("运行 NeoForge 安装器: {:?} -> {:?}", installer_path, install_dir);

    let output = Command::new("java")
        .arg("-jar")
        .arg(installer_path)
        .arg("--installClient")
        .arg(install_dir)
        .output()
        .context("执行 NeoForge 安装器失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("NeoForge 安装器执行失败: {}", stderr));
    }

    println!("NeoForge 安装器执行成功");
    Ok(())
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct NeoForgeVersionList {
    #[serde(default)]
    isSnapshot: bool,
    versions: Vec<String>,
}

pub fn get_neoforge_versions(mc_version: &str) -> Result<Vec<String>> {
    let stripped = mc_version.strip_prefix("1.").unwrap_or(mc_version);
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let resp = Client::new().get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let version_list: NeoForgeVersionList = resp.json()?;
    let filtered = version_list
        .versions
        .into_iter()
        .filter(|v| v.contains(stripped))
        .collect::<Vec<_>>();
    Ok(filtered)
}

pub fn install_neoforge(mc_version: &str, neoforge_version: &str, download_dir: &str) -> Result<()> {
    let minecraft_dir = PathBuf::from(download_dir);
    fs::create_dir_all(&minecraft_dir)?;

    let base_url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}",
        neoforge_version
    );
    let jar_name = format!("neoforge-{}-installer.jar", neoforge_version);
    let jar_url = format!("{}/{}", base_url, jar_name);

    let installer_path = PathBuf::from(&jar_name);
    download_file_with_progress(&jar_url, &installer_path, 16)
        .with_context(|| format!("下载 Neoforge 安装器失败: {}", jar_url))?;
    println!("NeoForge 安装器下载完成: {:?}", installer_path);

    run_neoforge_installer(&installer_path, &minecraft_dir)?;

    fs::remove_file(&installer_path).context("删除 NeoForge 安装器失败")?;
    println!("已删除 NeoForge 安装器: {:?}", installer_path);

    println!("NeoForge 安装完成！版本ID：{}-neoforge{}", mc_version, neoforge_version);
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
