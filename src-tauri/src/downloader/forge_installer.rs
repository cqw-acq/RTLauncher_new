use crate::downloader::mod_loader_installer_shared as shared;
use crate::http_client::shared_client;
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::PathBuf;

/// 比较两个由点分隔的版本标识段：纯数字段按数值比较（不受前导 0 和位数限制），
/// 纯数字段低于含字母的段，含字母的段之间按字符串比较。
fn compare_version_ident(a: &str, b: &str) -> Ordering {
    fn as_number(s: &str) -> Option<&str> {
        if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
            let trimmed = s.trim_start_matches('0');
            Some(if trimmed.is_empty() { "0" } else { trimmed })
        } else {
            None
        }
    }
    match (as_number(a), as_number(b)) {
        (Some(x), Some(y)) => x.len().cmp(&y.len()).then_with(|| x.cmp(y)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Forge / NeoForge 加载器版本号比较（升序语义，降序排列请交换参数）。
///
/// - `+` 之后的构建元数据不参与比较；
/// - 第一个 `-` 之前是主版本，按 `.` 逐段比较，缺失的段视为 0，
///   所以 `21.1.100` > `21.1.99`，`47.4.29` > `47.4.9`；
/// - 主版本相同时，正式版高于带后缀的版本（如 `21.10.0-beta`、`26.1.0.0-alpha.3`）；
/// - 两边都带后缀时按 SemVer 预发布规则逐段比较（`alpha.10` > `alpha.9`）；
/// - 以上都相等时按原字符串比较，保证是全序、排序稳定、可以直接 dedup。
pub(crate) fn compare_loader_versions(a: &str, b: &str) -> Ordering {
    fn split(v: &str) -> (Vec<&str>, Option<Vec<&str>>) {
        let without_build = v.split('+').next().unwrap_or(v);
        match without_build.split_once('-') {
            Some((main, pre)) => (main.split('.').collect(), Some(pre.split('.').collect())),
            None => (without_build.split('.').collect(), None),
        }
    }
    let (a_main, a_pre) = split(a);
    let (b_main, b_pre) = split(b);
    let len = a_main.len().max(b_main.len());
    for i in 0..len {
        let x = a_main.get(i).copied().unwrap_or("0");
        let y = b_main.get(i).copied().unwrap_or("0");
        let ord = compare_version_ident(x, y);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    let pre_ord = match (&a_pre, &b_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            let mut ord = Ordering::Equal;
            for (p, q) in x.iter().zip(y.iter()) {
                ord = compare_version_ident(p, q);
                if ord != Ordering::Equal {
                    break;
                }
            }
            if ord == Ordering::Equal {
                x.len().cmp(&y.len())
            } else {
                ord
            }
        }
    };
    pre_ord.then_with(|| a.cmp(b))
}

/// 按版本号从新到旧排序并去重。
pub(crate) fn sort_loader_versions_desc(versions: &mut Vec<String>) {
    versions.sort_by(|a, b| compare_loader_versions(b, a));
    versions.dedup();
}

/// Forge 版本列表的条目形如 `{mc}-{forge}`，去掉 MC 前缀后再按 Forge 版本号降序排列。
fn sort_forge_versions_desc(mc_version: &str, versions: &mut Vec<String>) {
    let prefix = format!("{}-", mc_version);
    let strip = |v: &str| -> String { v.strip_prefix(&prefix).unwrap_or(v).to_string() };
    versions.sort_by(|a, b| compare_loader_versions(&strip(b), &strip(a)).then_with(|| b.cmp(a)));
    versions.dedup();
}

#[derive(Debug, Deserialize, Clone)]
struct ForgeBmclEntry {
    #[serde(default)]
    version: String,
    #[serde(default)]
    build: Option<String>,
    #[serde(default)]
    mcversion: Option<String>,
}
async fn fetch_bmcl_forge_versions(mc_version: &str) -> Option<Vec<String>> {
    let client = shared_client().await;
    let url = format!(
        "https://bmclapi2.bangbang93.com/forge/{}/versions",
        mc_version
    );
    let text = client.get(&url).send().await.ok()?.text().await.ok()?;
    if let Ok(list) = serde_json::from_str::<Vec<ForgeBmclEntry>>(&text) {
        if !list.is_empty() {
            let mc_prefix = format!("{}-", mc_version);
            let mut versions: Vec<String> = list
                .into_iter()
                .filter(|e| {
                    e.mcversion
                        .as_ref()
                        .map(|v| v == mc_version)
                        // 用 "{mc}-" 前缀而不是 contains，避免 1.21.1 串到 1.21.11
                        .unwrap_or_else(|| e.version.starts_with(&mc_prefix))
                })
                .map(|e| {
                    let forge_ver = e.build.unwrap_or(e.version);
                    format!("{}-{}", mc_version, forge_ver)
                })
                .collect();
            sort_forge_versions_desc(mc_version, &mut versions);
            return Some(versions);
        }
    }
    None
}
async fn fetch_webpage_forge_versions(mc_version: &str) -> Result<Vec<String>> {
    let client = shared_client().await;
    let mut candidates: Vec<String> = Vec::new();
    candidates.push(format!(
        "https://files.minecraftforge.net/net/minecraftforge/forge/index_{}.html",
        mc_version
    ));
    let parts: Vec<&str> = mc_version.split('.').collect();
    if parts.len() > 2 {
        let short_version = format!("{}.{}", parts[0], parts[1]);
        candidates.push(format!(
            "https://files.minecraftforge.net/net/minecraftforge/forge/index_{}.html",
            short_version
        ));
    }
    let mut html: Option<(String, bool)> = None;
    for (idx, url) in candidates.iter().enumerate() {
        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let text = resp
                        .text()
                        .await
                        .with_context(|| format!("读取 Forge 页面内容失败: {}", url))?;
                    if !text.is_empty() {
                        // idx == 0 才是本 MC 版本自己的页面；回退到 index_{x.y}.html 时
                        // 页面上是别的 MC 版本，不能把其中的版本号算到当前版本头上
                        html = Some((text, idx == 0));
                        break;
                    }
                }
            }
            Err(_) => continue,
        }
    }
    let (html, is_exact_page) = match html {
        Some(h) => h,
        None => bail!("无法获取 Forge 页面（尝试了 {:?}）", candidates),
    };
    let versions = parse_forge_webpage_versions(mc_version, &html, is_exact_page)?;
    if versions.is_empty() {
        bail!("从 Forge 页面解析版本列表失败（未找到 installer 链接）");
    }
    Ok(versions)
}

