//! LiteLoader 加载器：版本列表 + 下载 + 安装
//!
//! 依赖库（1.12.2-SNAPSHOT 版本）：
//!   - net.minecraft:launchwrapper:1.12
//!   - org.spongepowered:mixin:0.7.4-SNAPSHOT  (从 https://repo.spongepowered.org/maven/)
//!   - org.ow2.asm:asm-all:5.2
//!   - com.mumfrey:liteloader:1.12.2-SNAPSHOT    (Jenkins runtime jar)

use crate::downloader::concurrent_download::{self, DownloadTask};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const LITELOADER_VERSIONS_URL: &str = "https://dl.liteloader.com/versions/versions.json";

/// 单个库的定义
#[derive(Debug, Clone)]
struct LibraryDef {
    name: String,             // maven 格式：group:artifact:version
    repo_url: Option<String>, // 自定义仓库 URL（空 = 用默认）
}

#[derive(Debug, Clone)]
struct LiteLoaderInfo {
    file: String,               // e.g. "liteloader-1.12.2-SNAPSHOT.jar"
    version: String,            // e.g. "1.12.2-SNAPSHOT"
    tweak_class: String,        // e.g. "com.mumfrey.liteloader.launch.LiteLoaderTweaker"
    libraries: Vec<LibraryDef>, // 依赖的库，包含 section 级和 artifact 级
    repo_url: String,           // 主 Maven repo URL
}

