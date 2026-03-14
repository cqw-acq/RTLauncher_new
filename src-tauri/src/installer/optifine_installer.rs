use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    fs, path::{Path, PathBuf}, process::Command
};
use super::downloader_wrapper::download_file_blocking;

#[derive(Debug, Deserialize)]
struct OptifineInfo {
    #[serde(default)]
    _id: String,
    #[serde(default)]
    mcversion: String,
    #[serde(default)]
    patch: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    filename: String,
}

pub fn list_optifine_versions(mc_version: &str) -> Result<Vec<String>> {
    let url = "https://optifine.net/downloads";
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)                      AppleWebKit/537.36 (KHTML, like Gecko)                      Chrome/112.0.0.0 Safari/537.36")
        .build()
        .context("构造 reqwest 客户端失败")?;

    let resp = client.get(url).send()
        .context("请求 OptiFine Downloads 页面失败")?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }

    let html = resp.text().context("读取 HTML 内容失败")?;

    let version_pattern = format!(r#"<h2>Minecraft {}</h2>"#, mc_version.replace(".", r"\."));
    let version_re = Regex::new(&version_pattern)?;

    let link_re = Regex::new(r#"href=['"]http://optifine\.net/adloadx\?f=([^'"]+)['"]"#)?;

    let mut versions = Vec::new();

    if let Some(version_pos) = version_re.find(&html) {
        let content = &html[version_pos.start()..];

        let next_version_re = Regex::new(r#"<h2>Minecraft \d+\.\d+\.\d+</h2>"#)?;
        let end_pos = if let Some(next_match) = next_version_re.find(&content[version_pattern.len()..]) {
            version_pattern.len() + next_match.start()
        } else {
            content.len()
        };

        let version_content = &content[..end_pos];

        for caps in link_re.captures_iter(version_content) {
            if let Some(filename) = caps.get(1) {
                versions.push(filename.as_str().to_string());
            }
        }
    }

    Ok(versions)
}

pub fn install_optifine_alone(optifine_version: &str, minecraft_dir: &str, opt_wrapper_path: &str, mc_version: &str) -> Result<()> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/112.0.0.0 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::REFERER, "https://optifine.net/".parse().unwrap());
            headers
        })
        .build()
        .context("构造 reqwest 客户端失败")?;

    let adload_url = format!("https://optifine.net/adloadx?f={}", optifine_version);
    let response = client.get(&adload_url)
        .send()
        .context("请求 OptiFine adloadx 页面失败")?;

    println!("响应 URL: {}", response.url());
    println!("响应头: {:#?}", response.headers());

    let html = response.text().context("读取 adloadx 页面文本失败")?;

    println!("HTML 内容预览:\n{}", &html[..html.len().min(500)]);

    let re = Regex::new(r#"(?i)href=['"](downloadx\?f=[^'"]*?)['"]"#).unwrap();
    let download_url = if let Some(caps) = re.captures(&html) {
        let mut link = caps.get(1).unwrap().as_str().to_string();
        link = link.replace("&amp;", "&");
        format!("https://optifine.net/{}", link)
    } else {
        return Err(anyhow!("未在 HTML 中找到下载链接 (Download)\n完整 HTML:\n{}", html));
    };
    println!("解析到下载链接: {}", download_url);

    let jar_filename = optifine_version;
    let minecraft_path = PathBuf::from(minecraft_dir);
    let optifine_dir = minecraft_path.join("optifine");
    fs::create_dir_all(&optifine_dir)
        .context("创建 .minecraft/optifine 目录失败")?;
    let target_path = optifine_dir.join(jar_filename);
    download_file_with_progress(&download_url, &target_path, 16)?;
    println!("OptiFine jar 下载完成: {:?}", target_path);

    let format_path = |p: &PathBuf| -> String {
        p.to_string_lossy().replace("\\", "/")
    };

    let sep = if cfg!(windows) { ";" } else { ":" };
    let target_str = format_path(&target_path);
    let classpath = format!("{}{}{}", opt_wrapper_path, sep, target_str);

    let o = format!("{}-{}", mc_version, jar_filename);
    let args: Vec<String> = vec![
        "-cp".to_string(),
        classpath,
        "net.stevexmh.OptifineInstaller".to_string(),
        minecraft_dir.to_string(),
        o,
    ];

    let mut cmd = Command::new("java");
    cmd.args(&args);

    println!("尝试执行命令:{:?}", cmd);
    let status = cmd.status().context("启动 OptiFine 安装器失败")?;
    if !status.success() {
        return Err(anyhow!("OptiFine 安装器执行失败,退出码: {:?}", status.code()));
    }
    println!("OptiFine 安装器执行完成.");

    Ok(())
}

pub fn install_optifine_with_forge(optifine_version: &str, minecraft_dir: &str) -> Result<()> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/112.0.0.0 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::REFERER, "https://optifine.net/".parse().unwrap());
            headers
        })
        .build()
        .context("构造 reqwest 客户端失败")?;

    let adload_url = format!("https://optifine.net/adloadx?f={}", optifine_version);
    let response = client.get(&adload_url)
        .send()
        .context("请求 OptiFine adloadx 页面失败")?;

    println!("响应 URL: {}", response.url());
    println!("响应头: {:#?}", response.headers());

    let html = response.text().context("读取 adloadx 页面文本失败")?;
    println!("HTML 内容预览:\n{}", &html[..html.len().min(500)]);

    let re = Regex::new(r#"(?i)href=['"](downloadx\?f=[^'"]*?)['"]"#).unwrap();
    let download_url = if let Some(caps) = re.captures(&html) {
        let mut link = caps.get(1).unwrap().as_str().to_string();
        link = link.replace("&amp;", "&");
        format!("https://optifine.net/{}", link)
    } else {
        return Err(anyhow!("未在 HTML 中找到下载链接 (Download)\n完整 HTML:\n{}", html));
    };
    println!("解析到下载链接: {}", download_url);

    let jar_filename = optifine_version;
    let minecraft_path = PathBuf::from(minecraft_dir);
    let mods_dir = minecraft_path.join("mods");
    fs::create_dir_all(&mods_dir).context("创建 mods 目录失败")?;
    let target_path = mods_dir.join(jar_filename);

    download_file_with_progress(&download_url, &target_path, 16)?;
    println!("OptiFine jar 已下载到 mods 文件夹: {:?}", target_path);

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
