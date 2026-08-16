//! 下载器共享工具模块
//! 
//! 提取各 mod loader installer 中的通用代码，减少重复。

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{self, Value};
use std::fs;
use std::path::{Path, PathBuf};

// ============= 通用结构体定义 =============

#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub url: Option<String>,
    pub downloads: Option<LibraryDownloads>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GameVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoaderVersion {
    pub separator: String,
    pub build: u64,
    pub maven: String,
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetaResponse {
    pub game: Vec<GameVersion>,
    pub loader: Vec<LoaderVersion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BmclEntry {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub mcversion: Option<String>,
}

// ============= Maven 坐标解析 =============

/// 解析 Maven 坐标字符串："groupId:artifactId:version[:classifier][@extension]"
/// 返回 `(group_path, artifact_id, version, classifier, extension)`
pub fn parse_maven_coordinate(
    coord: &str,
) -> Result<(String, String, String, Option<String>, String)> {
    let (coord_clean, ext) = match coord.rsplit_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (coord, "jar".to_string()),
    };

    let parts: Vec<&str> = coord_clean.split(':').collect();
    if parts.len() < 3 {
        return Err(anyhow!("无效 Maven 坐标: {}", coord));
    }

    let group_id = parts[0];
    let artifact_id = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).map(|s| s.to_string());

    let group_path = group_id.replace('.', "/");
    Ok((
        group_path,
        artifact_id.to_string(),
        version.to_string(),
        classifier,
        ext,
    ))
}

/// 解析库文件路径为文件系统路径
/// 返回：(父目录路径, 文件名)
pub fn parse_library_path_for_fs(name: &str) -> Result<(PathBuf, String)> {
    let (group_path, artifact_id, version, classifier, ext) = parse_maven_coordinate(name)?;
    
    let mut path = PathBuf::new();
    path.push(group_path.replace('/', &std::path::MAIN_SEPARATOR.to_string()));
    path.push(&artifact_id);
    path.push(&version);

    let jar_name = match classifier {
        Some(c) => format!("{}-{}-{}.{}", artifact_id, version, c, ext),
        None => format!("{}-{}.{}", artifact_id, version, ext),
    };
    
    Ok((path, jar_name))
}

/// 解析库文件路径为 URL 路径
pub fn parse_library_path_for_url(name: &str) -> Result<String> {
    let (group_path, artifact_id, version, classifier, ext) = parse_maven_coordinate(name)?;
    
    let jar_name = match classifier {
        Some(c) => format!("{}-{}-{}.{}", artifact_id, version, c, ext),
        None => format!("{}-{}.{}", artifact_id, version, ext),
    };
    
    Ok(format!(
        "{}/{}/{}/{}",
        group_path, artifact_id, version, jar_name
    ))
}

// ============= 版本列表获取工具 =============

/// 从 BMCLAPI 风格的 JSON 响应中提取版本列表
pub fn parse_bmcl_versions(list: Vec<BmclEntry>, mc_version: &str) -> Vec<String> {
    let mut versions: Vec<String> = list
        .into_iter()
        .filter(|e| {
            e.mcversion
                .as_ref()
                .map(|v| v == mc_version)
                .unwrap_or_else(|| e.version.contains(mc_version))
        })
        .map(|e| {
            let forge_ver = e.build.unwrap_or(e.version);
            format!("{}-{}", mc_version, forge_ver)
        })
        .collect();
    versions.sort_by(|a, b| b.cmp(a));
    versions.dedup();
    versions
}

/// 从 Meta 响应中提取 Loader 版本列表
pub fn parse_meta_versions(meta: MetaResponse, mc_version: &str) -> Result<Vec<String>> {
    if !meta.game.iter().any(|g| g.version == mc_version) {
        return Err(anyhow!("MC版本 {} 不存在于元数据中", mc_version));
    }
    let loader_versions = meta.loader.into_iter().map(|l| l.version).collect();
    Ok(loader_versions)
}

// ============= XML 版本解析工具 =============

/// 从 Maven metadata.xml 中提取版本列表（适用于 Fabric API、Quilt API 等）
pub fn parse_maven_metadata(xml_text: &str) -> Result<Vec<String>> {
    use regex::Regex;
    
    // 匹配 <version>xxx</version> 标签
    let re = Regex::new(r"<version>([^<]+)</version>")?;
    let mut versions: Vec<String> = re
        .find_iter(xml_text)
        .filter_map(|m| {
            m.as_str()
                .strip_prefix("<version>")?
                .strip_suffix("</version>")
        })
        .map(|s| s.trim().to_string())
        .collect();
    
    // 过滤掉 SNAPSHOT 版本（通常不需要）
    versions.retain(|v| !v.contains("SNAPSHOT"));
    // 按版本号倒序排序
    versions.sort_by(|a, b| compare_versions(b, a));
    versions.dedup();
    
    Ok(versions)
}

