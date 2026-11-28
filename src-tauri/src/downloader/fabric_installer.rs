use crate::downloader::concurrent_download::{self, DownloadTask};
use crate::downloader::shared_utils::{self, Library, MetaResponse};
use crate::http_client::shared_client;
use anyhow::{anyhow, bail, Result};
use serde_json;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

/// 安装/列表默认优先走 BMCLAPI 镜像（国内更快），失败时自动回退到 Fabric 官方源
pub const PREFER_MIRROR: bool = true;

/// Fabric 的一个下载源：元数据（profile、版本列表）与库文件 Maven
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FabricSource {
    name: &'static str,
    meta_base: &'static str,
    maven_base: &'static str,
}

const MIRROR_SOURCE: FabricSource = FabricSource {
    name: "BMCLAPI 镜像",
    meta_base: "https://bmclapi2.bangbang93.com/fabric-meta",
    maven_base: "https://bmclapi2.bangbang93.com/maven",
};

const OFFICIAL_SOURCE: FabricSource = FabricSource {
    name: "Fabric 官方源",
    meta_base: "https://meta.fabricmc.net",
    maven_base: "https://maven.fabricmc.net",
};

/// 元数据请求的单次超时。镜像"能连上但不响应"时尽快切到下一个源，
/// 而不是等满共享 HTTP 客户端的 600 秒总超时。
const META_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// 按偏好排列下载源：`use_mirror` 为 true 时镜像优先、官方兜底，反之官方优先、镜像兜底
fn fabric_sources(use_mirror: bool) -> [FabricSource; 2] {
    if use_mirror {
        [MIRROR_SOURCE, OFFICIAL_SOURCE]
    } else {
        [OFFICIAL_SOURCE, MIRROR_SOURCE]
    }
}

async fn fetch_text(url: &str) -> Result<String> {
    let client = shared_client().await;
    let resp = client
        .get(url)
        .timeout(META_REQUEST_TIMEOUT)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        bail!("请求 {} 失败, HTTP 状态码: {}", url, status);
    }
    Ok(resp.text().await?)
}

/// 依次尝试各下载源，返回第一个成功的结果；全部失败时汇总每个源的错误
async fn with_source_fallback<T, F, Fut>(
    sources: &[FabricSource],
    what: &str,
    mut attempt: F,
) -> Result<T>
where
    F: FnMut(FabricSource) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut errors: Vec<(&'static str, String)> = Vec::new();
    for &source in sources {
        match attempt(source).await {
            Ok(value) => {
                if !errors.is_empty() {
                    println!("[Fabric] {}: 已回退到 {} 并成功", what, source.name);
                }
                return Ok(value);
            }
            Err(e) => {
                eprintln!("[Fabric] {}: {} 失败: {}", what, source.name, e);
                errors.push((source.name, e.to_string()));
            }
        }
    }
    Err(anyhow!(summarize_source_errors(&errors)))
}

/// 汇总多个下载源的错误。各源报的是同一个错误（例如 MC 版本本身不存在）时只报一次。
fn summarize_source_errors(errors: &[(&str, String)]) -> String {
    match errors {
        [] => "没有可用的下载源".to_string(),
        [(_, first), rest @ ..] if rest.iter().all(|(_, e)| e == first) => first.clone(),
        _ => format!(
            "所有下载源均失败（{}）",
            errors
                .iter()
                .map(|(name, e)| format!("{}: {}", name, e))
                .collect::<Vec<_>>()
                .join("；")
        ),
    }
}

/// 获取 Fabric Loader 版本列表（`use_mirror` 决定优先源，失败时回退到另一个源）
pub async fn get_fabric_loader_versions(mc_version: &str, use_mirror: bool) -> Result<Vec<String>> {
    with_source_fallback(
        &fabric_sources(use_mirror),
        "获取 Fabric Loader 版本列表",
        |source| {
            let url = format!("{}/v2/versions", source.meta_base);
            async move {
                let text = fetch_text(&url).await?;
                let meta: MetaResponse = serde_json::from_str(&text)
                    .map_err(|e| anyhow!("解析 Fabric 元数据失败 ({}): {}", url, e))?;
                shared_utils::parse_meta_versions(meta, mc_version)
            }
        },
    )
    .await
}