/// 从官方 versions.json 获取指定 MC 版本可用的 LiteLoader 子版本列表
async fn fetch_versions_from_json(mc_version: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("构造 reqwest 客户端失败")?;
    println!("[LiteLoader] 获取版本列表: GET {}", LITELOADER_VERSIONS_URL);
    let resp = client
        .get(LITELOADER_VERSIONS_URL)
        .send()
        .await
        .context("请求 LiteLoader versions.json 失败")?;
    if !resp.status().is_success() {
        bail!(
            "请求 LiteLoader versions.json 失败，状态码: {}",
            resp.status()
        );
    }
    let text = resp.text().await.context("读取 versions.json 失败")?;
    let root: Value = serde_json::from_str(&text).context("解析 versions.json 失败")?;

    let sub = root
        .get("versions")
        .ok_or_else(|| anyhow!("JSON 中未找到 'versions' 字段"))?;

    let sub_obj = sub
        .get(mc_version)
        .ok_or_else(|| anyhow!("未找到大版本 {} 的配置", mc_version))?;

    let mut results: Vec<String> = Vec::new();

    // Release 版本: 从 artefacts 中拉取
    if let Some(artefacts_val) = sub_obj.get("artefacts") {
        if let Some(artefacts_map) = artefacts_val.as_object() {
            for (_, artifact_val) in artefacts_map {
                if let Some(item_map) = artifact_val.as_object() {
                    for (_, item_val) in item_map {
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

    // Snapshot 版本: 从 snapshots 中拉取
    if let Some(snap_val) = sub_obj.get("snapshots") {
        if let Some(snap_obj) = snap_val.as_object() {
            if let Some(lite_obj) = snap_obj.get("com.mumfrey:liteloader") {
                if let Some(lite_map) = lite_obj.as_object() {
                    for (_, v) in lite_map {
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

    results.sort_by(|a, b| b.cmp(a));
    results.dedup();
    if results.is_empty() {
        bail!("未在 {} 中找到任何可用子版本", mc_version);
    }
    Ok(results)
}

pub async fn get_liteloader_versions(mc_version: &str) -> Result<Vec<String>> {
    fetch_versions_from_json(mc_version).await
}

/// 从 library 数组的 JSON 元素中解析 LibraryDef
fn parse_library_entries(libs_arr: &[Value]) -> Vec<LibraryDef> {
    let mut result: Vec<LibraryDef> = Vec::new();
    for lib in libs_arr {
        if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
            let url = lib
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            result.push(LibraryDef {
                name: name.to_string(),
                repo_url: url,
            });
        }
    }
    result
}

/// 从 versions.json 解析 LiteLoader 的详细信息（文件、tweak 类、库依赖）
async fn fetch_liteloader_info(mc_version: &str, lite_version: &str) -> Result<LiteLoaderInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("构造 reqwest 客户端失败")?;
    let resp = client
        .get(LITELOADER_VERSIONS_URL)
        .send()
        .await
        .context("请求 LiteLoader versions.json 失败")?;
    if !resp.status().is_success() {
        bail!(
            "请求 LiteLoader versions.json 失败，状态码: {}",
            resp.status()
        );
    }
    let text = resp.text().await.context("读取 versions.json 失败")?;
    let root: Value = serde_json::from_str(&text).context("解析 versions.json 失败")?;

    let sub_obj = root
        .get("versions")
        .and_then(|v| v.get(mc_version))
        .ok_or_else(|| anyhow!("versions.json 中未找到 MC 版本 {}", mc_version))?;

    // repo URL
    let repo_url = sub_obj
        .get("repo")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("http://dl.liteloader.com/versions/")
        .to_string();

    // 先尝试在 artefacts (release) 中查找
    let mut info_obj: Option<&Value> = None;
    let mut section_key: Option<String> = None; // 记录是在 "artefacts" 还是 "snapshots" 中找到的

    if let Some(artefacts_val) = sub_obj.get("artefacts") {
        if let Some(artefacts_map) = artefacts_val.as_object() {
            for (_, artifact_val) in artefacts_map {
                if let Some(item_map) = artifact_val.as_object() {
                    for (_, item_val) in item_map {
                        if let Some(obj) = item_val.as_object() {
                            if let Some(v) = obj.get("version") {
                                if v.as_str() == Some(lite_version) {
                                    info_obj = Some(item_val);
                                    section_key = Some("artefacts".to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 再尝试在 snapshots 中查找
    if info_obj.is_none() {
        if let Some(snap_val) = sub_obj.get("snapshots") {
            if let Some(snap_obj) = snap_val.as_object() {
                if let Some(lite_obj) = snap_obj.get("com.mumfrey:liteloader") {
                    if let Some(lite_map) = lite_obj.as_object() {
                        for (_, v) in lite_map {
                            if let Some(obj) = v.as_object() {
                                if let Some(vv) = obj.get("version") {
                                    if vv.as_str() == Some(lite_version) {
                                        info_obj = Some(v);
                                        section_key = Some("snapshots".to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let info_obj = info_obj
        .ok_or_else(|| anyhow!("versions.json 中未找到 LiteLoader 版本 {}", lite_version))?;

    // 解析 tweakClass
    let tweak_class = info_obj
        .get("tweakClass")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("未找到 tweakClass"))?
        .to_string();

    // 解析 file
    let file = info_obj
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("未找到 file"))?
        .to_string();

    // 收集 libraries：先 section 级，再 artifact 级，去重
    let mut libraries: Vec<LibraryDef> = Vec::new();
    let mut seen_names: Vec<String> = Vec::new();

    // 1. section 级 libraries (snapshots.libraries 或 artefacts.libraries)
    if let Some(sec_key) = &section_key {
        if let Some(sec_val) = sub_obj.get(sec_key) {
            if let Some(libs_val) = sec_val.get("libraries") {
                if let Some(libs_arr) = libs_val.as_array() {
                    for def in parse_library_entries(libs_arr) {
                        if !seen_names.contains(&def.name) {
                            seen_names.push(def.name.clone());
                            libraries.push(def);
                        }
                    }
                }
            }
        }
    }

    // 2. artifact 级 libraries (每个具体版本内的 libraries)
    if let Some(libs_val) = info_obj.get("libraries") {
        if let Some(libs_arr) = libs_val.as_array() {
            for def in parse_library_entries(libs_arr) {
                if !seen_names.contains(&def.name) {
                    seen_names.push(def.name.clone());
                    libraries.push(def);
                }
            }
        }
    }

    println!(
        "[LiteLoader] 解析到 {} 个依赖库: {:?}",
        libraries.len(),
        libraries.iter().map(|l| l.name.clone()).collect::<Vec<_>>()
    );

    Ok(LiteLoaderInfo {
        file,
        version: lite_version.to_string(),
        tweak_class,
        libraries,
        repo_url,
    })
}

/// 拼接仓库地址与相对路径，保证两者之间恰好一个 `/`。
/// 以前直接 `trim_end_matches('/')` 后拼接，丢了分隔符，写进 version.json 的
/// 地址变成 `https://libraries.minecraft.netnet/minecraft/...`。
fn join_url(base: &str, relative: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        relative.trim_start_matches('/')
    )
}

/// 将 maven 坐标 (group:artifact:version) 解析为文件路径和 jar 名
fn parse_maven_coords(name: &str) -> Result<(String, String, String, String)> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() != 3 {
        bail!("无效的 maven 坐标: {}", name);
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1].to_string();
    let version = parts[2].to_string();
    let sub_path = format!("{}/{}/{}", group, artifact, version);
    Ok((group, artifact, version, sub_path))
}

/// 获取 snapshot jar 实际文件名（通过 maven-metadata.xml）
async fn resolve_snapshot_jar_name(
    repo_url: &str,
    group: &str,
    artifact: &str,
    version: &str,
) -> Result<Option<String>> {
    let metadata_url = format!(
        "{}/{}/{}/{}/maven-metadata.xml",
        repo_url.trim_end_matches('/'),
        group,
        artifact,
        version
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("构造 reqwest 客户端失败")?;

    println!("[LiteLoader] 解析 snapshot: {}", metadata_url);
    match client.get(&metadata_url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return Ok(None);
            }
            let text = match resp.text().await {
                Ok(t) => t,
                Err(_) => return Ok(None),
            };
            // 查找 <timestamp> 和 <buildNumber>
            let ts = text.find("<timestamp>").and_then(|i| {
                text[i + 11..]
                    .find("</timestamp>")
                    .map(|j| text[i + 11..i + 11 + j].trim().to_string())
            });
            let bn = text.find("<buildNumber>").and_then(|i| {
                text[i + 13..]
                    .find("</buildNumber>")
                    .map(|j| text[i + 13..i + 13 + j].trim().to_string())
            });

            match (ts, bn) {
                (Some(ts_val), Some(bn_val)) => {
                    let base = version.trim_end_matches("-SNAPSHOT");
                    let full_jar = format!("{}-{}-{}-{}.jar", artifact, base, ts_val, bn_val);
                    println!(
                        "[LiteLoader] snapshot 解析: timestamp={}, buildNumber={}",
                        ts_val, bn_val
                    );
                    return Ok(Some(full_jar));
                }
                _ => {}
            }
            Ok(None)
        }
        Err(_) => Ok(None),
    }
}

/// 构造 LiteLoader runtime jar 的下载 URL 列表
fn get_runtime_jar_urls(mc_version: &str, file_name: &str) -> Vec<String> {
    let mut urls = Vec::new();
    // 1. Jenkins 官方 build (支持 Range，可分块下载)
    urls.push(format!(
        "https://jenkins.liteloader.com/job/LiteLoader%20{}/lastSuccessfulBuild/artifact/build/libs/{}",
        mc_version, file_name
    ));
    // 2. dl.liteloader.com 官方 maven 路径作为 fallback
    urls.push(format!(
        "http://dl.liteloader.com/versions/com/mumfrey/liteloader/{}/{}",
        mc_version, file_name
    ));
    urls
}

/// 单个库的实际文件信息（用于生成 version.json 的 downloads.artifact）
#[derive(Debug, Clone)]
struct ResolvedLibrary {
    name: String,          // maven 坐标
    repo_url: String,      // 下载仓库 URL
    jar_file_name: String, // 实际 jar 文件名（SNAPSHOT 已解析为实际文件名）
    jar_url: String,       // 完整下载 URL
}

impl ResolvedLibrary {
    fn relative_path(&self) -> String {
        let parts: Vec<&str> = self.name.split(':').collect();
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        format!("{}/{}/{}/{}", group, artifact, version, self.jar_file_name)
    }
}

/// 解析所有依赖库的实际文件名（SNAPSHOT 版本需要动态解析）
async fn resolve_all_library_filenames(
    info: &LiteLoaderInfo,
    mc_version: &str,
) -> Result<Vec<ResolvedLibrary>> {
    let mut resolved: Vec<ResolvedLibrary> = Vec::new();

    // 1. 依赖库
    for lib in &info.libraries {
        let (group, artifact, version, _) = parse_maven_coords(&lib.name)?;

        // 确定主 repo URL
        let repo_url = if let Some(url) = &lib.repo_url {
            if url.trim().is_empty() {
                if lib.name.starts_with("net.minecraft") {
                    "https://libraries.minecraft.net/".to_string()
                } else {
                    "https://maven.aliyun.com/repository/public/".to_string()
                }
            } else {
                url.clone()
            }
        } else if lib.name.starts_with("net.minecraft") {
            "https://libraries.minecraft.net/".to_string()
        } else if lib.name.starts_with("org.spongepowered") {
            "https://repo.spongepowered.org/maven/".to_string()
        } else {
            "https://maven.aliyun.com/repository/public/".to_string()
        };

        // 如果是 SNAPSHOT 版本，解析实际文件名
        let jar_file_name = if version.contains("SNAPSHOT") {
            // 先尝试通过 maven-metadata.xml 解析
            let mut resolved_name = None;
            if lib.name.starts_with("org.spongepowered") {
                if let Ok(Some(name)) = resolve_snapshot_jar_name(
                    "https://repo.spongepowered.org/maven",
                    &group,
                    &artifact,
                    &version,
                )
                .await
                {
                    resolved_name = Some(name);
                }
            }
            // fallback: 尝试 lite 主 repo
            if resolved_name.is_none() {
                if let Ok(Some(name)) =
                    resolve_snapshot_jar_name(&info.repo_url, &group, &artifact, &version).await
                {
                    resolved_name = Some(name);
                }
            }
            // 最终 fallback: 使用一个已知的常见 SNAPSHOT 文件名
            match resolved_name {
                Some(n) => n,
                None => {
                    // 用已知的常见 fallback
                    let base = version.trim_end_matches("-SNAPSHOT");
                    if lib.name.starts_with("org.spongepowered") {
                        format!("{}-{}-20171010.121826-8.jar", artifact, base)
                    } else {
                        format!("{}-{}.jar", artifact, version)
                    }
                }
            }
        } else {
            format!("{}-{}.jar", artifact, version)
        };

        // 构造完整下载 URL（repo_url + maven 路径 + 文件名）
        let jar_url = join_url(
            &repo_url,
            &format!("{}/{}/{}/{}", group, artifact, version, jar_file_name),
        );

        resolved.push(ResolvedLibrary {
            name: lib.name.clone(),
            repo_url,
            jar_file_name,
            jar_url,
        });
    }

    // 2. LiteLoader 自身 runtime jar
    {
        let name = format!("com.mumfrey:liteloader:{}", info.version);
        let lite_repo_url = format!(
            "https://jenkins.liteloader.com/job/LiteLoader%20{}/lastSuccessfulBuild/artifact/build/libs/",
            mc_version
        );
        // LiteLoader 的 file 字段已经是实际文件名
        let jar_file_name = info.file.clone();
        let jar_url = join_url(&lite_repo_url, &jar_file_name);
        resolved.push(ResolvedLibrary {
            name,
            repo_url: lite_repo_url,
            jar_file_name,
            jar_url,
        });
    }

    println!("[LiteLoader] 已解析 {} 个库的实际文件名:", resolved.len());
    for r in &resolved {
        println!("  - {} -> {}", r.name, r.jar_file_name);
    }
    Ok(resolved)
}

/// 构造 version.json（inheritsFrom + launchwrapper + tweaker + 所有库）
/// 每个库都带 downloads.artifact.path，使用解析后的实际文件名
fn build_version_json(
    mc_version: &str,
    lite_version: &str,
    info: &LiteLoaderInfo,
    resolved_libs: &[ResolvedLibrary],
) -> Value {
    let mut libraries: Vec<Value> = Vec::new();

    for r in resolved_libs {
        let rel_path = r.relative_path();

        // 构造下载 URL 列表（fallback 多源，供下载模块使用）
        // 但 version.json 中只写主 URL 和 path
        libraries.push(json!({
            "name": r.name,
            "url": r.repo_url,
            "downloads": {
                "artifact": {
                    "path": rel_path,
                    "url": r.jar_url
                }
            }
        }));
    }

    json!({
        "id": format!("{}-{}-liteloader", mc_version, lite_version),
        "time": "2024-01-01T00:00:00+0000",
        "releaseTime": "2017-11-28T12:00:00+0000",
        "type": "release",
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "inheritsFrom": mc_version,
        "arguments": {
            "game": [
                "--tweakClass", info.tweak_class
            ],
            "jvm": []
        },
        "libraries": libraries
    })
}

/// 并发下载 LiteLoader runtime jar 和 所有依赖库
/// 使用解析后的 ResolvedLibrary 列表，确保下载文件名与 version.json 中的 path 一致
async fn download_all_libraries(
    resolved_libs: &[ResolvedLibrary],
    mc_version: &str,
    mc_dir: &Path,
) -> Result<()> {
    let mut tasks: Vec<DownloadTask> = Vec::new();

    for r in resolved_libs {
        let (_, _, _, sub_path) = parse_maven_coords(&r.name)?;
        let library_dir = mc_dir.join("libraries").join(&sub_path);

        // 为每个库构造多源 fallback URL
        let mut urls: Vec<String> = Vec::new();
        urls.push(r.jar_url.clone()); // 主 URL（解析后的准确 URL）

        // 添加 fallback 源
        let (group, artifact, version, _) = parse_maven_coords(&r.name)?;
        if r.name.starts_with("net.minecraft") {
            let jar_name = format!("{}-{}.jar", artifact, version);
            urls.push(format!(
                "https://libraries.minecraft.net/{}/{}/{}/{}",
                group, artifact, version, jar_name
            ));
            urls.push(format!(
                "https://bmclapi2.bangbang93.com/maven/{}/{}/{}/{}",
                group, artifact, version, jar_name
            ));
        } else if r.name.starts_with("org.spongepowered") && version.contains("SNAPSHOT") {
            // SNAPSHOT: 添加常见 fallback timestamps
            let base = version.trim_end_matches("-SNAPSHOT");
            let known_fallbacks = [
                ("20171010.121826", "8"),
                ("20170527.013711", "1"),
                ("20170914.181344", "1"),
            ];
            for (ts, bn) in &known_fallbacks {
                let jar = format!("{}-{}-{}-{}.jar", artifact, base, ts, bn);
                let url = format!(
                    "https://repo.spongepowered.org/maven/{}/{}/{}/{}",
                    group, artifact, version, jar
                );
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        } else if !r.name.starts_with("com.mumfrey") {
            // 标准 Maven 库
            let jar_name = format!("{}-{}.jar", artifact, version);
            urls.push(format!(
                "https://maven.aliyun.com/repository/public/{}/{}/{}/{}",
                group, artifact, version, jar_name
            ));
            urls.push(format!(
                "https://repo1.maven.org/maven2/{}/{}/{}/{}",
                group, artifact, version, jar_name
            ));
        } else {
            // LiteLoader runtime jar: 添加 Jenkins fallback
            let runtime_urls = get_runtime_jar_urls(mc_version, &r.jar_file_name);
            for u in runtime_urls {
                if !urls.contains(&u) {
                    urls.push(u);
                }
            }
        }

        println!(
            "[LiteLoader] 下载: {} -> {} ({} 个源)",
            r.name,
            r.jar_file_name,
            urls.len()
        );
        for (i, url) in urls.iter().enumerate() {
            println!("  [{}/{}] {}", i + 1, urls.len(), url);
        }
        tasks.push(DownloadTask {
            file_name: r.jar_file_name.clone(),
            target_dir: library_dir,
            urls,
            sha1: None,
        });
    }

    println!("[LiteLoader] 总共需要下载 {} 个文件", tasks.len());

    let res = concurrent_download::download_all(tasks, None).await;
    println!(
        "[LiteLoader] 库下载完成: 成功 {} 个, 失败 {} 个",
        res.success_count,
        res.failures.len()
    );
    if !res.failures.is_empty() {
        for f in &res.failures {
            println!("  - 失败: {} ({})", f.file_name, f.error);
        }
    }
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

/// 安装 LiteLoader（主函数，供 Tauri handler 调用）
/// 流程: 解析库信息 -> 解析 SNAPSHOT 实际文件名 -> 生成 version.json (含正确 path) -> 下载
pub async fn install_liteloader(
    mc_version: &str,
    lite_version: &str,
    mc_dir: &str,
    _java_path: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<f64>>,
) -> Result<String> {
    let mc_path = PathBuf::from(mc_dir);

    println!(
        "[LiteLoader] 开始安装: MC={}, LiteLoader={}",
        mc_version, lite_version
    );

    // 阶段 1: 从 versions.json 获取 LiteLoader 信息
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(0.05);
    }
    let info = fetch_liteloader_info(mc_version, lite_version)
        .await
        .with_context(|| format!("获取 LiteLoader {} 信息失败", lite_version))?;
    println!(
        "[LiteLoader] 信息: file={}, version={}, tweakClass={}",
        info.file, info.version, info.tweak_class
    );

    // 阶段 2: 解析所有库的实际文件名（重点: SNAPSHOT 版本需要解析 maven-metadata.xml）
    // 这是 PCL/HMCL 等标准启动器的做法: 先搞清楚文件到底叫什么，再写 version.json，再下载
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(0.1);
    }
    let resolved_libs = resolve_all_library_filenames(&info, mc_version)
        .await
        .context("解析 SNAPSHOT 库文件名失败")?;

    // 阶段 3: 生成版本目录 + version.json（每个库包含 downloads.artifact.path，使用解析后的文件名）
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(0.15);
    }
    let version_id = format!("{}-{}-liteloader", mc_version, lite_version);
    let versions_dir = mc_path.join("versions").join(&version_id);
    fs::create_dir_all(&versions_dir)
        .with_context(|| format!("创建版本目录失败: {:?}", versions_dir))?;

    let version_json = build_version_json(mc_version, lite_version, &info, &resolved_libs);
    let json_path = versions_dir.join(format!("{}.json", version_id));
    let json_str =
        serde_json::to_string_pretty(&version_json).context("序列化 version.json 失败")?;
    fs::write(&json_path, &json_str)
        .with_context(|| format!("写入 version.json 失败: {:?}", json_path))?;
    println!("[LiteLoader] version.json 已生成: {:?}", json_path);

    // 阶段 4: 并发下载所有 libraries（使用解析后的文件名，与 version.json 中的 path 完全一致）
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(0.2);
    }
    download_all_libraries(&resolved_libs, mc_version, &mc_path).await?;

    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send(1.0);
    }

    println!("[LiteLoader] 安装完成: {}", version_id);
    Ok(version_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_url_keeps_exactly_one_separator() {
        assert_eq!(
            join_url("https://libraries.minecraft.net/", "net/minecraft/a.jar"),
            "https://libraries.minecraft.net/net/minecraft/a.jar"
        );
        assert_eq!(
            join_url("https://libraries.minecraft.net", "net/minecraft/a.jar"),
            "https://libraries.minecraft.net/net/minecraft/a.jar"
        );
        assert_eq!(
            join_url("https://repo.example/maven//", "/org/a.jar"),
            "https://repo.example/maven/org/a.jar"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn resolved_library_urls_have_a_separator_between_repo_and_path() {
        // 本地 E2E 装出来的 json 里 url 是 https://libraries.minecraft.netnet/minecraft/...、
        // .../build/libsliteloader-1.12.2-SNAPSHOT.jar 这种少了一个斜杠的地址。
        // 这里只用非 SNAPSHOT 依赖，避免测试访问网络。
        let info = LiteLoaderInfo {
            file: "liteloader-1.12.2-SNAPSHOT.jar".to_string(),
            version: "1.12.2-SNAPSHOT".to_string(),
            tweak_class: "com.mumfrey.liteloader.launch.LiteLoaderTweaker".to_string(),
            libraries: vec![
                LibraryDef {
                    name: "net.minecraft:launchwrapper:1.12".to_string(),
                    repo_url: Some("https://libraries.minecraft.net/".to_string()),
                },
                LibraryDef {
                    name: "org.ow2.asm:asm-all:5.2".to_string(),
                    repo_url: None,
                },
            ],
            repo_url: "http://dl.liteloader.com/versions/".to_string(),
        };

        let resolved = resolve_all_library_filenames(&info, "1.12.2")
            .await
            .unwrap();
        let urls: Vec<&str> = resolved.iter().map(|r| r.jar_url.as_str()).collect();
        assert_eq!(
            urls,
            vec![
                "https://libraries.minecraft.net/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar",
                "https://maven.aliyun.com/repository/public/org/ow2/asm/asm-all/5.2/asm-all-5.2.jar",
                "https://jenkins.liteloader.com/job/LiteLoader%201.12.2/lastSuccessfulBuild/artifact/build/libs/liteloader-1.12.2-SNAPSHOT.jar",
            ]
        );

        let json = build_version_json("1.12.2", "1.12.2-SNAPSHOT", &info, &resolved);
        let libs = json["libraries"].as_array().unwrap();
        assert_eq!(libs.len(), urls.len());
        for (lib, expected) in libs.iter().zip(&urls) {
            assert_eq!(
                lib["downloads"]["artifact"]["url"].as_str(),
                Some(*expected)
            );
        }
        assert_eq!(
            libs[2]["downloads"]["artifact"]["path"].as_str(),
            Some("com/mumfrey/liteloader/1.12.2-SNAPSHOT/liteloader-1.12.2-SNAPSHOT.jar")
        );
    }
}