/// 比较版本字符串（简单实现，适用于大多数情况）
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| -> Vec<i32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i32>().ok())
            .collect()
    };
    
    let a_parts = parse_version(a);
    let b_parts = parse_version(b);
    
    a_parts.cmp(&b_parts)
}

// ============= JSON 工具 =============

/// 安全解析 JSON，返回 Option 而不是 Result
pub fn safe_json_parse<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_str(text).ok()
}

/// 安全解析 JSON，带错误信息
pub fn json_parse_with_error<T: serde::de::DeserializeOwned>(
    text: &str,
    context: &str,
) -> Result<T> {
    serde_json::from_str(text).with_context(|| format!("解析 JSON 失败 ({})", context))
}
// ============= 实例名称处理 =============

/// 清理实例名称，移除不合法的文件名字符
pub fn sanitize_instance_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if matches!(
            ch,
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' | '\n' | '\r' | '\t'
        ) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim().trim_matches('.');
    if trimmed.is_empty() {
        "minecraft-instance".to_string()
    } else {
        trimmed.to_string()
    }
}

fn current_iso_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;
    // 使用简单的 ISO 格式时间戳，无需额外依赖
    let years = 1970 + secs / 31536000;
    format!("{}-01-01T00:00:00+00:00", years)
}

fn dircpy(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            dircpy(&path, &target)?;
        } else {
            let _ = fs::copy(&path, &target);
        }
    }
    Ok(())
}