/// 从 Fabric API 版本号中取出它对应的 Minecraft 版本。
///
/// 版本号形如 `0.92.12+1.20.1`、`0.4.1+build.245-1.14`；
/// 早期的 `0.3.0-pre+build.135` 没有标注 MC 版本，返回 None。
fn fabric_api_target_mc(api_version: &str) -> Option<&str> {
    let (_, build_meta) = api_version.split_once('+')?;
    let mc = match build_meta.strip_prefix("build.") {
        Some(rest) => rest.split_once('-')?.1,
        None => build_meta,
    };
    if mc.is_empty() {
        None
    } else {
        Some(mc)
    }
}

/// 只保留目标 MC 版本与 `mc_version` 完全一致的 Fabric API 版本。
/// 不能用子串匹配：`1.21.1` 是 `1.21.11` 的前缀，会把别的 MC 版本的构建混进来。
fn filter_fabric_api_versions(all_versions: Vec<String>, mc_version: &str) -> Vec<String> {
    all_versions
        .into_iter()
        .filter(|v| fabric_api_target_mc(v) == Some(mc_version))
        .collect()
}

/// 获取 Fabric API 版本列表
pub async fn get_fabric_api_versions(mc_version: &str) -> Result<Vec<String>> {
    let url = "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/maven-metadata.xml";
    let client = shared_client().await;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let xml_text = resp.text().await?;

    // 使用共享的 XML 解析工具
    let all_versions = shared_utils::parse_maven_metadata(&xml_text)?;
    // 过滤出与当前 MC 版本精确匹配的 API 版本
    Ok(filter_fabric_api_versions(all_versions, mc_version))
}

#[derive(Debug, serde::Deserialize)]
struct FabricProfileJson {
    id: String,
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(default)]
    libraries: Vec<Library>,
}

/// 解析并校验 profile JSON，镜像返回错误页/残缺内容时让调用方回退到下一个源
fn parse_fabric_profile(text: &str) -> Result<FabricProfileJson> {
    let profile: FabricProfileJson =
        serde_json::from_str(text).map_err(|e| anyhow!("Fabric profile JSON 无效: {}", e))?;
    if profile.id.trim().is_empty() || profile.main_class.trim().is_empty() {
        bail!("Fabric profile JSON 缺少 id 或 mainClass");
    }
    Ok(profile)
}

/// 安装 Fabric Loader（`use_mirror` 决定优先源，失败时回退到另一个源）
pub async fn install_fabric_loader(
    mc_version: &str,
    loader_version: &str,
    mc_folder_path: &str,
    use_mirror: bool,
) -> Result<String> {
    let sources = fabric_sources(use_mirror);
    let (profile_json_text, profile) =
        with_source_fallback(&sources, "下载 Fabric Loader profile", |source| {
            let url = format!(
                "{}/v2/versions/loader/{}/{}/profile/json",
                source.meta_base, mc_version, loader_version
            );
            async move {
                let text = fetch_text(&url).await?;
                let profile = parse_fabric_profile(&text)?;
                Ok((text, profile))
            }
        })
        .await?;

    let version_id = format!("{}-{}-fabric", mc_version, loader_version);
    let versions_dir = PathBuf::from(mc_folder_path)
        .join("versions")
        .join(&version_id);
    fs::create_dir_all(&versions_dir)?;
    let profile_json_path = versions_dir.join(format!("{}.json", version_id));
    fs::write(&profile_json_path, &profile_json_text)?;

    // 使用共享的 concurrent_download 批量下载库文件（每个库都带上各源的地址互为备用）
    download_libraries(&profile.libraries, &sources, mc_folder_path).await?;

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

    println!("Fabric Loader 安装完成，版本 ID: {}", version_id);
    Ok(version_id)
}

/// 安装 Fabric API
pub async fn install_fabric_api(
    mc_version: &str,
    fabric_api_version: &str,
    mc_folder_path: &str,
) -> Result<()> {
    let fabric_api_url = format!(
        "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/{}/fabric-api-{}.jar",
        fabric_api_version, fabric_api_version
    );
    let mods_dir = PathBuf::from(mc_folder_path)
        .join("versions")
        .join(mc_version)
        .join("mods");
    fs::create_dir_all(&mods_dir)?;
    let jar_name = format!("fabric-api-{}.jar", fabric_api_version);

    // 使用共享的 concurrent_download 下载单个文件
    let task = DownloadTask {
        file_name: jar_name,
        target_dir: mods_dir,
        urls: vec![fabric_api_url],
        sha1: None,
    };
    match concurrent_download::download_file(&task, None, None).await {
        crate::downloader::modular_download::SingleDownloadResult::Success { .. } => {}
        crate::downloader::modular_download::SingleDownloadResult::Failed { error, .. } => {
            return Err(anyhow!("下载 Fabric API 失败: {}", error));
        }
    }
    println!("Fabric API 安装完成，版本: {}", fabric_api_version);
    Ok(())
}

