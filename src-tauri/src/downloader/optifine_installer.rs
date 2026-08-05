use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

use crate::downloader::concurrent_download;
use crate::downloader::shared_utils::{
    parse_library_path_for_fs, sanitize_instance_name,
};
use crate::http_client::shared_client;

const BMCL_API_ROOT: &str = "https://bmclapi2.bangbang93.com";

const VANILLA_MAIN: &str = "net.minecraft.client.main.Main";
const LAUNCH_WRAPPER_MAIN: &str = "net.minecraft.launchwrapper.Launch";
const MOD_LAUNCHER_MAIN: &str = "cpw.mods.modlauncher.Launcher";
const BOOTSTRAP_LAUNCHER_MAIN: &str = "cpw.mods.bootstraplauncher.BootstrapLauncher";
const FORGE_BOOTSTRAP_MAIN: &str = "net.minecraftforge.bootstrap.ForgeBootstrap";
const NEO_FORGE_BOOTSTRAP_MAIN: &str = "net.neoforged.fml.startup.Client";

const FORGE_OPTIFINE_COMPATIBLE_MAINS: &[&str] = &[
    VANILLA_MAIN,
    LAUNCH_WRAPPER_MAIN,
    MOD_LAUNCHER_MAIN,
    BOOTSTRAP_LAUNCHER_MAIN,
    FORGE_BOOTSTRAP_MAIN,
    NEO_FORGE_BOOTSTRAP_MAIN,
];

#[derive(Debug, Deserialize, Clone)]
struct OptiFineBmclEntry {
    #[serde(default)]
    pub dl: String,
    #[serde(default)]
    pub ver: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub mirror: String,
    #[serde(default)]
    pub mcversion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptiFineVersionInfo {
    pub game_version: String,
    pub self_version: String,
    pub full_version: String,
    pub download_url: String,
    pub official_url: String,
    pub is_pre: bool,
}

fn lookup_version_normalize(version: &str) -> String {
    match version {
        "1.8" => "1.8.0".to_string(),
        "1.9" => "1.9.0".to_string(),
        v => v.to_string(),
    }
}

fn lookup_version_denormalize(version: &str) -> String {
    match version {
        "1.8.0" => "1.8".to_string(),
        "1.9.0" => "1.9".to_string(),
        v => v.to_string(),
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<i32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i32>().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}

pub async fn get_optifine_versions(mc_version: &str) -> Result<Vec<OptiFineVersionInfo>> {
    let client = shared_client().await;
    let url = format!("{}/optifine/versionlist", BMCL_API_ROOT);

    let text = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("请求 BMCLAPI OptiFine 版本列表失败: {}", url))?
        .text()
        .await
        .context("读取 BMCLAPI OptiFine 版本列表失败")?;

    let list: Vec<OptiFineBmclEntry> = serde_json::from_str(&text)
        .with_context(|| format!("解析 BMCLAPI OptiFine 版本列表失败:\n{}", text))?;

    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    let denorm_mc = lookup_version_denormalize(mc_version);

    for entry in list {
        if entry.mcversion.is_empty() {
            continue;
        }
        let entry_game = lookup_version_normalize(&entry.mcversion);
        if entry_game != denorm_mc && entry.mcversion != mc_version {
            continue;
        }

        let self_version = format!("{}_{}", entry.type_, entry.patch);
        let mirror_url = format!(
            "{}/optifine/{}/{}/{}",
            BMCL_API_ROOT,
            lookup_version_normalize(&entry.mcversion),
            entry.type_,
            entry.patch
        );

        if !seen.insert(mirror_url.clone()) {
            continue;
        }

        let is_pre = entry.patch.starts_with("pre") || entry.patch.starts_with("alpha");
        let game_version_norm = lookup_version_normalize(&entry.mcversion);
        let full = format!("{}_{}", game_version_norm, self_version);

        let official_url = if entry.dl.is_empty() {
            mirror_url.clone()
        } else {
            entry.dl.clone()
        };

        results.push(OptiFineVersionInfo {
            game_version: game_version_norm,
            self_version: self_version.clone(),
            full_version: full,
            download_url: mirror_url,
            official_url,
            is_pre,
        });
    }

    results.sort_by(|a, b| compare_versions(&b.self_version, &a.self_version));
    Ok(results)
}

fn derive_filename_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if let Some(idx) = segments.iter().position(|s| *s == "optifine") {
        let after = &segments[idx + 1..];
        if after.len() >= 3 {
            return format!(
                "OptiFine_{}_{}_{}.jar",
                after[0], after[1], after[2]
            );
        }
    }