/// 合并原版和 ModLoader 的 version.json 到实例目录
/// 
/// 用于 ModLoader 下载后，将两个版本的 JSON 合并为一个实例，
/// 这样 versions 文件夹中只会有一个实例文件夹。
pub fn merge_version_jsons_to_instance(
    instance_name: &str,
    mc_version: &str,
    loader_version_name: &str,
    loader_type_hint: &str,
    minecraft_path: &Path,
) -> Result<()> {
    let versions_root = minecraft_path.join("versions");
    let vanilla_json_path = versions_root
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    let vanilla: Value = if vanilla_json_path.exists() {
        let text = fs::read_to_string(&vanilla_json_path)
            .with_context(|| format!("读取原版 version.json 失败: {:?}", vanilla_json_path))?;
        serde_json::from_str(&text).context("解析原版 version.json 失败")?
    } else {
        bail!("找不到原版 version.json: {:?}", vanilla_json_path);
    };

    let loader: Option<Value> = {
        let mut try_paths: Vec<std::path::PathBuf> = Vec::new();
        try_paths.push(
            versions_root
                .join(loader_version_name)
                .join(format!("{}.json", loader_version_name)),
        );
        if let Ok(entries) = fs::read_dir(&versions_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = match entry.file_name().into_string() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if dir_name == mc_version || dir_name == instance_name {
                    continue;
                }
                if !dir_name.starts_with(mc_version) {
                    continue;
                }
                let json_path = path.join(format!("{}.json", dir_name));
                if json_path.exists() {
                    let is_match = if !loader_type_hint.is_empty() {
                        let dir_lower = dir_name.to_ascii_lowercase();
                        let hint_lower = loader_type_hint.to_ascii_lowercase();
                        dir_lower.contains(&hint_lower)
                    } else {
                        false
                    };
                    if is_match {
                        if try_paths.len() < 2 {
                            try_paths.push(json_path);
                        } else {
                            try_paths.insert(1, json_path);
                        }
                    } else {
                        try_paths.push(json_path);
                    }
                }
            }
        }
        let mut result: Option<Value> = None;
        for p in &try_paths {
            if p.exists() {
                if let Ok(text) = fs::read_to_string(p) {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if v.get("mainClass").is_some() {
                            println!("[Instance] 找到 loader version.json: {}", p.display());
                            result = Some(v);
                            break;
                        }
                    }
                }
            }
        }
        result
    };

    let mut result = vanilla.clone();
    {
        let obj = result
            .as_object_mut()
            .ok_or_else(|| anyhow!("原版 version.json 顶层不是 JSON 对象"))?;
        obj.insert("id".to_string(), Value::String(instance_name.to_string()));
        obj.insert("time".to_string(), Value::String(current_iso_time()));
        obj.insert("releaseTime".to_string(), Value::String(current_iso_time()));
        if !obj.contains_key("minimumLauncherVersion") {
            obj.insert(
                "minimumLauncherVersion".to_string(),
                Value::Number(21.into()),
            );
        }
        // 合并 JSON 时移除 inheritsFrom，避免启动时递归引用 parent 并覆盖我们合并好的参数
        obj.remove("inheritsFrom");
        obj.remove("inherits-from");
        if let Some(loader_obj) = loader.as_ref().and_then(|v| v.as_object()) {
            if let Some(mc) = loader_obj.get("mainClass") {
                obj.insert("mainClass".to_string(), mc.clone());
            }
            if let Some(loader_args) = loader_obj.get("arguments").and_then(|v| v.as_object()) {
                let vanilla_args_obj = obj.get("arguments").and_then(|v| v.as_object()).cloned();
                let get_array = |o: &serde_json::Map<String, Value>, key: &str| -> Vec<Value> {
                    o.get(key)
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default()
                };
                let loader_jvm = get_array(loader_args, "jvm");
                let vanilla_jvm = vanilla_args_obj
                    .as_ref()
                    .map(|o| get_array(o, "jvm"))
                    .unwrap_or_default();
                let mut merged_jvm = loader_jvm;
                merged_jvm.extend(vanilla_jvm);
                let loader_game = get_array(loader_args, "game");
                let vanilla_game = vanilla_args_obj
                    .as_ref()
                    .map(|o| get_array(o, "game"))
                    .unwrap_or_default();
                let mut merged_game = loader_game;
                merged_game.extend(vanilla_game);
                let mut new_args = serde_json::Map::new();
                if !merged_jvm.is_empty() {
                    new_args.insert("jvm".to_string(), Value::Array(merged_jvm));
                }
                if !merged_game.is_empty() {
                    new_args.insert("game".to_string(), Value::Array(merged_game));
                }
                for (k, v) in loader_args {
                    if k != "jvm" && k != "game" {
                        new_args.insert(k.clone(), v.clone());
                    }
                }
                obj.insert("arguments".to_string(), Value::Object(new_args));
            } else if let Some(mc_args) = loader_obj.get("minecraftArguments") {
                obj.insert("minecraftArguments".to_string(), mc_args.clone());
            }
            if let Some(typ) = loader_obj.get("type") {
                obj.insert("type".to_string(), typ.clone());
            }
            for (k, v) in loader_obj {
                if k == "libraries"
                    || k == "id"
                    || k == "inheritsFrom"
                    || k == "inherits-from"
                    || k == "downloads"
                    || k == "assetIndex"
                    || k == "assets"
                    || k == "logging"
                    || k == "time"
                    || k == "releaseTime"
                    || k == "mainClass"
                    || k == "arguments"
                    || k == "minecraftArguments"
                    || k == "type"
                {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
            }
        }
        let mut merged_libs: Vec<Value> = Vec::new();
        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        let extract_gac = |name: &str| -> String {
            let parts: Vec<&str> = name.split(':').collect();
            match parts.len() {
                3 => {
                    format!("{}:{}", parts[0], parts[1])
                }
                4 => {
                    format!("{}:{}:{}", parts[0], parts[1], parts[3])
                }
                _ => name.to_string(),
            }
        };
        let extract_path_key = |path: &str| -> String {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 3 {
                let take = parts.len() - 2;
                parts[..take].join("/")
            } else {
                path.to_string()
            }
        };
        if let Some(loader_obj) = loader.as_ref().and_then(|v| v.as_object()) {
            if let Some(libs) = loader_obj.get("libraries").and_then(|v| v.as_array()) {
                for lib in libs {
                    if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
                        let key = extract_gac(name);
                        if seen_keys.insert(key) {
                            merged_libs.push(lib.clone());
                        }
                    } else if let Some(artifact_path) = lib
                        .get("downloads")
                        .and_then(|d| d.get("artifact"))
                        .and_then(|a| a.get("path"))
                        .and_then(|p| p.as_str())
                    {
                        let key = extract_path_key(artifact_path);
                        if seen_keys.insert(key) {
                            merged_libs.push(lib.clone());
                        }
                    } else {
                        merged_libs.push(lib.clone());
                    }
                }
            }
        }
        if let Some(libs) = obj.get("libraries").and_then(|v| v.as_array()) {
            for lib in libs {
                if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
                    let key = extract_gac(name);
                    if seen_keys.insert(key) {
                        merged_libs.push(lib.clone());
                    }
                } else if let Some(artifact_path) = lib
                    .get("downloads")
                    .and_then(|d| d.get("artifact"))
                    .and_then(|a| a.get("path"))
                    .and_then(|p| p.as_str())
                {
                    let key = extract_path_key(artifact_path);
                    if seen_keys.insert(key) {
                        merged_libs.push(lib.clone());
                    }
                } else {
                    merged_libs.push(lib.clone());
                }
            }
        }
        obj.insert("libraries".to_string(), Value::Array(merged_libs));
    }

    let version_dir = versions_root.join(instance_name);
    fs::create_dir_all(&version_dir).ok();
    let json_path = version_dir.join(format!("{}.json", instance_name));
    let text = serde_json::to_string_pretty(&result).context("序列化合并后的 version.json 失败")?;
    fs::write(&json_path, text)
        .with_context(|| format!("写入合并后的 version.json 失败: {:?}", json_path))?;
    let vanilla_jar = versions_root
        .join(mc_version)
        .join(format!("{}.jar", mc_version));
    let target_jar = version_dir.join(format!("{}.jar", instance_name));
    if vanilla_jar.exists() {
        let _ = fs::copy(&vanilla_jar, &target_jar);
    }
    let vanilla_natives = versions_root
        .join(mc_version)
        .join(format!("{}-natives", mc_version));
    let target_natives = version_dir.join(format!("{}-natives", instance_name));
    if vanilla_natives.exists() {
        let _ = dircpy(&vanilla_natives, &target_natives);
    }
    for sub in &["mods", "resourcepacks", "shaderpacks", "config", "saves"] {
        let _ = fs::create_dir_all(version_dir.join(sub));
    }
    println!(
        "[Instance] version.json merged: {} <- ({}, {})",
        json_path.display(),
        mc_version,
        loader_version_name
    );
    Ok(())
}