/// 库文件的候选下载地址：profile 里声明的仓库优先，其后依次是各下载源的 Maven（去重）
fn library_download_urls(lib: &Library, sources: &[FabricSource]) -> Result<Vec<String>> {
    let url_sub_path = shared_utils::parse_library_path_for_url(&lib.name)?;
    let mut bases: Vec<&str> = Vec::new();
    let declared = lib.url.as_deref().map(|u| u.trim_end_matches('/'));
    for base in declared
        .into_iter()
        .chain(sources.iter().map(|s| s.maven_base.trim_end_matches('/')))
    {
        if !base.is_empty() && !bases.contains(&base) {
            bases.push(base);
        }
    }
    Ok(bases
        .into_iter()
        .map(|base| format!("{}/{}", base, url_sub_path))
        .collect())
}

/// 使用共享的 concurrent_download 批量下载库文件
async fn download_libraries(
    libraries: &[Library],
    sources: &[FabricSource],
    mc_folder_path: &str,
) -> Result<()> {
    let mut tasks = Vec::new();
    for lib in libraries {
        let (sub_path, jar_name) = shared_utils::parse_library_path_for_fs(&lib.name)?;
        let library_dir = PathBuf::from(mc_folder_path)
            .join("libraries")
            .join(&sub_path);

        tasks.push(DownloadTask {
            file_name: jar_name,
            target_dir: library_dir,
            urls: library_download_urls(lib, sources)?,
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

    let result = concurrent_download::download_all(tasks, None).await;
    if !result.failures.is_empty() {
        eprintln!("\n以下文件下载失败:");
        for f in &result.failures {
            eprintln!("  - {}: {}", f.file_name, f.error);
        }
        return Err(anyhow!("部分文件下载失败"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn fabric_api_filter_does_not_mix_prefix_mc_versions() {
        let all = versions(&[
            "0.141.6+1.21.11",
            "0.141.5+1.21.11",
            "0.116.7+1.21.1",
            "0.116.6+1.21.1",
            "0.102.0+1.21",
            "0.100.0+1.21.10",
        ]);
        assert_eq!(
            filter_fabric_api_versions(all.clone(), "1.21.1"),
            versions(&["0.116.7+1.21.1", "0.116.6+1.21.1"])
        );
        assert_eq!(
            filter_fabric_api_versions(all.clone(), "1.21.11"),
            versions(&["0.141.6+1.21.11", "0.141.5+1.21.11"])
        );
        assert_eq!(
            filter_fabric_api_versions(all, "1.21"),
            versions(&["0.102.0+1.21"])
        );
    }

    #[test]
    fn fabric_api_filter_handles_legacy_build_suffix() {
        let all = versions(&[
            "0.3.0-pre+build.135",
            "0.4.1+build.245-1.14",
            "0.5.1+build.294-1.15",
            "0.14.0+build.371-1.16.1",
            "0.13.1+build.370-1.16",
            "0.28.5+1.14",
        ]);
        assert_eq!(
            filter_fabric_api_versions(all.clone(), "1.14"),
            versions(&["0.4.1+build.245-1.14", "0.28.5+1.14"])
        );
        assert_eq!(
            filter_fabric_api_versions(all.clone(), "1.16"),
            versions(&["0.13.1+build.370-1.16"])
        );
        assert_eq!(
            filter_fabric_api_versions(all, "1.16.1"),
            versions(&["0.14.0+build.371-1.16.1"])
        );
    }

    #[test]
    fn fabric_api_filter_excludes_experimental_variants() {
        let all = versions(&[
            "0.44.0+1.18",
            "0.42.9+1.18_experimental",
            "0.140.0+1.21.11_unobfuscated",
            "0.140.1+1.21.11",
        ]);
        assert_eq!(
            filter_fabric_api_versions(all.clone(), "1.18"),
            versions(&["0.44.0+1.18"])
        );
        assert_eq!(
            filter_fabric_api_versions(all, "1.21.11"),
            versions(&["0.140.1+1.21.11"])
        );
    }

    #[test]
    fn fabric_api_target_mc_parses_known_formats() {
        assert_eq!(fabric_api_target_mc("0.92.12+1.20.1"), Some("1.20.1"));
        assert_eq!(fabric_api_target_mc("0.4.1+build.245-1.14"), Some("1.14"));
        assert_eq!(
            fabric_api_target_mc("0.1.0+build.1-20w14infinite"),
            Some("20w14infinite")
        );
        assert_eq!(fabric_api_target_mc("0.3.0-pre+build.135"), None);
        assert_eq!(fabric_api_target_mc("0.1.0"), None);
        assert_eq!(fabric_api_target_mc("0.1.0+"), None);
    }

    #[test]
    fn sources_follow_preference_and_include_the_other_as_fallback() {
        assert_eq!(fabric_sources(true), [MIRROR_SOURCE, OFFICIAL_SOURCE]);
        assert_eq!(fabric_sources(false), [OFFICIAL_SOURCE, MIRROR_SOURCE]);
    }

    fn lib(name: &str, url: Option<&str>) -> Library {
        Library {
            name: name.to_string(),
            url: url.map(|u| u.to_string()),
            downloads: None,
        }
    }

    #[test]
    fn library_urls_keep_declared_repo_first_and_add_other_mavens() {
        let sources = fabric_sources(true);
        let urls = library_download_urls(
            &lib("net.fabricmc:fabric-loader:0.16.10", Some("https://maven.fabricmc.net/")),
            &sources,
        )
        .unwrap();
        assert_eq!(
            urls,
            vec![
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.10/fabric-loader-0.16.10.jar",
                "https://bmclapi2.bangbang93.com/maven/net/fabricmc/fabric-loader/0.16.10/fabric-loader-0.16.10.jar",
            ]
        );

        let urls = library_download_urls(&lib("org.ow2.asm:asm:9.7.1", None), &sources).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://bmclapi2.bangbang93.com/maven/org/ow2/asm/asm/9.7.1/asm-9.7.1.jar",
                "https://maven.fabricmc.net/org/ow2/asm/asm/9.7.1/asm-9.7.1.jar",
            ]
        );
    }

    #[tokio::test]
    async fn fallback_uses_next_source_when_first_fails() {
        let sources = fabric_sources(true);
        let mut tried = Vec::new();
        let result = with_source_fallback(&sources, "测试", |source| {
            tried.push(source.name);
            async move {
                if source == MIRROR_SOURCE {
                    Err(anyhow!("error trying to connect: operation timed out"))
                } else {
                    Ok(source.meta_base)
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(result, OFFICIAL_SOURCE.meta_base);
        assert_eq!(tried, vec![MIRROR_SOURCE.name, OFFICIAL_SOURCE.name]);
    }

    #[tokio::test]
    async fn fallback_stops_at_first_success() {
        let sources = fabric_sources(true);
        let mut tried = Vec::new();
        let result = with_source_fallback(&sources, "测试", |source| {
            tried.push(source.name);
            async move { Ok::<_, anyhow::Error>(source.name) }
        })
        .await
        .unwrap();
        assert_eq!(result, MIRROR_SOURCE.name);
        assert_eq!(tried, vec![MIRROR_SOURCE.name]);
    }

    #[tokio::test]
    async fn fallback_reports_every_source_when_all_fail() {
        let sources = fabric_sources(true);
        let err = with_source_fallback(&sources, "测试", |source| async move {
            Err::<(), _>(anyhow!("{} 挂了", source.name))
        })
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("BMCLAPI 镜像: BMCLAPI 镜像 挂了"), "{}", err);
        assert!(err.contains("Fabric 官方源: Fabric 官方源 挂了"), "{}", err);
    }

    #[test]
    fn identical_errors_are_reported_once() {
        let msg = "MC版本 1.12.2 不存在于元数据中".to_string();
        let errors = vec![("BMCLAPI 镜像", msg.clone()), ("Fabric 官方源", msg.clone())];
        assert_eq!(summarize_source_errors(&errors), msg);
    }

    #[test]
    fn profile_validation_rejects_error_payloads() {
        assert!(parse_fabric_profile(
            r#"{"$isServiceError":true,"code":"COMMON_NO_SUCH_OBJECT","message":"未找到fabric meta"}"#
        )
        .is_err());
        assert!(parse_fabric_profile("<html>502 Bad Gateway</html>").is_err());
        let profile = parse_fabric_profile(
            r#"{"id":"fabric-loader-0.16.10-1.21.1","inheritsFrom":"1.21.1","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.10","url":"https://maven.fabricmc.net/"}]}"#,
        )
        .unwrap();
        assert_eq!(profile.libraries.len(), 1);
    }
}
