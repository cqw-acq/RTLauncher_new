use crate::downloader::concurrent_download::{self, DownloadTask};
use crate::downloader::shared_utils::{self, Library, MetaResponse};
use crate::http_client::shared_client;
use anyhow::{anyhow, Result};
use serde_json;
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// 获取 Quilt Loader 版本列表
pub async fn get_quilt_loader_versions(mc_version: &str) -> Result<Vec<String>> {
    let url = "https://meta.quiltmc.org/v3/versions";
    let client = shared_client().await;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let meta: MetaResponse = resp.json().await?;
    shared_utils::parse_meta_versions(meta, mc_version)
}

/// 获取 Quilt API 版本列表
pub async fn get_quilt_api_versions(mc_version: &str) -> Result<Vec<String>> {
    let url = "https://maven.quiltmc.org/repository/release/org/quiltmc/quilted-fabric-api/quilted-fabric-api/maven-metadata.xml";
    let client = shared_client().await;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let xml_text = resp.text().await?;
    
    // 使用共享的 XML 解析工具
    let all_versions = shared_utils::parse_maven_metadata(&xml_text)?;
    // 过滤出匹配当前 MC 版本的 API 版本
    let filtered: Vec<String> = all_versions
        .into_iter()
        .filter(|v| v.contains(mc_version))
        .collect();
    
    Ok(filtered)
}

/// 安装 Quilt Loader
pub async fn install_quilt_loader(
    mc_version: &str,
    quilt_version: &str,
    mc_folder_path: &str,
    progress_tx: Option<mpsc::Sender<f64>>,
) -> Result<String> {
    let url = format!(
        "https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json",
        mc_version, quilt_version
    );
    let client = shared_client().await;
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let profile_json_text = resp.text().await?;

    let version_id = format!("{}-{}-quilt", mc_version, quilt_version);
    let versions_dir = PathBuf::from(mc_folder_path)
        .join("versions")
        .join(&version_id);
    fs::create_dir_all(&versions_dir)?;
    let profile_json_path = versions_dir.join(format!("{}.json", version_id));
    fs::write(&profile_json_path, &profile_json_text)?;

    #[derive(Debug, serde::Deserialize)]
    struct QuiltProfileJson {
        #[serde(default)]
        libraries: Vec<Library>,
    }
    let profile: QuiltProfileJson = serde_json::from_str(&profile_json_text)?;

    // 使用共享的 concurrent_download 批量下载库文件
    download_libraries(
        &profile.libraries,
        "https://maven.quiltmc.org/repository/release",
        mc_folder_path,
        progress_tx,
    )
    .await?;

    // 确保 options.txt 存在并设置语言为中文
    let options_path = versions_dir.join("options.txt");
    if options_path.exists() {
        let content = fs::read_to_string(&options_path)?;
        if content.contains("lang:") {
            let new_content = content
                .lines()
                .map(|line| {
                    if line.trim().starts_with("lang:") {
                        "lang:zh_cn"
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&options_path, new_content)?;
        } else {
            fs::write(&options_path, format!("{}\nlang:zh_cn", content))?;
        }
    } else {
        fs::write(&options_path, "lang:zh_cn")?;
    }
    
    println!("Quilt Loader 安装完成，版本 ID: {}", version_id);
    Ok(version_id)
}

/// 安装 Quilt API
pub async fn install_quilt_api(
    mc_version: &str,
    quilt_api_version: &str,
    mc_folder_path: &str,
    progress_tx: Option<mpsc::Sender<f64>>,
) -> Result<()> {
    let quilt_api_url = format!(
        "https://maven.quiltmc.org/repository/release/org/quiltmc/quilted-fabric-api/quilted-fabric-api/{0}/quilted-fabric-api-{0}.jar",
        quilt_api_version
    );
    let mods_dir = PathBuf::from(mc_folder_path)
        .join("versions")
        .join(mc_version)
        .join("mods");
    fs::create_dir_all(&mods_dir)?;
    let jar_name = format!("quilted-fabric-api-{}.jar", quilt_api_version);

    // 使用共享的 concurrent_download 下载单个文件
    let task = DownloadTask {
        file_name: jar_name,
        target_dir: mods_dir,
        urls: vec![quilt_api_url],
        sha1: None,
    };
    // 将 mpsc::Sender<f64> 转为 download_file 需要的 mpsc::Sender<(u64, u64)>
    // download_file 不需要文件级进度，直接下载单个文件，发送 0→100 的简单进度
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(0.0);
    }
    let single_progress = progress_tx.as_ref().map(|tx| {
        let tx = tx.clone();
        let (inner_tx, mut inner_rx) = mpsc::channel::<(u64, u64)>(16);
        tokio::spawn(async move {
            while let Some((done, total)) = inner_rx.recv().await {
                if total > 0 {
                    let pct = (done as f64 / total as f64) * 100.0;
                    let _ = tx.send(pct).await;
                }
            }
        });
        inner_tx
    });
    match concurrent_download::download_file(&task, single_progress, None).await {
        crate::downloader::modular_download::SingleDownloadResult::Success { .. } => {}
        crate::downloader::modular_download::SingleDownloadResult::Failed { error, .. } => {
            return Err(anyhow!("下载 Quilt API 失败: {}", error));
        }
    }
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(100.0);
    }

    println!("Quilt API 安装完成，版本: {}", quilt_api_version);
    Ok(())
}

/// 使用共享的 concurrent_download 批量下载库文件
async fn download_libraries(
    libraries: &[Library],
    default_url: &str,
    mc_folder_path: &str,
    progress_tx: Option<mpsc::Sender<f64>>,
) -> Result<()> {
    let mut tasks = Vec::new();
    for lib in libraries {
        let base_url = lib
            .url
            .as_deref()
            .unwrap_or(default_url)
            .trim_end_matches('/');
        let name = lib.name.clone();
        let (sub_path, jar_name) = shared_utils::parse_library_path_for_fs(&name)?;
        let library_dir = PathBuf::from(mc_folder_path)
            .join("libraries")
            .join(&sub_path);
        let url_sub_path = shared_utils::parse_library_path_for_url(&name)?;
        let download_url = format!("{}/{}", base_url, url_sub_path);

        tasks.push(DownloadTask {
            file_name: jar_name,
            target_dir: library_dir,
            urls: vec![download_url],
            sha1: lib
                .downloads
                .as_ref()
                .and_then(|d| d.artifact.as_ref())
                .map(|a| a.sha1.clone()),
        });
    }

    if tasks.is_empty() {
        return Ok(());
    }

    println!("准备下载 {} 个库文件...", tasks.len());

    let result = concurrent_download::download_all(tasks, progress_tx).await;
    if !result.failures.is_empty() {
        eprintln!("\n以下文件下载失败:");
        for f in &result.failures {
            eprintln!("  - {}: {}", f.file_name, f.error);
        }
        return Err(anyhow!("部分文件下载失败"));
    }

    Ok(())
}