    if let Some(last) = segments.last() {
        if !last.is_empty() && last.contains('.') {
            return last.to_string();
        }
    }

    if let Some(query_part) = url.split('?').nth(1) {
        for param in query_part.split('&') {
            if let Some(val) = param.strip_prefix("f=") {
                let decoded = val.replace("%2F", "/").replace("%20", " ");
                if !decoded.is_empty() {
                    return decoded;
                }
            }
        }
    }

    format!("optifine-installer.jar")
}

pub async fn download_optifine_installer(
    download_url: &str,
    mc_dir: &Path,
) -> Result<PathBuf> {
    download_optifine_installer_with_fallback(download_url, download_url, mc_dir).await
}

pub async fn download_optifine_installer_with_fallback(
    download_url: &str,
    fallback_url: &str,
    mc_dir: &Path,
) -> Result<PathBuf> {
    let cache_dir = mc_dir.join("cache").join("optifine_installer");
    fs::create_dir_all(&cache_dir).ok();

    let filename = derive_filename_from_url(download_url);
    let target = cache_dir.join(&filename);

    if target.exists() && fs::metadata(&target).map(|m| m.len() > 100_000).unwrap_or(false) {
        println!("[OptiFine] 使用缓存安装器: {}", target.display());
        return Ok(target);
    }

    let mut urls = Vec::new();
    if download_url != fallback_url {
        urls.push(download_url.to_string());
        urls.push(fallback_url.to_string());
    } else {
        urls.push(download_url.to_string());
    }

    println!(
        "[OptiFine] 下载安装器: {} (备用源: {})",
        download_url,
        if download_url != fallback_url { fallback_url } else { "无" }
    );

    let target_dir = target.parent().map(|p| p.to_path_buf())
        .unwrap_or_else(|| cache_dir.clone());

    let task = concurrent_download::DownloadTask {
        file_name: filename,
        target_dir,
        urls,
        sha1: None,
    };

    let result = concurrent_download::download_one(task).await
        .with_context(|| format!("下载 OptiFine 安装器失败: {} (备用源: {})", download_url, fallback_url))?;

    Ok(result)
}

fn get_library_file(mc_dir: &Path, library_name: &str) -> Result<PathBuf> {
    let (parent_dir, jar_name) = parse_library_path_for_fs(library_name)?;
    let lib_dir = mc_dir.join("libraries").join(parent_dir);
    fs::create_dir_all(&lib_dir).ok();
    Ok(lib_dir.join(jar_name))
}

fn zip_entry_exists<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, path: &str) -> bool {
    for i in 0..zip.len() {
        if let Ok(file) = zip.by_index(i) {
            let name = file.name().trim_start_matches('/');
            if name == path.trim_start_matches('/') {
                return true;
            }
        }
    }
    false
}

fn read_zip_entry_to_vec<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    path: &str,
) -> Option<Vec<u8>> {
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).ok()?;
        let name = file.name().trim_start_matches('/').to_string();
        if name == path.trim_start_matches('/') {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

fn extract_zip_entry_to_file<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    entry_path: &str,
    dest: &Path,
) -> Result<bool> {
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).with_context(|| format!("读取 zip 索引 {} 失败", i))?;
        let name = file.name().trim_start_matches('/').to_string();
        if name == entry_path.trim_start_matches('/') {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut out = fs::File::create(dest)
                .with_context(|| format!("创建文件失败: {}", dest.display()))?;
            std::io::copy(&mut file, &mut out).ok();
            return Ok(true);
        }
    }
    Ok(false)
}