/// 从 files.minecraftforge.net 的版本页面解析 `{mc}-{forge}` 列表（已按版本号降序）。
/// `is_exact_page` 为 false 时页面属于其他 MC 版本，只信任带 MC 版本号的 installer 链接。
fn parse_forge_webpage_versions(
    mc_version: &str,
    html: &str,
    is_exact_page: bool,
) -> Result<Vec<String>> {
    let mut versions: Vec<String> = Vec::new();
    let re_installer = Regex::new(&format!(
        r"forge-({}-\d+(?:\.\d+)+)-installer\.jar",
        regex::escape(mc_version)
    ))
    .context("构造 installer 正则失败")?;
    for cap in re_installer.captures_iter(html) {
        if let Some(v) = cap.get(1) {
            versions.push(v.as_str().to_string());
        }
    }
    if versions.is_empty() {
        let re_installer_loose = Regex::new(&format!(
            r"forge-({})-(\d+(?:\.\d+)+)-installer\.jar",
            regex::escape(mc_version)
        ))
        .context("构造宽松 installer 正则失败")?;
        for cap in re_installer_loose.captures_iter(html) {
            if let (Some(mc_v), Some(forge_v)) = (cap.get(1), cap.get(2)) {
                versions.push(format!("{}-{}", mc_v.as_str(), forge_v.as_str()));
            }
        }
    }
    if is_exact_page {
        let re_td_version =
            Regex::new(r#"<td[^>]*class="[^"]*download-version[^"]*"[^>]*>\s*(\d+(?:\.\d+)+)"#)
                .context("构造 td-version 正则失败")?;
        for cap in re_td_version.captures_iter(html) {
            if let Some(v) = cap.get(1) {
                versions.push(format!("{}-{}", mc_version, v.as_str()));
            }
        }
    }
    sort_forge_versions_desc(mc_version, &mut versions);
    Ok(versions)
}
async fn fetch_maven_metadata_forge_versions(mc_version: &str) -> Result<Vec<String>> {
    let client = shared_client().await;
    let url = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
    let text = client
        .get(url)
        .send()
        .await
        .context("Failed to request Forge maven-metadata.xml")?
        .text()
        .await
        .context("Failed to read maven-metadata.xml content")?;
    let versions = parse_forge_maven_metadata(mc_version, &text)?;
    if versions.is_empty() {
        bail!(
            "从 maven-metadata.xml 未找到 Minecraft {} 的 Forge 版本",
            mc_version
        );
    }
    Ok(versions)
}

/// 从 Forge 的 maven-metadata.xml 中取出指定 MC 版本的 `{mc}-{forge}` 列表（已按版本号降序）。
/// MC 版本按整段精确匹配，1.21.1 不会混入 1.21.10 / 1.21.11 的条目。
fn parse_forge_maven_metadata(mc_version: &str, xml: &str) -> Result<Vec<String>> {
    let re_version = Regex::new(r"<version>(\d+(?:\.\d+)+)-(\d+(?:\.\d+)+)</version>")
        .context("构造 maven version 正则失败")?;
    let mut versions: Vec<String> = Vec::new();
    for cap in re_version.captures_iter(xml) {
        if let (Some(mc_v), Some(forge_v)) = (cap.get(1), cap.get(2)) {
            if mc_v.as_str() == mc_version {
                versions.push(format!("{}-{}", mc_v.as_str(), forge_v.as_str()));
            }
        }
    }
    sort_forge_versions_desc(mc_version, &mut versions);
    Ok(versions)
}
pub async fn get_forge_versions(mc_version: &str) -> Result<Vec<String>> {
    let mc_version = mc_version.to_string();
    if let Some(v) = fetch_bmcl_forge_versions(&mc_version).await {
        println!("[Forge] BMCLAPI: {} 个版本", v.len());
        return Ok(v);
    }
    match fetch_maven_metadata_forge_versions(&mc_version).await {
        Ok(v) => {
            println!("[Forge] Maven: {} 个版本", v.len());
            return Ok(v);
        }
        Err(e_maven) => {
            eprintln!("[Forge] Maven 失败: {}", e_maven);
        }
    }
    let v = fetch_webpage_forge_versions(&mc_version).await?;
    println!("[Forge] 官网 HTML: {} 个版本", v.len());
    Ok(v)
}
async fn download_installer(
    mc_version: &str,
    forge_version: &str,
    mc_dir: &PathBuf,
) -> Result<PathBuf> {
    let cache_dir = mc_dir.join("cache").join("forge_installer");
    std::fs::create_dir_all(&cache_dir).ok();
    let forge_only = forge_version
        .strip_prefix(&format!("{}-", mc_version))
        .unwrap_or(forge_version)
        .to_string();
    fn mc_is_modern(mc: &str) -> bool {
        let parts: Vec<&str> = mc.split('.').collect();
        if parts.len() < 2 {
            return false;
        }
        if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            (major, minor) >= (1, 13)
        } else {
            false
        }
    }
    let is_modern = mc_is_modern(mc_version);
    let mut bmcl_versions: Vec<String> = Vec::new();
    if let Some(list) = fetch_bmcl_forge_versions(mc_version).await {
        println!("[Forge] BMCLAPI 返回 {} 个版本", list.len());
        bmcl_versions = list;
    }
    let mut matched_from_api: Vec<String> = Vec::new();
    let mc_suffix = format!("-{}", mc_version);
    for v in &bmcl_versions {
        if v.contains(&forge_only) {
            let should_keep = if is_modern {
                v.strip_prefix(mc_version)
                    .map(|rest| rest.trim_start_matches('-'))
                    .map(|rest| !rest.ends_with(&mc_suffix))
                    .unwrap_or(false)
            } else {
                true
            };
            if should_keep {
                matched_from_api.push(v.clone());
            }
        }
    }
    let fmt_mc_forge = format!("{}-{}", mc_version, forge_only);           
    let fmt_mc_forge_mc = format!("{}-{}-{}", mc_version, forge_only, mc_version); 
    let fmt_mc_forge_cap_mc = format!("{}-Forge{}-{}", mc_version, forge_only, mc_version);
    let fmt_forge_mc = format!("{}-{}", forge_only, mc_version);
    let fmt_forge_only = forge_only.clone();
    let mut version_candidates: Vec<String> = Vec::new();
    for m in &matched_from_api {
        if !version_candidates.contains(m) {
            version_candidates.push(m.clone());
        }
    }
    if is_modern {
        let guesses = vec![fmt_mc_forge.clone()];
        for g in guesses {
            if !version_candidates.contains(&g) {
                version_candidates.push(g);
            }
        }
    } else {
        let guesses = vec![
            fmt_mc_forge_mc.clone(),       
            fmt_mc_forge.clone(),          
            fmt_mc_forge_cap_mc.clone(),   
            fmt_forge_mc.clone(),          
            fmt_forge_only.clone(),        
        ];
        for g in guesses {
            if !version_candidates.contains(&g) {
                version_candidates.push(g);
            }
        }
    }
    println!(
        "[Forge] 候选版本列表 ({}): {:?}",
        version_candidates.len(),
        version_candidates
    );
    for v in &version_candidates {
        let file_name = format!("forge-{}-installer.jar", v);
        let target_path = cache_dir.join(&file_name);
        if target_path.exists() {
            println!("[Forge] 缓存命中: {}", file_name);
            return Ok(target_path);
        }
    }
    let mut candidate_urls: Vec<(String, String)> = Vec::new();
    for v in &version_candidates {
        let file_name = format!("forge-{}-installer.jar", v);
        candidate_urls.push((
            file_name.clone(),
            format!(
                "https://files.minecraftforge.net/maven/net/minecraftforge/forge/{}/forge-{}-installer.jar",
                v, v
            ),
        ));
        candidate_urls.push((
            file_name.clone(),
            format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
                v, v
            ),
        ));
    }
    let simple_name = format!("forge-{}-installer.jar", forge_version);
    candidate_urls.push((
        simple_name.clone(),
        format!(
            "https://files.minecraftforge.net/maven/net/minecraftforge/forge/{}-{}/forge-{}-{}-installer.jar",
            mc_version, forge_only, mc_version, forge_only
        ),
    ));
    candidate_urls.push((
        simple_name.clone(),
        format!(
            "https://files.forgecdn.net/files/{}/{}/forge-{}-installer.jar",
            mc_version.replace(".", ""),
            forge_only,
            forge_version
        ),
    ));
    candidate_urls.push((
        simple_name.clone(),
        format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
            forge_version, forge_version
        ),
    ));
    println!("[Forge] Attempting {} download URLs", candidate_urls.len());
    for (i, (_, url)) in candidate_urls.iter().enumerate() {
        println!("[Forge]   [{}/{}] {}", i + 1, candidate_urls.len(), url);
    }
    let client = shared_client().await;
    let mut last_error: Option<String> = None;
    for (file_name, url) in &candidate_urls {
        let target_path = cache_dir.join(&file_name);
        println!("[Forge] 正在下载: {}", url);
        match client
            .get(url)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    println!("[Forge]   <- HTTP {}, skipping", status);
                    last_error = Some(format!("HTTP {}", status));
                    continue;
                }
                match std::fs::File::create(&target_path) {
                    Ok(mut file) => {
                        use std::io::Write;
                        match resp.bytes().await {
                            Ok(bytes) => {
                                if let Err(e) = file.write_all(&bytes) {
                                    println!("[Forge]   <- Failed to write file: {}", e);
                                    last_error = Some(format!("写文件失败: {}", e));
                                    let _ = std::fs::remove_file(&target_path);
                                    continue;
                                }
                                let size = bytes.len() as u64;
                                if size == 0 {
                                    println!("[Forge]   <- File size is 0, skipping");
                                    last_error = Some("下载文件为空".to_string());
                                    let _ = std::fs::remove_file(&target_path);
                                    continue;
                                }
                                println!(
                                    "[Forge]   <- OK, {} bytes, file: {}",
                                    size,
                                    target_path.display()
                                );
                                return Ok(target_path);
                            }
                            Err(e) => {
                                println!("[Forge]   <- Failed to read response body: {}", e);
                                last_error = Some(format!("读取响应体失败: {}", e));
                                let _ = std::fs::remove_file(&target_path);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        println!("[Forge]   <- Failed to create file: {}", e);
                        last_error = Some(format!("创建文件失败: {}", e));
                        continue;
                    }
                }
            }
            Err(e) => {
                println!("[Forge]   <- Request failed: {}", e);
                last_error = Some(format!("请求失败: {}", e));
                continue;
            }
        }
    }
    println!(
        "[Forge] All download URLs failed, last error: {:?}",
        last_error
    );
    println!("[Forge] Tip: You can manually open the above links in a browser to test");
    println!(
        "[Forge]      Or place forge-*-installer.jar in the following directory to skip download:"
    );
    println!("[Forge]      {}", cache_dir.display());
    bail!(
        "下载 Forge Installer JAR 失败: {} (候选版本: {:?}, 最后错误: {})",
        simple_name,
        version_candidates,
        last_error.unwrap_or_else(|| "未知".to_string())
    )
}
pub async fn install_forge(
    mc_version: &str,
    forge_version: &str,
    mc_dir: &str,
    java_path: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<f64>>,
    wait_for_original: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<String> {
    let mc_path = PathBuf::from(mc_dir);
    let normalized_version = if forge_version.contains('-') {
        forge_version.to_string()
    } else {
        format!("{}-{}", mc_version, forge_version)
    };
    println!(
        "[Forge] 版本号归一化: {} -> {}",
        forge_version, normalized_version
    );
    let installer_jar = download_installer(mc_version, &normalized_version, &mc_path).await?;
    let java_executable = if java_path.is_empty() {
        let auto = shared::pick_java_executable(mc_version);
        println!("[Forge] 自动探测 Java: {} (java_path 为空)", auto);
        auto
    } else {
        java_path.to_string()
    };
    let cfg = shared::LoaderInstallerConfig {
        installer_jar_path: installer_jar,
        java_executable_path: PathBuf::from(java_executable),
        mc_version: mc_version.to_string(),
        mc_version_id: mc_version.to_string(),
        library_mirrors: vec![
            "https://bmclapi2.bangbang93.com/maven/".to_string(),
            "https://files.minecraftforge.net/maven/".to_string(),
            "https://libraries.minecraft.net/".to_string(),
            "https://maven.aliyun.com/repository/public/".to_string(),
            "https://repo.spongepowered.org/maven/".to_string(),
            "https://maven.fabricmc.net/".to_string(),
            "https://repo1.maven.org/maven2/".to_string(),
        ],
    };
    shared::install(&cfg, &mc_path, progress_tx, wait_for_original).await
}

#[cfg(test)]
mod tests {
    use super::{
        compare_loader_versions, parse_forge_maven_metadata, parse_forge_webpage_versions,
        sort_forge_versions_desc, sort_loader_versions_desc,
    };
    use std::cmp::Ordering;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn compares_segments_numerically() {
        assert_eq!(compare_loader_versions("21.1.100", "21.1.99"), Ordering::Greater);
        assert_eq!(compare_loader_versions("47.4.29", "47.4.9"), Ordering::Greater);
        assert_eq!(compare_loader_versions("52.1.21", "52.1.9"), Ordering::Greater);
        assert_eq!(
            compare_loader_versions("14.23.5.2864", "14.23.5.2860"),
            Ordering::Greater
        );
        assert_eq!(compare_loader_versions("21.1.9", "21.1.10"), Ordering::Less);
        assert_eq!(compare_loader_versions("21.1.99", "21.1.99"), Ordering::Equal);
        // 次版本号也按数值比较：21.10 > 21.9 > 21.1
        assert_eq!(compare_loader_versions("21.10.0", "21.9.5"), Ordering::Greater);
        assert_eq!(compare_loader_versions("26.1.0.10", "26.1.0.9"), Ordering::Greater);
        // 超出 u64 的数字段也能比较
        assert_eq!(
            compare_loader_versions("1.99999999999999999999999", "1.100000000000000000000000"),
            Ordering::Less
        );
    }

    #[test]
    fn release_ranks_above_suffixed_build() {
        assert_eq!(compare_loader_versions("21.10.0", "21.10.0-beta"), Ordering::Greater);
        assert_eq!(compare_loader_versions("20.2.3-beta", "20.2.3"), Ordering::Less);
        // 主版本号更大时后缀不影响
        assert_eq!(compare_loader_versions("20.2.10-beta", "20.2.9"), Ordering::Greater);
        assert_eq!(compare_loader_versions("20.2.10-beta", "20.2.9-beta"), Ordering::Greater);
        // 预发布标识按数值比较，构建元数据（+ 之后）不影响先后
        assert_eq!(
            compare_loader_versions("26.1.0.0-alpha.10+snapshot-6", "26.1.0.0-alpha.9+snapshot-6"),
            Ordering::Greater
        );
        assert_eq!(
            compare_loader_versions("26.1.0.0-alpha.15+pre-3", "26.1.0.0"),
            Ordering::Less
        );
        // 数字标识低于字母标识，短的低于长的
        assert_eq!(compare_loader_versions("1.0.0-1", "1.0.0-alpha"), Ordering::Less);
        assert_eq!(compare_loader_versions("1.0.0-alpha", "1.0.0-alpha.1"), Ordering::Less);
    }

    #[test]
    fn sort_desc_is_numeric_and_dedups() {
        let mut v = strings(&[
            "21.1.53", "21.1.99", "21.1.100", "21.1.267", "21.1.9", "21.1.100", "21.1.10-beta",
            "21.1.10",
        ]);
        sort_loader_versions_desc(&mut v);
        assert_eq!(
            v,
            strings(&["21.1.267", "21.1.100", "21.1.99", "21.1.53", "21.1.10", "21.1.10-beta", "21.1.9"])
        );
    }

    #[test]
    fn sorts_forge_list_with_mc_prefix() {
        let mut v = strings(&[
            "1.20.1-47.4.9",
            "1.20.1-47.4.8",
            "1.20.1-47.4.29",
            "1.20.1-47.4.10",
            "1.20.1-47.0.0",
            "1.20.1-47.4.28",
        ]);
        sort_forge_versions_desc("1.20.1", &mut v);
        assert_eq!(
            v,
            strings(&[
                "1.20.1-47.4.29",
                "1.20.1-47.4.28",
                "1.20.1-47.4.10",
                "1.20.1-47.4.9",
                "1.20.1-47.4.8",
                "1.20.1-47.0.0",
            ])
        );
    }

    #[test]
    fn maven_metadata_matches_exact_mc_and_sorts() {
        let xml = r#"<metadata><versioning><versions>
            <version>1.21.11-61.2.5</version>
            <version>1.21.10-60.1.18</version>
            <version>1.21.1-52.1.9</version>
            <version>1.21.1-52.1.21</version>
            <version>1.21.1-52.1.10</version>
            <version>1.21.1-52.0.0</version>
            <version>1.21-51.0.33</version>
            <version>1.12.2-14.23.4.2720-4627</version>
        </versions></versioning></metadata>"#;
        let v = parse_forge_maven_metadata("1.21.1", xml).unwrap();
        assert_eq!(
            v,
            strings(&["1.21.1-52.1.21", "1.21.1-52.1.10", "1.21.1-52.1.9", "1.21.1-52.0.0"])
        );
        assert_eq!(
            parse_forge_maven_metadata("1.21.11", xml).unwrap(),
            strings(&["1.21.11-61.2.5"])
        );
        assert_eq!(
            parse_forge_maven_metadata("1.21", xml).unwrap(),
            strings(&["1.21-51.0.33"])
        );
        assert!(parse_forge_maven_metadata("1.20.1", xml).unwrap().is_empty());
    }

    #[test]
    fn webpage_versions_are_sorted_and_scoped_to_mc() {
        let html = r#"
            <a href="https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.1-52.1.9/forge-1.21.1-52.1.9-installer.jar">i</a>
            <a href="https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.1-52.1.21/forge-1.21.1-52.1.21-installer.jar">i</a>
            <a href="https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.11-61.2.5/forge-1.21.11-61.2.5-installer.jar">i</a>
            <td class="download-version">52.1.20</td>
        "#;
        let exact = parse_forge_webpage_versions("1.21.1", html, true).unwrap();
        assert_eq!(
            exact,
            strings(&["1.21.1-52.1.21", "1.21.1-52.1.20", "1.21.1-52.1.9"])
        );
        // 回退页面（属于其他 MC 版本）时不信任没有 MC 前缀的表格版本号
        let fallback = parse_forge_webpage_versions("1.21.1", html, false).unwrap();
        assert_eq!(fallback, strings(&["1.21.1-52.1.21", "1.21.1-52.1.9"]));
    }
}