/// 扫描所有版本的 version.json，判断是否有版本通过 inheritsFrom 引用 target。
fn is_referenced_by_other_versions(versions_root: &Path, target: &str) -> bool {
    if let Ok(entries) = fs::read_dir(versions_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let json_path = path.join(format!("{}.json", dir_name));
            let Ok(text) = fs::read_to_string(&json_path) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if v.get("inheritsFrom")
                .and_then(|x| x.as_str())
                .is_some_and(|s| s == target)
            {
                return true;
            }
        }
    }
    false
}

/// 合并成功后清理中间的 loader 版本目录（如 {mc}-neoforge-{lv}）。
///
/// 该目录只有一份 raw loader version.json（inheritsFrom 原版），既没有
/// 独立游戏 JAR，也缺少 -p/-DignoreList 等模块引导参数，无法直接启动，
/// 留着只会让版本列表多出一个启动失败的条目。
/// 仅当没有其他版本的 version.json 通过 inheritsFrom 引用它时才删除，
/// 避免破坏以简单 inherits 模式安装的整合包。
pub fn cleanup_loader_version_dir(loader_version: &str, minecraft_path: &Path) {
    let versions_root = minecraft_path.join("versions");
    let loader_dir = versions_root.join(loader_version);
    if !loader_dir.is_dir() {
        return;
    }
    if is_referenced_by_other_versions(&versions_root, loader_version) {
        println!(
            "[Cleanup] 版本 {} 仍被其他版本 inheritsFrom 引用，保留中间目录",
            loader_version
        );
        return;
    }
    match fs::remove_dir_all(&loader_dir) {
        Ok(_) => println!(
            "[Cleanup] 已删除中间 loader 版本目录: {}",
            loader_dir.display()
        ),
        Err(e) => eprintln!("[Cleanup] 删除中间 loader 版本目录失败（忽略）: {}", e),
    }
}

/// 组包后清理原版版本目录（versions/{mc}）。
///
/// merge_version_jsons_to_instance 产出的合并实例是自包含的（无 inheritsFrom、
/// 库已全量合并、游戏 JAR 已复制进实例目录），启动不再依赖原版目录。
/// 与 HMCL 一致：原版与模组加载器合并为单个版本条目。
/// 安全删除条件：
/// 1. 没有其他版本 json 通过 inheritsFrom 引用它；
/// 2. 目录内容干净（仅 json/jar/natives 与空的标准子目录），避免误删用户文件。
pub fn cleanup_vanilla_version_dir(mc_version: &str, minecraft_path: &Path) {
    let versions_root = minecraft_path.join("versions");
    let vanilla_dir = versions_root.join(mc_version);
    if !vanilla_dir.is_dir() {
        return;
    }
    if is_referenced_by_other_versions(&versions_root, mc_version) {
        println!(
            "[Cleanup] 原版版本 {} 仍被其他版本 inheritsFrom 引用，保留目录",
            mc_version
        );
        return;
    }
    let Ok(entries) = fs::read_dir(&vanilla_dir) else {
        return;
    };
    let mut not_pristine = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_core_file = path.is_file()
            && (name == format!("{}.json", mc_version) || name == format!("{}.jar", mc_version));
        let is_natives_dir =
            path.is_dir() && name == format!("{}-natives", mc_version);
        let is_empty_std_dir = path.is_dir()
            && [
                "mods",
                "resourcepacks",
                "shaderpacks",
                "config",
                "saves",
                "datapacks",
                "logs",
                "crash-reports",
            ]
            .contains(&name.as_str())
            && fs::read_dir(&path)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
        if !(is_core_file || is_natives_dir || is_empty_std_dir) {
            not_pristine = true;
            println!(
                "[Cleanup] 原版目录含用户文件，跳过删除: {}",
                path.display()
            );
            break;
        }
    }
    if not_pristine {
        return;
    }
    match fs::remove_dir_all(&vanilla_dir) {
        Ok(_) => println!(
            "[Cleanup] 已删除原版版本目录（已与加载器合并为单个实例）: {}",
            vanilla_dir.display()
        ),
        Err(e) => eprintln!("[Cleanup] 删除原版版本目录失败（忽略）: {}", e),
    }
}