fn remove_mods_toml_from_jar(jar_path: &Path) -> Result<()> {
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let bytes = fs::read(jar_path)
        .with_context(|| format!("读取 jar 失败: {}", jar_path.display()))?;
    let cursor = Cursor::new(bytes);
    let mut src_zip = ZipArchive::new(cursor).context("解析 jar 失败")?;

    let tmp_path = jar_path.with_extension("jar.tmp");
    let tmp_file = fs::File::create(&tmp_path)
        .with_context(|| format!("创建临时 jar 失败: {}", tmp_path.display()))?;
    let mut writer = ZipWriter::new(tmp_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..src_zip.len() {
        let mut file = src_zip.by_index(i).with_context(|| format!("读取 zip 项 {} 失败", i))?;
        let name = file.name().to_string();
        let trimmed = name.trim_start_matches('/');
        if trimmed.eq_ignore_ascii_case("META-INF/mods.toml") {
            continue;
        }
        writer.start_file(name, options).ok();
        std::io::copy(&mut file, &mut writer).ok();
    }
    writer.finish().ok();
    drop(src_zip);

    fs::rename(&tmp_path, jar_path)
        .with_context(|| format!("重命名临时 jar 失败: {} -> {}", tmp_path.display(), jar_path.display()))?;
    Ok(())
}

fn read_buildof(installer_zip: &mut ZipArchive<Cursor<Vec<u8>>>) -> Option<String> {
    let bytes = read_zip_entry_to_vec(installer_zip, "buildof.txt")?;
    String::from_utf8(bytes).ok().map(|s| s.trim().to_string())
}

fn parse_optifine_version_from_url(url: &str) -> Result<(String, String)> {
    let path = url.split('?').next().ok_or_else(|| anyhow!("无效的下载链接"))?;
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let optifine_idx = segments
        .iter()
        .position(|s| *s == "optifine")
        .ok_or_else(|| anyhow!("URL 中找不到 optifine 段: {}", url))?;

    let after_optifine = &segments[optifine_idx + 1..];
    if after_optifine.len() < 3 {
        bail!("URL 路径段数不足，无法解析版本: {}", url);
    }

    let _mcversion = after_optifine[0].to_string();
    let type_ = after_optifine[1].to_string();
    let patch = after_optifine[2].to_string();

    let self_version = format!("{}_{}", type_, patch);
    Ok((self_version, _mcversion))
}

fn parse_optifine_version_from_installer(
    installer_bytes: &[u8],
) -> Result<(String, String, String)> {
    let cursor = Cursor::new(installer_bytes.to_vec());
    let mut zip = ZipArchive::new(cursor).context("解析 OptiFine installer jar 失败")?;

    if let Some(buildof) = read_buildof(&mut zip) {
        println!("[OptiFine] 从 buildof.txt 解析版本: {}", buildof);
        if let Some(result) = try_parse_buildof_version(&buildof) {
            return Ok(result);
        }
    }

    let config_paths = [
        "Config.class",
        "net/optifine/Config.class",
        "notch/net/optifine/Config.class",
    ];
    let mut config_bytes = None;
    for p in &config_paths {
        if let Some(b) = read_zip_entry_to_vec(&mut zip, p) {
            config_bytes = Some(b);
            break;
        }
    }
    let config_bytes = config_bytes
        .ok_or_else(|| anyhow!("无法在安装器中找到 Config.class，文件可能损坏"))?;

    match parse_version_from_config_class(&config_bytes) {
        Ok(result) => Ok(result),
        Err(e) => {
            println!(
                "[OptiFine] Config.class 解析失败: {}，尝试从安装器 JAR 文件名推断版本",
                e
            );
            Err(anyhow!(
                "无法从安装器中解析版本信息，请确认安装器文件完整且版本受支持"
            ))
        }
    }
}

fn try_parse_buildof_version(buildof: &str) -> Option<(String, String, String)> {
    let mut mc_version: Option<String> = None;
    let mut of_edition: Option<String> = None;
    let mut of_release: Option<String> = None;

    for line in buildof.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(val) = line.strip_prefix("MC_VERSION=") {
            mc_version = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("MC_VERSION:") {
            mc_version = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_EDITION=") {
            of_edition = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_EDITION:") {
            of_edition = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_RELEASE=") {
            of_release = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("OF_RELEASE:") {
            of_release = Some(val.trim().to_string());
        }
    }

    match (mc_version, of_edition, of_release) {
        (Some(mc), Some(ed), Some(rel)) => {
            let self_version = format!("{}_{}", ed, rel);
            let full_version = format!("{}_{}", mc, self_version);
            Some((mc, self_version, full_version))
        }
        _ => None,
    }
}

fn parse_version_from_config_class(config_bytes: &[u8]) -> Result<(String, String, String)> {
    let mut utf8_strings: Vec<String> = Vec::new();
    let mut i = 0;
    while i + 3 <= config_bytes.len() {
        if config_bytes[i] == 0x01 {
            let length =
                ((config_bytes[i + 1] as usize) << 8) | (config_bytes[i + 2] as usize);
            let start = i + 3;
            if start + length <= config_bytes.len() {
                if let Ok(s) = std::str::from_utf8(&config_bytes[start..start + length]) {
                    utf8_strings.push(s.to_string());
                }
            }
            i = start + length;
        } else {
            i += 1;
        }
    }

    let get_next = |needle: &str| -> Option<String> {
        let idx = utf8_strings.iter().position(|s| s == needle)?;
        utf8_strings.get(idx + 1).cloned()
    };

    let mc_version = get_next("MC_VERSION")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 MC_VERSION"))?;
    let of_edition = get_next("OF_EDITION")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 OF_EDITION"))?;
    let of_release = get_next("OF_RELEASE")
        .ok_or_else(|| anyhow!("无法从 Config.class 中解析 OF_RELEASE"))?;

    let self_version = format!("{}_{}", of_edition, of_release);
    let full_version = format!("{}_{}", mc_version, self_version);
    Ok((mc_version, self_version, full_version))
}

struct ExtraLibraries {
    libraries: Vec<Value>,
    version_folder_name: String,
    main_class: String,
    game_args_tweaks: Vec<String>,
}

fn run_patcher(
    installer_path: &Path,
    minecraft_jar: &Path,
    output_optifine_lib: &Path,
) -> Result<()> {
    println!("[OptiFine] 调用 optifine.Patcher 修补原版 jar");
    let status = Command::new("java")
        .arg("-cp")
        .arg(installer_path)
        .arg("optifine.Patcher")
        .arg(minecraft_jar)
        .arg(installer_path)
        .arg(output_optifine_lib)
        .status()
        .context("启动 java (optifine.Patcher) 失败，请确认已安装 Java")?;

    if !status.success() {
        bail!(
            "optifine.Patcher 执行失败，退出码: {:?}。\n\
             可能的原因: Java 版本不兼容，或 Minecraft jar 不完整。",
            status.code()
        );
    }
    Ok(())
}

fn process_installer_and_build_json(
    installer_path: &Path,
    mc_dir: &Path,
    mc_version: &str,
    optifine_full_ver: &str,
    optifine_self_ver: &str,
) -> Result<ExtraLibraries> {
    let installer_bytes = fs::read(installer_path)
        .with_context(|| format!("读取 OptiFine 安装器失败: {}", installer_path.display()))?;
    let cursor = Cursor::new(installer_bytes.clone());
    let mut installer_zip = ZipArchive::new(cursor).context("解析 OptiFine installer zip 失败")?;

    let maven_version = optifine_full_ver;
    let optifine_lib_name = format!("optifine:OptiFine:{}", maven_version);
    let optifine_installer_lib_name = format!("optifine:OptiFine:{}:installer", maven_version);

    let optifine_lib_path = get_library_file(mc_dir, &optifine_lib_name)?;
    let optifine_installer_lib_path = get_library_file(mc_dir, &optifine_installer_lib_name)?;

    if let Some(parent) = optifine_installer_lib_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::copy(installer_path, &optifine_installer_lib_path)
        .with_context(|| format!("复制 installer 到 libraries 失败: {}", optifine_installer_lib_path.display()))?;
    let _ = remove_mods_toml_from_jar(&optifine_installer_lib_path);

    let has_patcher = zip_entry_exists(&mut installer_zip, "optifine/Patcher.class");
    if has_patcher {
        let versions_root = mc_dir.join("versions");
        let vanilla_jar = versions_root.join(mc_version).join(format!("{}.jar", mc_version));
        if !vanilla_jar.exists() {
            bail!(
                "找不到原版 Minecraft jar: {}\n请先安装原版 Minecraft {}",
                vanilla_jar.display(),
                mc_version
            );
        }
        if let Some(parent) = optifine_lib_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        run_patcher(installer_path, &vanilla_jar, &optifine_lib_path)?;
    } else {
        if let Some(parent) = optifine_lib_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::copy(installer_path, &optifine_lib_path)
            .with_context(|| format!("复制 OptiFine 到 libraries 失败: {}", optifine_lib_path.display()))?;
    }
    let _ = remove_mods_toml_from_jar(&optifine_lib_path);

    let mut libraries: Vec<Value> = Vec::new();
    let mut has_launch_wrapper = false;

    if zip_entry_exists(&mut installer_zip, "launchwrapper-2.0.jar") {
        let lw_lib_name = "optifine:launchwrapper:2.0".to_string();
        let lw_path = get_library_file(mc_dir, &lw_lib_name)?;
        if extract_zip_entry_to_file(&mut installer_zip, "launchwrapper-2.0.jar", &lw_path)? {
            libraries.push(json!({ "name": lw_lib_name }));
            has_launch_wrapper = true;
        }
    }

    if !has_launch_wrapper {
        if let Some(launchwrapper_of_bytes) = read_zip_entry_to_vec(&mut installer_zip, "launchwrapper-of.txt") {
            if let Ok(ver) = String::from_utf8(launchwrapper_of_bytes) {
                let ver = ver.trim().to_string();
                let entry_name = format!("launchwrapper-of-{}.jar", ver);
                let lw_lib_name = format!("optifine:launchwrapper-of:{}", ver);
                let lw_path = get_library_file(mc_dir, &lw_lib_name)?;
                if extract_zip_entry_to_file(&mut installer_zip, &entry_name, &lw_path)? {
                    libraries.push(json!({ "name": lw_lib_name }));
                    has_launch_wrapper = true;
                }
            }
        }
    }

    if !has_launch_wrapper {
        libraries.push(json!({ "name": "net.minecraft:launchwrapper:1.12" }));
    }

    libraries.insert(0, json!({ "name": optifine_lib_name }));

    let buildof = read_buildof(&mut installer_zip);
    if let Some(buildof_str) = buildof.as_ref() {
        println!("[OptiFine] buildof: {}", buildof_str);
    }

    let mc_ver_parts: Vec<&str> = mc_version.split('.').collect();
    let mc_major: i32 = mc_ver_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let (main_class, game_args_tweaks) = if mc_major >= 17 {
        let vanilla_json_path = mc_dir
            .join("versions")
            .join(mc_version)
            .join(format!("{}.json", mc_version));
        let original_main = if let Ok(text) = fs::read_to_string(&vanilla_json_path) {
            serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("mainClass")?.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| VANILLA_MAIN.to_string())
        } else {
            VANILLA_MAIN.to_string()
        };

        if FORGE_OPTIFINE_COMPATIBLE_MAINS.contains(&original_main.as_str())
            && (original_main == MOD_LAUNCHER_MAIN
                || original_main == BOOTSTRAP_LAUNCHER_MAIN
                || original_main == FORGE_BOOTSTRAP_MAIN
                || original_main == NEO_FORGE_BOOTSTRAP_MAIN)
        {
            println!("[OptiFine] Forge/NeoForge 检测到，OptiFine 将作为 mod 加载（不替换 mainClass）");
            (original_main, Vec::new())
        } else {
            (LAUNCH_WRAPPER_MAIN.to_string(), vec!["--tweakClass".to_string(), "optifine.OptiFineTweaker".to_string()])
        }
    } else {
        (LAUNCH_WRAPPER_MAIN.to_string(), vec!["--tweakClass".to_string(), "optifine.OptiFineTweaker".to_string()])
    };

    Ok(ExtraLibraries {
        libraries,
        version_folder_name: format!("{}-OptiFine-{}", mc_version, optifine_self_ver),
        main_class,
        game_args_tweaks,
    })
}

fn build_optifine_version_json(
    mc_dir: &Path,
    mc_version: &str,
    extra: &ExtraLibraries,
    optifine_self_ver: &str,
    optifine_full_ver: &str,
) -> Result<PathBuf> {
    let versions_root = mc_dir.join("versions");
    let vanilla_json_path = versions_root
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    if !vanilla_json_path.exists() {
        bail!("找不到原版 version.json: {:?}", vanilla_json_path);
    }

    let vanilla_text = fs::read_to_string(&vanilla_json_path)?;
    let mut vanilla: Value = serde_json::from_str(&vanilla_text)
        .context("解析原版 version.json 失败")?;

    let instance_id = extra.version_folder_name.clone();

    let obj = vanilla
        .as_object_mut()
        .ok_or_else(|| anyhow!("原版 version.json 不是对象"))?;

    obj.insert("id".to_string(), Value::String(instance_id.clone()));
    obj.insert("inheritsFrom".to_string(), Value::String(mc_version.to_string()));
    obj.insert("mainClass".to_string(), Value::String(extra.main_class.clone()));
    obj.insert("releaseTime".to_string(), Value::String(iso_now()));
    obj.insert("time".to_string(), Value::String(iso_now()));
    obj.insert("type".to_string(), Value::String("release".to_string()));

    if !extra.game_args_tweaks.is_empty() {
        let args_obj = obj
            .get_mut("arguments")
            .and_then(|v| v.as_object_mut());
        if let Some(args) = args_obj {
            let game_arr = args
                .get_mut("game")
                .and_then(|v| v.as_array_mut());
            if let Some(game) = game_arr {
                let mut tweaks: Vec<Value> = extra
                    .game_args_tweaks
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                game.append(&mut tweaks);
            } else {
                args.insert(
                    "game".to_string(),
                    Value::Array(
                        extra
                            .game_args_tweaks
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
        } else {
            let existing = obj
                .get("minecraftArguments")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let appended = format!(
                "{} {} {}",
                existing,
                extra.game_args_tweaks[0],
                extra.game_args_tweaks[1]
            );
            obj.insert("minecraftArguments".to_string(), Value::String(appended));
        }
    }

    let existing_libs = obj
        .get("libraries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut merged = extra.libraries.clone();
    merged.extend(existing_libs);
    obj.insert("libraries".to_string(), Value::Array(merged));

    let target_dir = versions_root.join(&instance_id);
    fs::create_dir_all(&target_dir).ok();

    let target_json = target_dir.join(format!("{}.json", instance_id));
    fs::write(
        &target_json,
        serde_json::to_string_pretty(&vanilla)?,
    )
    .with_context(|| format!("写入 OptiFine version.json 失败: {}", target_json.display()))?;

    println!(
        "[OptiFine] 版本 JSON 已生成: {} (OptiFine {})",
        target_json.display(),
        optifine_full_ver
    );
    let _ = optifine_self_ver;

    if let Err(e) = ensure_options_lang(&target_dir) {
        println!("[OptiFine] 警告: 创建/更新 options.txt 失败: {}", e);
    } else {
        println!("[OptiFine] options.txt 已更新");
    }

    Ok(target_dir)
}

fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let years = 1970 + secs / 31536000;
    format!("{}-06-15T00:00:00+00:00", years)
}

fn ensure_options_lang(versions_dir: &Path) -> Result<()> {
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
    Ok(())
}

pub async fn install_optifine_from_bmcl(
    mc_version: &str,
    optifine_download_url: &str,
    mc_dir: &Path,
) -> Result<String> {
    install_optifine_from_bmcl_with_fallback(
        mc_version,
        optifine_download_url,
        optifine_download_url,
        mc_dir,
    )
    .await
}

pub async fn install_optifine_from_bmcl_with_fallback(
    mc_version: &str,
    optifine_download_url: &str,
    optifine_fallback_url: &str,
    mc_dir: &Path,
) -> Result<String> {
    let installer_path =
        download_optifine_installer_with_fallback(optifine_download_url, optifine_fallback_url, mc_dir)
            .await?;

    let (self_ver, _url_mc_ver) = parse_optifine_version_from_url(optifine_download_url)?;
    let full_ver = format!("{}_{}", mc_version, self_ver);

    println!(
        "[OptiFine] 从 URL 解析版本: MC={}, self={}, full={}",
        mc_version, self_ver, full_ver
    );

    let extra = process_installer_and_build_json(
        &installer_path,
        mc_dir,
        mc_version,
        &full_ver,
        &self_ver,
    )?;

    let _ = build_optifine_version_json(
        mc_dir,
        mc_version,
        &extra,
        &self_ver,
        &full_ver,
    )?;

    Ok(extra.version_folder_name)
}

pub async fn install_optifine_from_local(
    installer_path: &Path,
    mc_dir: &Path,
    mc_version: &str,
) -> Result<String> {
    if !installer_path.exists() {
        bail!("找不到本地 OptiFine 安装器: {}", installer_path.display());
    }
    let installer_bytes = fs::read(installer_path)?;
    let (parsed_mc, self_ver, full_ver) =
        parse_optifine_version_from_installer(&installer_bytes)?;

    if parsed_mc != mc_version {
        bail!(
            "OptiFine 安装器对应的 Minecraft 版本为 {}，期望版本 {}",
            parsed_mc,
            mc_version
        );
    }

    println!(
        "[OptiFine] 本地安装器版本: MC={}, self={}, full={}",
        parsed_mc, self_ver, full_ver
    );

    let extra = process_installer_and_build_json(
        installer_path,
        mc_dir,
        mc_version,
        &full_ver,
        &self_ver,
    )?;

    let _ = build_optifine_version_json(
        mc_dir,
        mc_version,
        &extra,
        &self_ver,
        &full_ver,
    )?;

    Ok(extra.version_folder_name)
}

pub fn create_instance_from_optifine_version(
    mc_dir: &Path,
    mc_version: &str,
    optifine_loader_version: &str,
    instance_name: &str,
) -> Result<()> {
    use crate::downloader::shared_utils::merge_version_jsons_to_instance;

    let clean = sanitize_instance_name(instance_name);
    let default = format!("{}-optifine-{}", mc_version, optifine_loader_version);
    let final_name = if clean.trim().is_empty() {
        sanitize_instance_name(&default)
    } else {
        clean
    };
    merge_version_jsons_to_instance(
        &final_name,
        mc_version,
        optifine_loader_version,
        "optifine",
        mc_dir,
    )?;
    Ok(())
}