/// 把原版/加载器版本目录里的 mods/*.jar 复制到实例的 mods/ 目录。
///
/// 用于 Fabric API、Quilt API、叠加安装的 OptiFine 等以 mod 形式安装的文件，
/// 安装时它们落在 versions/{mc}/mods 或 versions/{loader}/mods，
/// 而 merge_version_jsons_to_instance 只复制 jar/natives，需要单独搬运 mods。
pub fn copy_version_mods_to_instance(
    instance_name: &str,
    from_version: &str,
    minecraft_path: &Path,
) {
    let versions_root = minecraft_path.join("versions");
    let src_mods = versions_root.join(from_version).join("mods");
    let dst_mods = versions_root.join(instance_name).join("mods");
    if !src_mods.is_dir() {
        return;
    }
    fs::create_dir_all(&dst_mods).ok();
    let Ok(entries) = fs::read_dir(&src_mods) else {
        return;
    };
    let mut copied = 0usize;
    for entry in entries.flatten() {
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let dst = dst_mods.join(entry.file_name());
        if dst.exists() {
            continue;
        }
        if fs::copy(&src, &dst).is_ok() {
            copied += 1;
        }
    }
    if copied > 0 {
        println!(
            "[Mods] 已复制 {} 个 mod 文件到实例 mods 目录: {}",
            copied,
            dst_mods.display()
        );
    }
}

/// 为原版 Minecraft 创建实例目录（复制 JSON 和 JAR）
pub fn create_vanilla_instance(
    instance_name: &str,
    mc_version: &str,
    minecraft_path: &Path,
) -> Result<()> {
    let versions_root = minecraft_path.join("versions");
    let vanilla_json_path = versions_root
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    let vanilla_jar_path = versions_root
        .join(mc_version)
        .join(format!("{}.jar", mc_version));

    if !vanilla_json_path.exists() {
        bail!("找不到原版 version.json: {:?}", vanilla_json_path);
    }

    let version_dir = versions_root.join(instance_name);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("创建实例目录失败: {:?}", version_dir))?;

    // 读取并修改 JSON
    let text = fs::read_to_string(&vanilla_json_path)
        .with_context(|| format!("读取原版 version.json 失败"))?;
    let mut json: Value = serde_json::from_str(&text).context("解析原版 version.json 失败")?;
    
    if let Some(obj) = json.as_object_mut() {
        obj.insert("id".to_string(), Value::String(instance_name.to_string()));
        obj.insert("time".to_string(), Value::String(current_iso_time()));
        obj.insert("releaseTime".to_string(), Value::String(current_iso_time()));
    }

    let json_target = version_dir.join(format!("{}.json", instance_name));
    fs::write(
        &json_target,
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )
    .with_context(|| format!("写入实例 version.json 失败"))?;

    // 复制 JAR
    if vanilla_jar_path.exists() {
        let jar_target = version_dir.join(format!("{}.jar", instance_name));
        let _ = fs::copy(&vanilla_jar_path, &jar_target);
    }

    // 复制 natives
    let vanilla_natives = versions_root
        .join(mc_version)
        .join(format!("{}-natives", mc_version));
    let target_natives = version_dir.join(format!("{}-natives", instance_name));
    if vanilla_natives.exists() {
        let _ = dircpy(&vanilla_natives, &target_natives);
    }

    // 创建子目录
    for sub in &["mods", "resourcepacks", "shaderpacks", "config", "saves"] {
        let _ = fs::create_dir_all(version_dir.join(sub));
    }

    println!("[Instance] 原版实例创建完成: {}", instance_name);
    Ok(())
}