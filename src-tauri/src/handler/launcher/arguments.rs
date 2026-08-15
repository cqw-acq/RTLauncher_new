use anyhow::Context;
use os_info::Type;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tauri::Emitter;

use super::{
    identity::{is_valid_uuid, launch_auth_identity, offline_uuid},
    java_runtime::{get_java_major_version, is_plausible_minecraft_version},
    memory::{is_heap_size_argument, resolve_max_memory_mb},
    process::GameLogEvent,
};

/// 从合并型整合包 version.json 的 `patches` 字段读取真实 Minecraft 版本
pub(super) fn detect_minecraft_version_from_patches(json: &serde_json::Value) -> Option<String> {
    json.get("patches")?.as_array()?.iter().find_map(|patch| {
        if patch.get("id").and_then(|v| v.as_str()) != Some("game") {
            return None;
        }
        let ver = patch.get("version").and_then(|v| v.as_str())?;
        Some(ver.to_string())
    })
}

/// 清理参数中的空格，将引号内的空格移除
/// 例如: "-DFabricMcEmu= net.minecraft.client.main.Main " -> "-DFabricMcEmu=net.minecraft.client.main.Main"
fn clean_param_spaces(param: &str) -> String {
    let trimmed = param.trim();
    // 检查参数是否被引号包围
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with("'") && trimmed.ends_with("'"))
    {
        // 移除外层引号
        let inner = &trimmed[1..trimmed.len() - 1];
        // 移除内部所有空格
        inner.chars().filter(|c| !c.is_whitespace()).collect()
    } else {
        trimmed.to_string()
    }
}

/// 简单的 shell 风格分词：按空格拆分，但保持单/双引号内的内容为一个 token；不支持转义。
/// 用于把用户输入的自定义 JVM 参数按空格/换行拆成独立参数。
fn shell_split(input: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// 对 -cp / -p 等路径列表参数做去重（保留原有顺序）。
///
/// 部分启动器（如 PCL CE）生成的 version.json 会把父版本库与加载器库
/// 简单拼接而不去重，导致同一个 jar 在 classpath 中出现多次。重复的
/// 路径会让 NeoForge/Forge 的 UnionFileSystem 在启动阶段直接抛出
/// "Duplicate key ... (attempted merging values ...)" 并以退出码 1 退出。
pub(super) fn dedup_path_list(value: &str) -> String {
    let sep = if value.contains(';') { ';' } else { ':' };
    let mut seen: HashSet<String> = HashSet::new();
    let mut parts: Vec<&str> = Vec::new();
    for piece in value.split(sep) {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        // Windows 文件系统不区分大小写，统一转小写后去重
        if seen.insert(piece.to_lowercase()) {
            parts.push(piece);
        }
    }
    parts.join(&sep.to_string())
}

/// Add JVM arguments read from a separate loader JSON.
///
/// A selected instance may use the vanilla JSON as `version_name` and a Forge
/// JSON as `loadName`. In that layout vanilla supplies `-cp`, while Forge alone
/// supplies the required `-p` module path. Keep a loader path argument unless
/// the same path key is already present in the effective argument list.
pub(super) fn append_loader_jvm_args(args: &mut Vec<String>, loader_args: &[String]) {
    let mut index = 0;
    while index < loader_args.len() {
        let key = &loader_args[index];
        let has_value = key.starts_with('-')
            && index + 1 < loader_args.len()
            && !loader_args[index + 1].starts_with('-');
        let is_path_key = matches!(
            key.as_str(),
            "-p" | "--module-path" | "-cp" | "--class-path"
        );
        let already_present = is_path_key && args.iter().any(|arg| arg == key);

        if !already_present {
            args.push(key.clone());
            if has_value {
                let value = &loader_args[index + 1];
                if is_path_key {
                    // PCL 等启动器写入的 classpath 可能包含重复 jar，去重后再转发
                    args.push(dedup_path_list(value));
                } else {
                    args.push(value.clone());
                }
            }
        }

        index += if has_value { 2 } else { 1 };
    }
}

/// 将Maven库名称转换为文件系统路径
/// 例如: "net.minecraft:launchwrapper:1.12" -> "net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar"
fn library_name_to_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 3 {
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        Some(format!(
            "{}/{}/{}/{}-{}.jar",
            group, artifact, version, artifact, version
        ))
    } else {
        None
    }
}

/// 从库路径中提取库的标识信息（group, artifact, version）
/// 例如: "org/apache/commons/commons-lang3/3.3.2/commons-lang3-3.3.2.jar" -> ("org/apache/commons", "commons-lang3", "3.3.2")
fn parse_library_path(path: &str) -> Option<(String, String, String)> {
    // 移除.jar扩展名
    let path_without_ext = path.strip_suffix(".jar")?;

    // 分割路径
    let parts: Vec<&str> = path_without_ext.split('/').collect();
    if parts.len() >= 4 {
        // 路径格式: group/artifact/version/artifact-version
        let group = parts[..parts.len() - 3].join("/");
        let artifact = parts[parts.len() - 3];
        let version = parts[parts.len() - 2];

        // 验证artifact-version格式
        let expected_filename = format!("{}-{}", artifact, version);
        if parts[parts.len() - 1] == expected_filename {
            Some((group, artifact.to_string(), version.to_string()))
        } else {
            None
        }
    } else {
        None
    }
}

/// 比较两个版本号，返回true如果version1 > version2
/// 简单版本比较，不支持语义化版本的所有特性
fn compare_versions(version1: &str, version2: &str) -> bool {
    let v1_parts: Vec<&str> = version1.split('.').collect();
    let v2_parts: Vec<&str> = version2.split('.').collect();

    for i in 0..std::cmp::max(v1_parts.len(), v2_parts.len()) {
        let v1_part = v1_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let v2_part = v2_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        if v1_part > v2_part {
            return true;
        } else if v1_part < v2_part {
            println!("liteloader是{}, forge是{}", version1, version2);
            return false;
        }
    }

    false
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VersionJson {
    arguments: Option<Arguments>,
    #[serde(rename = "javaVersion")]
    pub(super) java_version: Option<JavaVersion>,
    main_class: String,
    libraries: Vec<Library>,
    #[serde(rename = "inheritsFrom")]
    parent_version: Option<String>,
    logging: Option<Logging>,
    minecraft_arguments: Option<String>,
    asset_index: Option<AssetIndex>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub(super) major_version: u32,
}

#[derive(Debug, Deserialize)]
struct AssetIndex {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Logging {
    client: Option<LoggingClient>,
}

#[derive(Debug, Deserialize)]
struct LoggingClient {
    file: LogFile,
}

#[derive(Debug, Deserialize)]
struct LogFile {
    id: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Arguments {
    jvm: Option<Vec<JvmArgument>>,
    game: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum JvmArgument {
    String(String),
    Object {
        rules: Vec<Rule>,
        value: serde_json::Value,
    },
}

#[derive(Debug, Deserialize, Clone)]
struct Rule {
    #[serde(rename = "action")]
    action: String,
    #[serde(default)]
    os: Option<OsRule>,
}

#[derive(Debug, Deserialize, Clone)]
struct OsRule {
    name: Option<String>,
    arch: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Library {
    name: String,
    #[serde(default)]
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
    #[serde(default)]
    natives: HashMap<String, String>,
    #[serde(default)]
    serverreq: bool,
}

#[derive(Debug, Deserialize, Default)]
struct LibraryDownloads {
    artifact: Option<Artifact>,
    #[serde(default)]
    classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Deserialize)]
struct Artifact {
    path: String,
    url: String,
    sha1: String,
    size: u64,
}

#[tauri::command]
pub fn build_jvm_arguments(
    app: tauri::AppHandle,
    minecraft_path: &str,
    java_path: &str,
    wrapper_path: &str,
    max_memory: &str,
    version_name: &str,
    minecraft_version: &str,
    player_name: &str,
    auth_token: &str,
    uuid: &str,
    authlib_injector_path: &str,
    yggdrasil_api: &str,
    prefetched_data: &str,
    loadType: &str,
    loadName: &str,
    window_width: &str,
    window_height: &str,
    custom_jvm_args: &str,
) -> Result<String, String> {
    build_jvm_arguments_inner(
        app,
        minecraft_path,
        java_path,
        wrapper_path,
        max_memory,
        version_name,
        minecraft_version,
        player_name,
        auth_token,
        uuid,
        authlib_injector_path,
        yggdrasil_api,
        prefetched_data,
        loadType,
        loadName,
        window_width,
        window_height,
        custom_jvm_args,
    )
    .map(|args| args.join(" "))
    .map_err(|e| e.to_string())
}

pub(super) fn build_jvm_arguments_inner(
    app_handle: tauri::AppHandle,
    minecraft_path: &str,
    java_path: &str,
    wrapper_path: &str,
    max_memory: &str,
    version_name: &str,
    minecraft_version: &str,
    player_name: &str,
    auth_token: &str,
    uuid: &str,
    authlib_injector_path: &str,
    yggdrasil_api: &str,
    prefetched_data: &str,
    loadType: &str,
    loadName: &str,
    window_width: &str,
    window_height: &str,
    custom_jvm_args: &str,
) -> anyhow::Result<Vec<String>> {
    let minecraft_path_buf = PathBuf::from(minecraft_path);
    let (effective_max_memory_mb, memory_warning) = resolve_max_memory_mb(max_memory)?;
    if let Some(warning) = memory_warning.as_ref() {
        warn!("{warning}");
        let _ = app_handle.emit(
            "game-log",
            GameLogEvent {
                level: "warn".to_string(),
                message: warning.clone(),
            },
        );
    }

    // 确定实际的游戏目录（用于运行游戏）
    let game_directory = if loadType != "0" && !loadName.is_empty() {
        // 对于有加载器的情况，游戏目录应该是版本隔离目录
        minecraft_path_buf.join("versions").join(loadName)
    } else {
        // 对于原版，游戏目录应该是 minecraft_path 本身
        minecraft_path_buf.clone()
    };

    // 如果 uuid 为空或不合法，根据玩家名生成离线 UUID
    let uuid = if uuid.is_empty() || !is_valid_uuid(uuid) {
        let generated = offline_uuid(player_name);
        println!(
            "[启动器] UUID 无效 (\"{}\"), 已根据玩家名生成: {}",
            uuid, generated
        );
        generated
    } else {
        uuid.to_string()
    };
    let uuid = uuid.as_str();

    // 确定版本JSON的路径
    let version_path = if loadType != "0" && !loadName.is_empty() {
        // 对于有加载器的情况，尝试从加载器目录读取版本JSON
        let loader_json_path = minecraft_path_buf
            .join("versions")
            .join(loadName)
            .join(format!("{}.json", loadName));

        if loader_json_path.exists() {
            println!("使用加载器版本JSON: {}", loader_json_path.display());
            loader_json_path
        } else {
            // 回退到原版路径
            println!(
                "加载器版本JSON不存在: {}, 尝试原版路径: {}",
                loader_json_path.display(),
                version_name
            );
            let fallback_path = minecraft_path_buf
                .join("versions")
                .join(version_name)
                .join(format!("{}.json", version_name));

            if fallback_path.exists() {
                println!("使用原版版本JSON: {}", fallback_path.display());
                fallback_path
            } else {
                // 如果两个都不存在，返回错误
                return Err(anyhow::anyhow!(
                    "版本JSON文件不存在。尝试的路径:\n1. 加载器路径: {}\n2. 原版路径: {}",
                    loader_json_path.display(),
                    fallback_path.display()
                ));
            }
        }
    } else {
        // 对于原版，使用标准路径
        let standard_path = minecraft_path_buf
            .join("versions")
            .join(version_name)
            .join(format!("{}.json", version_name));

        if standard_path.exists() {
            standard_path
        } else {
            return Err(anyhow::anyhow!(
                "版本JSON文件不存在: {}",
                standard_path.display()
            ));
        }
    };

    let mut load_library_paths: Vec<String> = Vec::new();
    let mut load_jvm_params: Vec<String> = Vec::new();
    let mut load_game_params: Vec<String> = Vec::new();
    let mut load_main_class: Option<String> = None;

    let normalize = |p: &PathBuf| p.to_string_lossy().replace('\\', "/");

    if loadType != "0" {
        let load_path = minecraft_path_buf.join("versions").join(loadName);
        println!(
            "正在加载版本信息，loadType: {}, loadName: {}, loadPath: {}",
            loadType,
            loadName,
            load_path.display()
        );
        if loadType == "1" {
            if load_path.is_dir() {
                println!("load_path是目录，开始读取JSON文件");
                let entries: Vec<_> = std::fs::read_dir(&load_path)
                    .context("Failed to read load_path dir")?
                    .collect();
                println!("目录中共有 {} 个文件/文件夹", entries.len());
                for entry in entries {
                    let entry = entry.context("Failed to read dir entry")?;
                    let path = entry.path();
                    println!("检查文件: {}", path.display());
                    println!("  文件扩展名: {:?}", path.extension());
                    if path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                    {
                        debug!("找到JSON文件: {}", path.display());
                        let content = std::fs::read_to_string(&path)
                            .with_context(|| format!("Failed to read {}", path.display()))?;

                        let value: serde_json::Value = serde_json::from_str(&content)?;

                        println!(
                            "解析后的JSON值: {}",
                            serde_json::to_string_pretty(&value)
                                .unwrap_or_else(|_| "无法序列化JSON".to_string())
                        );

                        let root: &serde_json::Value = if let Some(vinfo) = value.get("versionInfo")
                        {
                            println!("使用versionInfo字段作为根对象");
                            vinfo
                        } else {
                            &value
                        };

                        println!("开始提取mainClass和参数");
                        println!(
                            "JSON根对象的所有键: {:?}",
                            root.as_object().map(|o| o.keys().collect::<Vec<_>>())
                        );
                        if let Some(main_class) = root.get("mainClass").and_then(|v| v.as_str()) {
                            debug!("找到mainClass: {}", main_class);
                            load_main_class = Some(main_class.to_string());
                        }

                        if let Some(mca) = root.get("minecraftArguments").and_then(|v| v.as_str()) {
                            for token in mca.split_whitespace() {
                                load_game_params.push(token.trim().to_string());
                            }
                        } else {
                            if let Some(args_obj) = root.get("arguments") {
                                let library_dir_str =
                                    normalize(&minecraft_path_buf.join("libraries"));
                                let classpath_sep = if cfg!(windows) { ";" } else { ":" };

                                // 处理 arguments.game
                                if let Some(game_arr) =
                                    args_obj.get("game").and_then(|v| v.as_array())
                                {
                                    for el in game_arr {
                                        if let Some(s) = el.as_str() {
                                            load_game_params.push(s.trim().to_string());
                                        } else if let Some(obj) = el.as_object() {
                                            // 处理带 rules 的对象
                                            let rules_match =
                                                launcher_rules_allow(obj.get("rules"));
                                            if rules_match {
                                                if let Some(val) = obj.get("value") {
                                                    if let Some(s) = val.as_str() {
                                                        load_game_params.push(s.trim().to_string());
                                                    } else if let Some(arr) = val.as_array() {
                                                        for item in arr {
                                                            if let Some(s) = item.as_str() {
                                                                load_game_params
                                                                    .push(s.trim().to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // 处理 arguments.jvm（关键：包含 -p、-cp、--add-opens 等）
                                if let Some(jvm_arr) =
                                    args_obj.get("jvm").and_then(|v| v.as_array())
                                {
                                    let natives_dir = minecraft_path_buf
                                        .join("versions")
                                        .join(version_name)
                                        .join(format!("{}-natives", version_name))
                                        .to_string_lossy()
                                        .replace('\\', "/");
                                    for el in jvm_arr {
                                        fn apply_placeholder(
                                            s: &str,
                                            classpath_sep: &str,
                                            library_dir_str: &str,
                                            version_name: &str,
                                            natives_dir: &str,
                                        ) -> String {
                                            let mut replaced = s.to_string();
                                            replaced = replaced
                                                .replace("${classpath_separator}", classpath_sep);
                                            replaced = replaced
                                                .replace("${library_directory}", library_dir_str);
                                            replaced =
                                                replaced.replace("${version_name}", version_name);
                                            replaced = replaced
                                                .replace("${natives_directory}", natives_dir);
                                            replaced =
                                                replaced.replace("${launcher_name}", "RTLauncher");
                                            replaced = replaced.replace(
                                                "${launcher_version}",
                                                env!("CARGO_PKG_VERSION"),
                                            );
                                            // 剩余的${}格式参数转换为{}格式（与游戏参数保持一致）
                                            let re = Regex::new(r"\$\{[^}]+\}").unwrap();
                                            replaced = re.replace_all(&replaced, "{}").to_string();
                                            replaced.trim().to_string()
                                        }

                                        let collect_str = |s: &str,
                                                           out: &mut Vec<String>,
                                                           classpath_sep: &str,
                                                           library_dir_str: &str,
                                                           version_name: &str,
                                                           natives_dir: &str| {
                                            let replaced = apply_placeholder(
                                                s,
                                                classpath_sep,
                                                library_dir_str,
                                                version_name,
                                                natives_dir,
                                            );
                                            if !replaced.is_empty() {
                                                out.push(replaced);
                                            }
                                        };

                                        fn check_rules(
                                            obj: &serde_json::Map<String, serde_json::Value>,
                                        ) -> bool {
                                            match obj.get("rules").and_then(|r| r.as_array()) {
                                                Some(rules) => {
                                                    let mut allowed = false;
                                                    for rule in rules {
                                                        if let Some(action) = rule
                                                            .get("action")
                                                            .and_then(|a| a.as_str())
                                                        {
                                                            let os_match = match rule
                                                                .get("os")
                                                                .and_then(|o| o.as_object())
                                                            {
                                                                Some(os) => match os
                                                                    .get("name")
                                                                    .and_then(|n| n.as_str())
                                                                {
                                                                    Some("windows") => {
                                                                        cfg!(windows)
                                                                    }
                                                                    Some("osx") => {
                                                                        cfg!(target_os = "macos")
                                                                    }
                                                                    Some("linux") => {
                                                                        cfg!(target_os = "linux")
                                                                    }
                                                                    _ => true,
                                                                },
                                                                None => true,
                                                            };
                                                            if action == "allow" && os_match {
                                                                allowed = true;
                                                            } else if action == "disallow"
                                                                && os_match
                                                            {
                                                                allowed = false;
                                                            }
                                                        }
                                                    }
                                                    allowed
                                                }
                                                None => true,
                                            }
                                        }

                                        if let Some(s) = el.as_str() {
                                            collect_str(
                                                s,
                                                &mut load_jvm_params,
                                                classpath_sep,
                                                &library_dir_str,
                                                &version_name,
                                                &natives_dir,
                                            );
                                        } else if let Some(obj) = el.as_object() {
                                            if check_rules(obj) {
                                                if let Some(val) = obj.get("value") {
                                                    if let Some(s) = val.as_str() {
                                                        collect_str(
                                                            s,
                                                            &mut load_jvm_params,
                                                            classpath_sep,
                                                            &library_dir_str,
                                                            &version_name,
                                                            &natives_dir,
                                                        );
                                                    } else if let Some(arr) = val.as_array() {
                                                        for item in arr {
                                                            if let Some(s) = item.as_str() {
                                                                collect_str(
                                                                    s,
                                                                    &mut load_jvm_params,
                                                                    classpath_sep,
                                                                    &library_dir_str,
                                                                    &version_name,
                                                                    &natives_dir,
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                debug!("未找到arguments字段");
                            }
                        }

                        debug!("JSON参数处理完成: main_class={:?}, game_params_len={}, jvm_params_len={}",
                            load_main_class, load_game_params.len(), load_jvm_params.len());

                        // 检查是否是LiteLoader
                        let is_liteloader = load_main_class
                            .as_ref()
                            .map_or(false, |s| s.contains("LiteLoader"));
                        println!("是否是LiteLoader: {}", is_liteloader);

                        // 处理versionPatch.json（如果有）
                        let mut patch_library_paths: Vec<String> = Vec::new();
                        if is_liteloader {
                            let patch_json_path = load_path.join("versionPatch.json");
                            if patch_json_path.exists() {
                                debug!("找到versionPatch.json，开始处理");
                                let patch_content = std::fs::read_to_string(&patch_json_path)
                                    .with_context(|| {
                                        format!("Failed to read {}", patch_json_path.display())
                                    })?;
                                let patch_value: serde_json::Value =
                                    serde_json::from_str(&patch_content).with_context(|| {
                                        format!("Failed to parse {}", patch_json_path.display())
                                    })?;

                                if let Some(patch_libraries) =
                                    patch_value.get("libraries").and_then(|v| v.as_array())
                                {
                                    for patch_lib in patch_libraries {
                                        if let Some(downloads) = patch_lib.get("downloads") {
                                            if let Some(artifact) = downloads.get("artifact") {
                                                if let Some(path_str) =
                                                    artifact.get("path").and_then(|p| p.as_str())
                                                {
                                                    let abs = minecraft_path_buf
                                                        .join("libraries")
                                                        .join(path_str);
                                                    let norm = normalize(&abs);
                                                    patch_library_paths.push(norm);
                                                }
                                            }
                                            if let Some(classifiers) = downloads
                                                .get("classifiers")
                                                .and_then(|v| v.as_object())
                                            {
                                                for art in classifiers.values() {
                                                    if let Some(path_str) =
                                                        art.get("path").and_then(|p| p.as_str())
                                                    {
                                                        let abs = minecraft_path_buf
                                                            .join("libraries")
                                                            .join(path_str);
                                                        let norm = normalize(&abs);
                                                        patch_library_paths.push(norm);
                                                    }
                                                }
                                            }
                                        } else if let Some(name_val) =
                                            patch_lib.get("name").and_then(|n| n.as_str())
                                        {
                                            // 对于没有downloads字段但有name字段的库
                                            if let Some(lib_path) = library_name_to_path(name_val) {
                                                let abs = minecraft_path_buf
                                                    .join("libraries")
                                                    .join(&lib_path);
                                                let norm = normalize(&abs);
                                                patch_library_paths.push(norm);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(libraries) = root.get("libraries").and_then(|v| v.as_array()) {
                            for lib in libraries {
                                if let Some(downloads) = lib.get("downloads") {
                                    if let Some(artifact) = downloads.get("artifact") {
                                        if let Some(path_str) =
                                            artifact.get("path").and_then(|p| p.as_str())
                                        {
                                            let abs =
                                                minecraft_path_buf.join("libraries").join(path_str);
                                            let norm = normalize(&abs);
                                            debug!("library artifact path: {}", abs.display());
                                            load_library_paths.push(norm.clone());

                                            if let Some(name) =
                                                lib.get("name").and_then(|n| n.as_str())
                                            {
                                                if name.starts_with("net.minecraftforge:forge") {
                                                    if let Some(folder) = abs.parent() {
                                                        if folder.is_dir() {
                                                            for jf in std::fs::read_dir(folder)? {
                                                                let jf = jf?;
                                                                let jfpath = jf.path();
                                                                if jfpath
                                                                    .extension()
                                                                    .and_then(|s| s.to_str())
                                                                    == Some("jar")
                                                                {
                                                                    println!(
                                                                        "forge jar: {}",
                                                                        jfpath.display()
                                                                    );
                                                                    load_library_paths
                                                                        .push(normalize(&jfpath));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if let Some(classifiers) =
                                        downloads.get("classifiers").and_then(|v| v.as_object())
                                    {
                                        for art in classifiers.values() {
                                            if let Some(path_str) =
                                                art.get("path").and_then(|p| p.as_str())
                                            {
                                                let abs = minecraft_path_buf
                                                    .join("libraries")
                                                    .join(path_str);
                                                let norm = normalize(&abs);
                                                println!(
                                                    "library classifier path: {}",
                                                    abs.display()
                                                );
                                                load_library_paths.push(norm);
                                            }
                                        }
                                    }
                                } else {
                                    // 对于所有有name字段的库，都根据name构建路径并添加到classpath
                                    // 这确保了像LiteLoader这样的mod加载器的所有依赖库都被正确加载
                                    // 无论是否有downloads、url或serverreq字段
                                    if let Some(name_val) = lib.get("name").and_then(|n| n.as_str())
                                    {
                                        if let Some(lib_path) = library_name_to_path(name_val) {
                                            let abs = minecraft_path_buf
                                                .join("libraries")
                                                .join(&lib_path);
                                            let norm = normalize(&abs);
                                            println!(
                                                "library artifact path (from name): {}",
                                                abs.display()
                                            );
                                            // 检查是否已经添加过，避免重复
                                            if !load_library_paths.contains(&norm) {
                                                load_library_paths.push(norm);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 如果是LiteLoader，比较load_library_paths和patch_library_paths
                        // 如果patch中的库版本更高，则替换load中的库
                        if is_liteloader && !patch_library_paths.is_empty() {
                            debug!("正在比较LiteLoader和versionPatch.json中的库版本...");

                            // 存储需要移除的load库的索引
                            let mut indices_to_remove: Vec<usize> = Vec::new();

                            // 存储需要添加的patch库
                            let mut patches_to_add: Vec<String> = Vec::new();

                            // 遍历load_library_paths，查找是否有更高版本的patch库
                            for (i, load_path) in load_library_paths.iter().enumerate() {
                                if let Some((load_group, load_artifact, load_version)) =
                                    parse_library_path(load_path)
                                {
                                    // 在patch_library_paths中查找相同group和artifact的库
                                    for patch_path in &patch_library_paths {
                                        if let Some((patch_group, patch_artifact, patch_version)) =
                                            parse_library_path(patch_path)
                                        {
                                            // 如果group和artifact相同，比较版本号
                                            if load_group == patch_group
                                                && load_artifact == patch_artifact
                                            {
                                                // 如果patch库的版本更高，标记load库为需要移除
                                                if compare_versions(&patch_version, &load_version) {
                                                    indices_to_remove.push(i);
                                                    patches_to_add.push(patch_path.clone());
                                                    debug!("替换库: {} (load版本: {}) -> {} (patch版本: {})",
                                                        load_path, load_version, patch_path, patch_version);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 移除需要替换的load库（从后往前移，避免索引变化）
                            indices_to_remove.sort();
                            indices_to_remove.dedup();
                            for i in indices_to_remove.into_iter().rev() {
                                load_library_paths.remove(i);
                            }

                            // 添加patch库
                            for patch_path in patches_to_add {
                                if !load_library_paths.contains(&patch_path) {
                                    load_library_paths.push(patch_path);
                                }
                            }
                        }
                    }
                }
            } else {
                debug!("load_path不是目录: {}", load_path.display());
            }
        } else {
            if load_path.is_dir() {
                for entry in
                    std::fs::read_dir(&load_path).context("Failed to read load_path dir")?
                {
                    let entry = entry.context("Failed to read dir entry")?;
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                    {
                        let content = std::fs::read_to_string(&path)
                            .with_context(|| format!("Failed to read file {}", path.display()))?;
                        debug!("Content of {}:\n{}", path.display(), content);

                        // 解析 JSON 并提取库信息
                        let value: serde_json::Value = serde_json::from_str(&content)
                            .with_context(|| format!("Failed to parse {}", path.display()))?;

                        let root: &serde_json::Value = if let Some(vinfo) = value.get("versionInfo")
                        {
                            vinfo
                        } else {
                            &value
                        };

                        // 提取库信息
                        if let Some(libraries) = root.get("libraries").and_then(|v| v.as_array()) {
                            for lib in libraries {
                                if let Some(downloads) = lib.get("downloads") {
                                    if let Some(artifact) = downloads.get("artifact") {
                                        if let Some(path_str) =
                                            artifact.get("path").and_then(|p| p.as_str())
                                        {
                                            let abs =
                                                minecraft_path_buf.join("libraries").join(path_str);
                                            let norm = normalize(&abs);
                                            load_library_paths.push(norm);
                                        }
                                    }
                                    if let Some(classifiers) =
                                        downloads.get("classifiers").and_then(|v| v.as_object())
                                    {
                                        for art in classifiers.values() {
                                            if let Some(path_str) =
                                                art.get("path").and_then(|p| p.as_str())
                                            {
                                                let abs = minecraft_path_buf
                                                    .join("libraries")
                                                    .join(path_str);
                                                let norm = normalize(&abs);
                                                load_library_paths.push(norm);
                                            }
                                        }
                                    }
                                } else if let Some(name_val) =
                                    lib.get("name").and_then(|n| n.as_str())
                                {
                                    if let Some(lib_path) = library_name_to_path(name_val) {
                                        let abs =
                                            minecraft_path_buf.join("libraries").join(&lib_path);
                                        let norm = normalize(&abs);
                                        if !load_library_paths.contains(&norm) {
                                            load_library_paths.push(norm);
                                        }
                                    }
                                }
                            }
                        }

                        // 提取参数信息
                        if let Some(args_obj) = root.get("arguments") {
                            if let Some(game_arr) = args_obj.get("game").and_then(|v| v.as_array())
                            {
                                for el in game_arr {
                                    if let Some(s) = el.as_str() {
                                        load_game_params.push(s.trim().to_string());
                                    }
                                }
                            }
                            if let Some(jvm_arr) = args_obj.get("jvm").and_then(|v| v.as_array()) {
                                for el in jvm_arr {
                                    if let Some(s) = el.as_str() {
                                        load_jvm_params.push(s.trim().to_string());
                                    }
                                }
                            }
                        }

                        // 提取minecraftArguments
                        if let Some(mca) = root.get("minecraftArguments").and_then(|v| v.as_str()) {
                            for token in mca.split_whitespace() {
                                load_game_params.push(token.trim().to_string());
                            }
                        }

                        // 提取mainClass
                        if let Some(main_class) = root.get("mainClass").and_then(|v| v.as_str()) {
                            load_main_class = Some(main_class.to_string());
                        }
                    }
                }
            } else {
                debug!("load_path is not a directory: {}", load_path.display());
            }
        }
    }

    // 读取 version.json 为两种格式：结构化的 VersionJson 和原始的 serde_json::Value
    let version_json_content = std::fs::read_to_string(&version_path).with_context(|| {
        format!(
            "Failed to read version json from {}",
            version_path.display()
        )
    })?;

    let version_json_value: serde_json::Value = serde_json::from_str(&version_json_content)
        .context("Failed to parse version json as value")?;

    let mut version_json: VersionJson =
        serde_json::from_str(&version_json_content).context("Failed to parse version json")?;

    let parent_version: Option<String> = version_json.parent_version.clone();

    // 确定基础 Minecraft 版本（用于定位游戏 JAR）
    // 优先级：传入的 minecraft_version > parent_version (inheritsFrom) > patches 中的 game 版本 > version_name
    // 注意：不能因为 versions/<minecraft_version>/<minecraft_version>.jar 存在就信任该值。
    // 例如整合包目录名 "SH的乌龟世界" 或旧实例遗留在 UI 配置里的过期 id 会指向一个与
    // 当前实例无关的 JAR（如 Java 26 字节码的原版包），导致 classpath 混入错误版本。
    // 只有形如 "1.20.1"/"26.2"/"24w14a" 的合法版本号才值得信任。
    let supplied_version_is_trusted =
        !minecraft_version.is_empty() && is_plausible_minecraft_version(minecraft_version);

    let base_minecraft_version = if supplied_version_is_trusted {
        println!(
            "使用传入的 minecraft_version 作为基础版本: {}",
            minecraft_version
        );
        minecraft_version.to_string()
    } else if let Some(parent) = &parent_version {
        println!("使用 inheritsFrom 作为基础版本: {}", parent);
        parent.clone()
    } else if let Some(patch_version) = detect_minecraft_version_from_patches(&version_json_value) {
        println!("从 patches 中提取基础版本: {}", patch_version);
        patch_version
    } else {
        println!("使用 version_name 作为基础版本: {}", version_name);
        version_name.to_string()
    };

    if let Some(parent) = &version_json.parent_version {
        let parent_path = minecraft_path_buf
            .join("versions")
            .join(parent)
            .join(format!("{}.json", parent));

        let parent_json: VersionJson = serde_json::from_reader(
            std::fs::File::open(parent_path).context("Failed to open parent json")?,
        )?;

        if version_json.asset_index.is_none() {
            version_json.asset_index = parent_json.asset_index;
        }

        // 继承 parent 的 logging（日志 XML 文件通常在父版本目录下）
        if version_json.logging.is_none() {
            version_json.logging = parent_json.logging;
        }

        // 合并 parent 的 libraries（当前版本的库优先，避免重复）
        // 使用 (group, artifact) 作为 key 来去重
        use std::collections::HashSet;
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for lib in &version_json.libraries {
            let parts: Vec<&str> = lib.name.split(':').collect();
            if parts.len() >= 2 {
                seen.insert((parts[0].to_string(), parts[1].to_string()));
            }
        }
        for parent_lib in parent_json.libraries {
            let parts: Vec<&str> = parent_lib.name.split(':').collect();
            if parts.len() >= 2 {
                let key = (parts[0].to_string(), parts[1].to_string());
                if !seen.contains(&key) {
                    version_json.libraries.push(parent_lib);
                    seen.insert(key);
                }
            }
        }

        // ===== 关键修复：合并 parent 的 arguments（参照 HMCL）=====
        // Forge/NeoForge 的 -p (--module-path)、-cp、--add-opens 等关键参数
        // 都定义在 parent 的 arguments.jvm 中。必须以 parent 的参数为基础进行合并。
        match (&version_json.arguments, &parent_json.arguments) {
            (None, Some(_)) => {
                // 当前版本没有 arguments，直接使用 parent 的
                version_json.arguments = parent_json.arguments;
            }
            (Some(_), Some(_)) => {
                // ===== 修复：两边都有 arguments 时，以 parent 的 jvm 参数为主 =====
                // 因为 Forge/NeoForge 的 -p、-cp 等关键模块路径参数定义在 parent (Forge) JSON 中，
                // 当前版本（子版本/整合包版本）的 jvm 参数通常为空或不重要。
                // 如果 parent 有 jvm 参数，就用 parent 的；只有 parent 没有时才用当前的。
                let parent_has_jvm = parent_json
                    .arguments
                    .as_ref()
                    .and_then(|a| a.jvm.as_ref())
                    .is_some();
                if parent_has_jvm {
                    if let Some(cur_args) = version_json.arguments.as_mut() {
                        cur_args.jvm = parent_json.arguments.as_ref().and_then(|a| a.jvm.clone());
                    }
                }
                // game 参数同理
                let parent_has_game = parent_json
                    .arguments
                    .as_ref()
                    .and_then(|a| a.game.as_ref())
                    .is_some();
                if parent_has_game {
                    if let Some(cur_args) = version_json.arguments.as_mut() {
                        cur_args.game = parent_json.arguments.as_ref().and_then(|a| a.game.clone());
                    }
                }
            }
            _ => {}
        }
        // 老版本 format 兼容：如果 parent 使用 minecraft_arguments 而不是 arguments.game
        if version_json.minecraft_arguments.is_none() && parent_json.minecraft_arguments.is_some() {
            version_json.minecraft_arguments = parent_json.minecraft_arguments;
        }

        // 如果 parent 的 mainClass 是 bootstraplauncher（Forge 1.17+），使用 parent 的 mainClass
        // 因为当前版本的 mainClass 可能还是 net.minecraft.client.main.Main
        if version_json.main_class.to_lowercase().contains("minecraft")
            && !parent_json.main_class.to_lowercase().contains("minecraft")
        {
            version_json.main_class = parent_json.main_class;
        }
    }

    if loadType != "0" && load_main_class.is_some() {
        version_json.main_class = load_main_class.unwrap();
    }

    let os_info = os_info::get();
    let is_windows = os_info.os_type() == Type::Windows;
    let is_macos = os_info.os_type() == Type::Macos;
    let _is_linux = os_info.os_type() == Type::Linux;

    fn check_rules(rules: &[Rule], os_info: &os_info::Info) -> bool {
        let mut allowed = true;
        for rule in rules {
            let mut rule_matched = false;

            if let Some(os_rule) = &rule.os {
                let os_match = match os_rule.name.as_deref() {
                    Some("windows") => os_info.os_type() == Type::Windows,
                    Some("osx") => os_info.os_type() == Type::Macos,
                    Some("linux") => os_info.os_type() == Type::Linux,
                    _ => true,
                };

                let version_match = if let Some(version_pattern) = &os_rule.version {
                    let re = Regex::new(version_pattern).unwrap();
                    re.is_match(&os_info.version().to_string())
                } else {
                    true
                };

                rule_matched = os_match && version_match;
            }

            match rule.action.as_str() {
                "allow" => allowed = rule_matched,
                "disallow" => allowed = !rule_matched,
                _ => (),
            }
        }
        allowed
    }

    let format_path = |p: PathBuf| -> String { p.to_string_lossy().replace('\\', "/") };

    // 处理可能为空的认证字段
    let effective_token = if auth_token.trim().is_empty() {
        "0"
    } else {
        auth_token
    };
    let (user_type, user_properties) = launch_auth_identity(effective_token, yggdrasil_api);

    // 生成 UUID v3 (基于名称) 从玩家名
    fn generate_uuid_from_name(name: &str) -> String {
        // 使用简单的 FNV 1a 哈希 + 常量填充生成 UUID v3
        let mut hash: [u8; 16] = [0u8; 16];
        let mut state: u64 = 14695981039346656037u64; // FNV-1a offset basis
        for b in name.as_bytes() {
            state ^= *b as u64;
            state = state.wrapping_mul(1099511628211); // FNV prime
        }
        // 把 64-bit state 散布到 16 字节
        let mut s = state;
        for i in 0..8 {
            hash[i] = (s & 0xff) as u8;
            s >>= 8;
        }
        // 用另一个哈希填充后 8 字节
        state = state.wrapping_mul(1099511628211);
        state ^= name.len() as u64;
        for i in 8..16 {
            hash[i] = (state & 0xff) as u8;
            state >>= 8;
        }
        // 设置版本 3 (基于名称)
        hash[6] = (hash[6] & 0x0f) | 0x30;
        // 设置 RFC 4122 变体
        hash[8] = (hash[8] & 0x3f) | 0x80;
        format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]),
            u16::from_be_bytes([hash[4], hash[5]]),
            u16::from_be_bytes([hash[6], hash[7]]),
            u16::from_be_bytes([hash[8], hash[9]]),
            u64::from_be_bytes([hash[10], hash[11], hash[12], hash[13], hash[14], hash[15], 0, 0])
                >> 16
        )
    }

    let effective_uuid = if uuid.trim().is_empty() {
        generate_uuid_from_name(player_name)
    } else {
        uuid.to_string()
    };

    let replace_placeholders = |s: &str| -> String {
        let result = s
            .replace("${auth_player_name}", player_name)
            .replace("${auth_session}", &effective_token)
            .replace("${auth_access_token}", &effective_token)
            .replace("${auth_uuid}", &effective_uuid)
            .replace("${user_properties}", user_properties)
            .replace("${version_name}", version_name)
            .replace(
                "${natives_directory}",
                &format_path(
                    minecraft_path_buf
                        .join("versions")
                        .join(version_name)
                        .join(format!("{}-natives", version_name)),
                ),
            )
            .replace("${game_directory}", &format_path(game_directory.clone()))
            .replace(
                "${assets_root}",
                &format_path(minecraft_path_buf.join("assets")),
            )
            .replace(
                "${assets_index_name}",
                &version_json
                    .asset_index
                    .as_ref()
                    .map(|a| a.id.trim())
                    .unwrap_or(&String::new()),
            )
            .replace("${user_type}", user_type)
            .replace("${version_type}", "RTL");

        // 将剩余的${}格式参数转换为{}格式
        let re = Regex::new(r"\$\{[^}]+\}").unwrap();
        re.replace_all(&result, "{}").to_string()
    };

    let mut class_path_entries: Vec<String> = version_json
        .libraries
        .iter()
        .filter_map(|lib| {
            // 检查规则或serverreq标志
            let allowed = check_rules(&lib.rules, &os_info) || lib.serverreq;
            if !allowed {
                return None;
            }
            let artifact_path = if !lib.downloads.classifiers.is_empty() {
                let classifier = lib
                    .natives
                    .get(match os_info.os_type() {
                        Type::Windows => "windows",
                        Type::Macos => "osx",
                        Type::Linux => "linux",
                        _ => return None,
                    })
                    .and_then(|s| s.strip_prefix("natives-"));
                lib.downloads
                    .classifiers
                    .get(classifier?)
                    .map(|a| minecraft_path_buf.join("libraries").join(&a.path))
            } else if lib.downloads.artifact.is_some() {
                lib.downloads
                    .artifact
                    .as_ref()
                    .map(|a| minecraft_path_buf.join("libraries").join(&a.path))
            } else {
                // 没有 downloads 信息：根据 name 构建路径
                // 兼容低版本 Forge / 旧格式 JSON，它们的库没有 downloads 字段
                if let Some(path) = library_name_to_path(&lib.name) {
                    Some(minecraft_path_buf.join("libraries").join(path))
                } else {
                    None
                }
            };

            // 对于 -SNAPSHOT 版本，实际 jar 文件名可能带 timestamp/buildNumber
            // 例如: mixin-0.7.4-SNAPSHOT.jar -> mixin-0.7.4-20171010.121826-8.jar
            // 无论是否有 downloads 信息，都需要做这个检查
            let resolved_path = match artifact_path {
                Some(mut full) => {
                    if !full.exists() && lib.name.contains("-SNAPSHOT") {
                        if let Some(parent) = full.parent() {
                            if let Ok(entries) = std::fs::read_dir(parent) {
                                let name_parts: Vec<&str> = lib.name.split(':').collect();
                                if name_parts.len() >= 2 {
                                    for entry in entries.flatten() {
                                        let p = entry.path();
                                        if let Some(ext) = p.extension() {
                                            if ext == "jar" {
                                                if let Some(fname) = p.file_name() {
                                                    let s = fname.to_string_lossy();
                                                    if s.starts_with(name_parts[1]) {
                                                        full = p;
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
                    Some(full)
                }
                None => None,
            };

            resolved_path.map(|p| format_path(p))
        })
        .collect();

    // 验证 classpath 中每个文件是否存在（启动前检查，避免"找不到主类"）
    {
        let mut missing: Vec<String> = Vec::new();
        for entry in &class_path_entries {
            let pb = PathBuf::from(entry);
            if !pb.exists() {
                missing.push(entry.clone());
            } else if let Ok(meta) = std::fs::metadata(&pb) {
                if meta.len() == 0 {
                    missing.push(format!("{} (空文件)", entry));
                }
            }
        }
        if !missing.is_empty() {
            let error_msg = format!("classpath 中以下文件不存在或为空:\n{}", missing.join("\n"));
            error!("classpath 中以下文件不存在或为空:\n{}", missing.join("\n"));
            return Err(anyhow::anyhow!("{}", error_msg));
        }
    }

    // 处理load_library_paths，实现库版本替换的逻辑
    if !load_library_paths.is_empty() {
        // 存储需要移除的原版库的索引
        let mut indices_to_remove: Vec<usize> = Vec::new();

        // 存储已处理的load库的标识信息
        let mut processed_load_libs: Vec<(String, String, String)> = Vec::new();

        // 首先检查load_library_paths中的库
        for load_path in &load_library_paths {
            // 解析load库的路径
            if let Some((load_group, load_artifact, load_version)) = parse_library_path(load_path) {
                // 检查是否已经处理过相同group和artifact的库
                let already_processed = processed_load_libs
                    .iter()
                    .any(|(g, a, _)| g == &load_group && a == &load_artifact);

                if !already_processed {
                    // 标记为已处理
                    processed_load_libs.push((
                        load_group.clone(),
                        load_artifact.clone(),
                        load_version.clone(),
                    ));

                    // 检查原版库中是否有相同group和artifact的库
                    for (i, vanilla_path) in class_path_entries.iter().enumerate() {
                        if let Some((vanilla_group, vanilla_artifact, vanilla_version)) =
                            parse_library_path(vanilla_path)
                        {
                            // 如果group和artifact相同，比较版本号
                            if vanilla_group == load_group && vanilla_artifact == load_artifact {
                                // 如果load库的版本更高，标记原版库为需要移除
                                if compare_versions(&load_version, &vanilla_version) {
                                    indices_to_remove.push(i);
                                    println!(
                                        "替换库: {} (原版版本: {}) -> {} (load版本: {})",
                                        vanilla_path, vanilla_version, load_path, load_version
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 移除需要替换的原版库（从后往前移，避免索引变化）
        indices_to_remove.sort();
        indices_to_remove.dedup();
        for i in indices_to_remove.into_iter().rev() {
            class_path_entries.remove(i);
        }

        // 添加load_library_paths中的库
        for p in &load_library_paths {
            if !class_path_entries.contains(p) {
                class_path_entries.push(p.clone());
            }
        }
    }

    // 对于使用 Java Module System 的启动方式（Forge 1.17+ / NeoForge），
    // fmlloader 会从 libraryDirectory 自己查找 minecraft client jar
    // 不再需要把原版 minecraft jar 加入 classpath，否则会被 Module System 识别为
    // automatic module 与 minecraft 模块冲突（"Modules minecraft and _1._18._2 export package ..."）。
    //
    // 判断依据：
    // 1. libraries 中包含 fmlloader 或 bootstraplauncher（对于 Forge 版本名直接启动的情况）
    // 2. 主类中包含 bootstraplauncher / modlauncher / neo / fml / ModLauncher（
    //    对于通过 loadType/loadName 机制加载 Forge 配置但 version_name 是原版版本名的情况）
    //
    // 注意：Fabric、LiteLoader 等不使用模块系统，需要游戏JAR在classpath中
    let main_class_lc = version_json.main_class.to_lowercase();
    let uses_module_system =
        version_json.libraries.iter().any(|lib| {
            lib.name.contains(":fmlloader:") || lib.name.contains(":bootstraplauncher:")
        }) || main_class_lc.contains("bootstraplauncher")
            || main_class_lc.contains("modlauncher")
            || main_class_lc.contains("cpw.mods")
            || main_class_lc.contains("fml");

    // Fabric 检测：主类包含 fabric 或 libraries 中包含 fabric loader
    let is_fabric = main_class_lc.contains("fabric")
        || version_json
            .libraries
            .iter()
            .any(|lib| lib.name.contains(":fabricloader:"));

    // 如果是Fabric，确保游戏JAR被添加到classpath（覆盖模块系统检测）
    if is_fabric {
        println!("检测到Fabric加载器，确保游戏JAR在classpath中");
    }

    // 确定要添加的游戏JAR路径
    let game_jar_path = if loadType != "0" && !loadName.is_empty() {
        // 对于有加载器的情况，需要检查是否真的在加载器目录中有独立的JAR
        // 对于像Fabric这样的加载器，JAR通常就是原版JAR，所以可能还是用原版路径
        let loader_jar = minecraft_path_buf
            .join("versions")
            .join(loadName)
            .join(format!("{}.jar", loadName));

        // 使用基础 Minecraft 版本来定位原版 JAR
        // 这样可以处理整合包的情况，其中 version_name 是整合包名，但 JAR 在原版目录中
        let vanilla_jar = minecraft_path_buf
            .join("versions")
            .join(&base_minecraft_version)
            .join(format!("{}.jar", base_minecraft_version));

        if vanilla_jar.exists() {
            println!(
                "使用原版游戏JAR (基础版本 {}): {}",
                base_minecraft_version,
                vanilla_jar.display()
            );
            vanilla_jar
        } else if loader_jar.exists() {
            println!("使用加载器独立游戏JAR: {}", loader_jar.display());
            loader_jar
        } else {
            // 两个都不存在，尝试使用 version_name 作为最后的回退
            let fallback_jar = minecraft_path_buf
                .join("versions")
                .join(version_name)
                .join(format!("{}.jar", version_name));
            println!(
                "原版和加载器JAR都不存在，尝试回退路径: {}",
                fallback_jar.display()
            );
            fallback_jar
        }
    } else {
        // 对于原版，使用标准路径
        minecraft_path_buf
            .join("versions")
            .join(&base_minecraft_version)
            .join(format!("{}.jar", base_minecraft_version))
    };

    // 验证游戏JAR文件是否存在
    if !game_jar_path.exists() {
        return Err(anyhow::anyhow!(
            "游戏JAR文件不存在: {}\n请确保游戏版本已正确下载",
            game_jar_path.display()
        ));
    }

    // 只有使用模块系统的加载器才不添加游戏JAR
    // Fabric、LiteLoader、Quilt等都需要游戏JAR在classpath中
    // Fabric特别需要游戏JAR，即使检测到模块系统特征也要添加
    if !uses_module_system || is_fabric {
        let game_jar = format_path(game_jar_path.clone());
        println!(
            "添加游戏JAR到classpath: {} (is_fabric: {})",
            game_jar, is_fabric
        );
        class_path_entries.push(game_jar);
    } else {
        println!("使用模块系统，跳过添加游戏JAR到classpath");
    }

    // ===== 关键修复：classpath 去重 =====
    // 部分启动器生成的 version.json 的 libraries 列表本身含重复项（例如
    // PCL 的 NeoForge 实例把原版库与加载器库重复拼接），重复 jar 会令
    // NeoForge 的 UnionFileSystem 抛出 "Duplicate key" 而启动失败，这里统一去重。
    let mut seen_classpath: HashSet<String> = HashSet::new();
    class_path_entries.retain(|p| seen_classpath.insert(p.to_lowercase()));

    // ===== 预构建 class_path（用于 ${classpath} 占位符替换）=====
    // 必须在 jvm_args_from_version 之前构建，因为 jvm_args_from_version 的 replace_placeholders
    // 会使用这个变量。
    let cp_sep_for_replace = if is_windows { ";" } else { ":" };
    let class_path: String = class_path_entries.join(cp_sep_for_replace);

    // 年轻代不应大于最大堆的三分之一；否则小内存设备即使 Xmx 合法也会
    // 因固定的 `-Xmn768m` 产生不合理的初始化压力。
    let young_generation_mb = 768_u64.min((effective_max_memory_mb / 3).max(256));
    let mut args: Vec<String> = vec![
        format!("-Xmn{young_generation_mb}m"),
        format!("-Xmx{effective_max_memory_mb}m"),
    ];

    // ===== 参照 HMCL：修正 load_jvm_params 中的 neoforge/forge jar 路径 =====
    // Forge JSON 中的 -p 参数可能包含 "neoforge-${version_name}.jar" 这样的引用，
    // 这些需要被替换为完整的 jar 文件路径，否则 module-path 不正确
    let mut neoforge_jar = String::new();
    let mut forge_jar = String::new();
    if !loadName.is_empty() && loadType != "0" {
        let library_dir = minecraft_path_buf.join("libraries");
        // 尝试从 libraries 目录找到 neoforge/forge 的 jar
        // 典型路径: libraries/net/neoforged/neoforge/{version}/neoforge-{version}.jar
        // 典型路径: libraries/net/minecraftforge/forge/{version}/forge-{version}.jar
        for lib in &version_json.libraries {
            if lib.name.contains("neoforge") || lib.name.contains(":forge:") {
                let parts: Vec<&str> = lib.name.split(':').collect();
                if parts.len() >= 3 {
                    let group = parts[0].replace('.', "/");
                    let artifact = parts[1];
                    let version = parts[2];
                    let jar_name = format!("{}-{}.jar", artifact, version);
                    let jar_path = library_dir
                        .join(&group)
                        .join(artifact)
                        .join(version)
                        .join(&jar_name);
                    let jar_path_str = format_path(jar_path);
                    if lib.name.contains("neoforge") && neoforge_jar.is_empty() {
                        neoforge_jar = jar_path_str.clone();
                    }
                    if lib.name.contains(":forge:") && forge_jar.is_empty() {
                        forge_jar = jar_path_str.clone();
                    }
                }
            }
        }
        // 如果 libraries 里找不到，尝试在 versions/{loadName} 目录查找
        if neoforge_jar.is_empty() && forge_jar.is_empty() {
            let load_dir = minecraft_path_buf.join("versions").join(&loadName);
            if let Ok(entries) = std::fs::read_dir(&load_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jar") {
                        let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
                        if fname.contains("neoforge") {
                            neoforge_jar = format_path(path);
                        } else if fname.contains("forge") && forge_jar.is_empty() {
                            forge_jar = format_path(path);
                        }
                    }
                }
            }
        }
    }

    // 对 load_jvm_params 中的每个参数进行 neoforge/forge jar 路径修正
    let mut load_jvm_params_fixed: Vec<String> = Vec::new();
    for p in &load_jvm_params {
        let mut fixed = p.clone();
        // 替换 neoforge-${version_name}.jar 样式的引用
        if !neoforge_jar.is_empty() {
            // 例如: ".../neoforge-${version_name}.jar" 或 "neoforge-${version_name}"
            fixed = fixed.replace("neoforge-${version_name}.jar", &neoforge_jar);
            fixed = fixed.replace("neoforge-${version_name}", &neoforge_jar);
            // 其他可能的形式
            if fixed.contains("neoforge")
                && fixed.contains(".jar")
                && !std::path::Path::new(&fixed).exists()
            {
                // 如果参数看起来是 jar 路径但不存在，尝试替换为 neoforge jar
                if fixed.contains(&version_name) {
                    fixed = neoforge_jar.clone();
                }
            }
        }
        if !forge_jar.is_empty() {
            fixed = fixed.replace("forge-${version_name}.jar", &forge_jar);
            fixed = fixed.replace("forge-${version_name}", &forge_jar);
        }
        load_jvm_params_fixed.push(fixed);
    }

    let extra_before_cp: Vec<String> = if !load_jvm_params_fixed.is_empty() {
        load_jvm_params_fixed
            .iter()
            .map(|p| clean_param_spaces(p))
            .collect()
    } else {
        Vec::new()
    };

    let extra_after_cp: Vec<String> = load_game_params;

    // ===== 关键修复：处理 version_json.arguments.jvm 中的参数（参照 HMCL）=====
    // Forge/NeoForge 自己的配置文件中会指定 -p (--module-path)、-cp、--add-opens 等参数
    // -p 参数包含模块化 JAR (bootstraplauncher, securejarhandler 等)
    // 这些参数是 Forge 精心设计的，必须完整保留并正确加入到启动命令中
    let mut jvm_args_from_version: Vec<String> = Vec::new();
    if let Some(arguments) = &version_json.arguments {
        if let Some(jvm_args) = &arguments.jvm {
            let library_dir = format_path(minecraft_path_buf.join("libraries"));
            let classpath_sep = if is_windows { ";" } else { ":" };

            // 构建 neoforge/forge loader jar 的正确路径
            // 典型的 neoforge jar 路径: libraries/net/neoforged/neoforge/{version}/neoforge-{version}.jar
            // 典型的 forge jar 路径: libraries/net/minecraftforge/forge/{version}/forge-{version}.jar
            let mut neoforge_jar_path = String::new();
            let mut forge_jar_path = String::new();
            // 在 libraries 列表中查找 neoforge/forge 的 artifact 路径（不论 loadType/loadName）
            for lib in &version_json.libraries {
                if lib.name.contains("neoforge") || lib.name.contains(":forge:") {
                    if let Some(artifact) = &lib.downloads.artifact {
                        let jar_path =
                            format_path(minecraft_path_buf.join("libraries").join(&artifact.path));
                        if lib.name.contains("neoforge") && neoforge_jar_path.is_empty() {
                            neoforge_jar_path = jar_path.clone();
                        }
                        if lib.name.contains(":forge:") && forge_jar_path.is_empty() {
                            forge_jar_path = jar_path.clone();
                        }
                    } else {
                        // 没有 downloads.artifact，根据 name 构建路径
                        let parts: Vec<&str> = lib.name.split(':').collect();
                        if parts.len() >= 3 {
                            let group_path = parts[0].replace('.', "/");
                            let artifact_name = parts[1];
                            let artifact_version = parts[2];
                            let jar_name = format!("{}-{}.jar", artifact_name, artifact_version);
                            let rel_path = format!(
                                "{}/{}/{}/{}",
                                group_path, artifact_name, artifact_version, jar_name
                            );
                            let jar_path =
                                format_path(minecraft_path_buf.join("libraries").join(&rel_path));
                            if lib.name.contains("neoforge") && neoforge_jar_path.is_empty() {
                                neoforge_jar_path = jar_path.clone();
                            }
                            if lib.name.contains(":forge:") && forge_jar_path.is_empty() {
                                forge_jar_path = jar_path.clone();
                            }
                        }
                    }
                }
            }
            // 如果 libraries 里找不到，还可以在 versions/{loadName}/libraries 里找（NeoForge 安装器布局）
            if (neoforge_jar_path.is_empty() && forge_jar_path.is_empty()) && !loadName.is_empty() {
                let load_lib_dir = minecraft_path_buf
                    .join("versions")
                    .join(&loadName)
                    .join("libraries");
                if let Ok(entries) = std::fs::read_dir(&load_lib_dir) {
                    for entry in entries.flatten() {
                        if let Some(fname) = entry.path().file_name().and_then(|f| f.to_str()) {
                            if fname.contains("neoforge")
                                && fname.ends_with(".jar")
                                && neoforge_jar_path.is_empty()
                            {
                                neoforge_jar_path = format_path(entry.path());
                            } else if fname.contains("forge")
                                && fname.ends_with(".jar")
                                && forge_jar_path.is_empty()
                            {
                                forge_jar_path = format_path(entry.path());
                            }
                        }
                    }
                }
            }

            // 占位符替换函数（参照 HMCL）
            let replace_placeholders = |s: &str| -> String {
                let mut result = s.to_string();
                result = result.replace("${classpath_separator}", classpath_sep);
                result = result.replace("${library_directory}", &library_dir);
                result = result.replace("${version_name}", &version_name);
                result = result.replace("${launcher_name}", "RTLauncher");
                result = result.replace("${launcher_version}", env!("CARGO_PKG_VERSION"));

                // ===== 关键修复：替换 ${classpath} 占位符 =====
                // Forge/NeoForge 在 -cp 参数的 value 里用 ${classpath} 来引用整个 classpath
                // 必须替换成我们构建的 class_path（即所有 libraries 的路径拼接）
                result = result.replace("${classpath}", &class_path);

                // 处理 neoforge/forge jar 的特殊替换
                // Forge JSON 中可能包含类似 "neoforge-${version_name}.jar" 的引用
                // 需要替换为完整路径
                if !neoforge_jar_path.is_empty() {
                    // 查找类似 ".../neoforge-${version_name}.jar" 的模式并替换
                    if result.contains("neoforge-${version_name}.jar") {
                        result = result.replace("neoforge-${version_name}.jar", &neoforge_jar_path);
                    } else if result.contains("neoforge-${version_name}") {
                        result = result.replace("neoforge-${version_name}", &neoforge_jar_path);
                    }
                    // 也可能是 neoforge-X.Y.Z.jar 这种相对路径形式
                    if result.contains("neoforge")
                        && result.contains(".jar")
                        && !result.contains(&library_dir)
                        && !result.contains('/')
                        && !result.contains('\\')
                    {
                        // 看起来像个简单文件名，把它替换为完整路径
                        result = neoforge_jar_path.clone();
                    }
                }
                if !forge_jar_path.is_empty() {
                    if result.contains("forge-${version_name}.jar") {
                        result = result.replace("forge-${version_name}.jar", &forge_jar_path);
                    } else if result.contains("forge-${version_name}") {
                        result = result.replace("forge-${version_name}", &forge_jar_path);
                    }
                    if result.contains("forge")
                        && result.contains(".jar")
                        && !result.contains(&library_dir)
                        && !result.contains('/')
                        && !result.contains('\\')
                    {
                        // 简单文件名 -> 完整路径
                        result = forge_jar_path.clone();
                    }
                }

                // 将剩余的${}格式参数转换为{}格式（与 game 参数替换保持一致）
                let re = Regex::new(r"\$\{[^}]+\}").unwrap();
                re.replace_all(&result, "{}").to_string()
            };

            for arg in jvm_args {
                match arg {
                    JvmArgument::String(s) => {
                        let replaced = replace_placeholders(s);
                        if !replaced.trim().is_empty() {
                            jvm_args_from_version.push(replaced.trim().to_string());
                        }
                    }
                    JvmArgument::Object { rules, value } => {
                        if check_rules(rules, &os_info) {
                            match value {
                                serde_json::Value::String(s) => {
                                    let replaced = replace_placeholders(s);
                                    if !replaced.trim().is_empty() {
                                        jvm_args_from_version.push(replaced.trim().to_string());
                                    }
                                }
                                serde_json::Value::Array(arr) => {
                                    for item in arr {
                                        if let Some(s) = item.as_str() {
                                            let replaced = replace_placeholders(s);
                                            if !replaced.trim().is_empty() {
                                                jvm_args_from_version
                                                    .push(replaced.trim().to_string());
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    if is_macos {
        args.push("-XstartOnFirstThread".to_string());
    }

    if os_info.architecture().map_or(false, |a| a.contains("x86")) {
        args.push("-Xss1M".to_string());
    }

    if is_windows {
        args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string());
    }

    args.extend(vec![
        "-XX:+UseG1GC".to_string(),
        "-XX:-UseAdaptiveSizePolicy".to_string(),
        "-XX:-OmitStackTraceInFastThrow".to_string(),
        "-Djdk.lang.Process.allowAmbiguousCommands=true".to_string(),
        "-Dfml.ignoreInvalidMinecraftCertificates=True".to_string(),
        "-Dfml.ignorePatchDiscrepancies=True".to_string(),
    ]);

    // 只在Java 9+才添加模块系统相关参数
    // 检测Java版本，避免在旧版本Java上添加不支持的参数导致启动失败
    let java_major_version = get_java_major_version(java_path);
    let java_version_num: u32 = java_major_version.parse().unwrap_or(8);

    if java_version_num >= 9 {
        // 标准的Java模块访问权限（适用于Java 9+）
        // 只针对 ALL-UNNAMED 开放权限，因为 Forge 会自己创建 ModuleLayer
        // 我们不需要让 Java 原生模块系统介入，否则会导致模块冲突
        args.extend(vec![
            "--add-opens=java.base/java.lang=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.lang.invoke=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.lang.reflect=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.net=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.nio=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.util=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.util.concurrent=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/java.util.concurrent.atomic=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/jdk.internal.misc=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/sun.nio.ch=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/sun.security.util=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/sun.security.x509=ALL-UNNAMED".to_string(),
            "--add-opens=java.base/sun.net.www.protocol.jar=ALL-UNNAMED".to_string(),
            "--add-exports=java.base/sun.nio.ch=ALL-UNNAMED".to_string(),
            "--add-exports=java.base/jdk.internal.misc=ALL-UNNAMED".to_string(),
            "--add-exports=java.base/sun.security.util=ALL-UNNAMED".to_string(),
            "--add-exports=java.desktop/sun.awt=ALL-UNNAMED".to_string(),
            "--add-exports=java.desktop/sun.java2d=ALL-UNNAMED".to_string(),
            // 不添加针对具名模块的 --add-opens 和 --add-reads
            // 因为这些模块在 Java 启动时还不存在，Forge 会自己创建 ModuleLayer
        ]);
    }

    if let Some(logging) = &version_json.logging {
        if let Some(client) = &logging.client {
            // 先在当前版本目录找日志文件，找不到则尝试父版本目录
            let mut log_file_path = minecraft_path_buf
                .join("versions")
                .join(version_name)
                .join(&client.file.id);
            if !log_file_path.exists() {
                if let Some(pv) = &parent_version {
                    let parent_file = minecraft_path_buf
                        .join("versions")
                        .join(pv)
                        .join(&client.file.id);
                    if parent_file.exists() {
                        log_file_path = parent_file;
                    }
                }
            }
            if log_file_path.is_file() {
                let log_path = format_path(log_file_path);
                // Windows 上 Java 的 URI.toURL() 需要 file:/// 协议前缀
                // 否则会把盘符 D: 误认为 URL scheme
                if cfg!(windows) {
                    args.push(format!("-Dlog4j.configurationFile=file:///{}", log_path));
                } else {
                    args.push(format!("-Dlog4j.configurationFile=file:{}", log_path));
                }
            } else {
                warn!(
                    "未找到 Minecraft 日志配置文件 {}，跳过 -Dlog4j.configurationFile",
                    log_file_path.display()
                );
            }
        }
    }

    let mut fixed_params = vec![format!(
        "-DlibraryDirectory={}",
        format_path(minecraft_path_buf.join("libraries"))
    )];

    // Linux 和 Windows 必须保留 JVM 自己提供的
    // os.name/os.version/os.arch。LWJGL 会区分大小写检查 "Linux"；
    // 将它改成 "linux" 会令游戏以 -3 退出，Unix 显示为退出码 253。
    // 仅为需要兼容旧版 LWJGL 的 macOS 保留历史兼容值。
    if cfg!(target_os = "macos") {
        fixed_params.push("-Dos.name=Mac OS X".to_string());
    }

    // 确保 -Djava.library.path 参数总是被添加
    // 根据loadType和loadName来决定使用哪个版本名
    // 只在neoforge时使用loadName，其他modloader使用version_name
    // 仅靠 loadName 是否包含 "neoforge" 判断不够可靠：
    // 合并型整合包（如 "PVZ_Survive"）的目录名不含 neoforge 却仍是 NeoForge，
    // PCL 安装的 NeoForge 实例（如 "_ _ _ _ - _ _ _"）mainClass 与 Forge 相同，
    // 且没有 net.neoforged:neoforge 坐标，只带 fancymodloader 库。
    // 需要再从已解析的 version.json 主类/库坐标交叉确认。
    let is_neoforge = loadType != "0"
        && (!loadName.is_empty() && loadName.to_lowercase().contains("neoforge")
            || version_json.main_class.to_lowercase().contains("neoforged")
            || version_json.libraries.iter().any(|lib| {
                let n = lib.name.to_lowercase();
                n.contains("net.neoforged:fancymodloader:")
                    || n.contains("net.neoforged:neoforge:")
                    || n.contains("net.neoforged:fmlloader:")
            }));
    // 对于loadType为1的情况，如果是neoforge，使用loadName；否则使用version_name
    let native_version = if loadType == "1" && is_neoforge {
        &loadName
    } else {
        version_name
    };
    let native_path = format_path(
        minecraft_path_buf
            .join("versions")
            .join(version_name)
            .join(format!("{}-natives", native_version)),
    );
    args.push(format!("-Djava.library.path={}", native_path));

    // 添加neoforge需要的额外系统属性
    if is_neoforge {
        args.push(format!("-Djna.tmpdir={}", native_path));
        args.push(format!(
            "-Dorg.lwjgl.system.SharedLibraryExtractPath={}",
            native_path
        ));
        args.push(format!("-Dio.netty.native.workdir={}", native_path));
    }

    let existing_params: HashSet<String> = version_json
        .arguments
        .iter()
        .flat_map(|a| a.jvm.iter().flatten())
        .filter_map(|arg| match arg {
            JvmArgument::String(s) => Some(s.split('=').next().unwrap().to_string()),
            _ => None,
        })
        .collect();

    for param in fixed_params {
        let key = param.split('=').next().unwrap();
        if !existing_params.contains(key) {
            args.push(param);
        }
    }

    // 处理 authlib-injector + Yggdrasil 第三方验证（LittleSkin 等）
    let effective_authlib_path = if !yggdrasil_api.is_empty() && authlib_injector_path.is_empty() {
        // 用户配置了第三方验证服务器，但没有指定 authlib-injector 路径
        // 自动下载/查找 authlib-injector
        eprintln!(
            "[Launcher] 检测到 Yggdrasil API: {}, 自动获取 authlib-injector...",
            yggdrasil_api
        );
        let downloaded =
            crate::auth::yissadrail::get_or_download_authlib_injector(&minecraft_path_buf);
        if downloaded.is_empty() {
            warn!("[Launcher] 警告: 无法获取 authlib-injector，游戏内皮肤可能无法显示");
        } else {
            info!("[Launcher] 使用 authlib-injector: {}", downloaded);
        }
        downloaded
    } else {
        authlib_injector_path.to_string()
    };

    if !yggdrasil_api.is_empty() {
        if effective_authlib_path.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "第三方登录需要 authlib-injector，但自动下载失败。请检查网络后重试，或在启动设置中指定有效的 authlib-injector.jar 路径。"
            ));
        }

        let authlib_path = PathBuf::from(&effective_authlib_path);
        if !authlib_path.is_file() {
            return Err(anyhow::anyhow!(
                "authlib-injector 文件不存在或不是文件: {}",
                authlib_path.display()
            ));
        }

        args.push(format!(
            "-javaagent:{}={}",
            authlib_path
                .canonicalize()
                .unwrap_or(authlib_path)
                .to_string_lossy(),
            yggdrasil_api.trim()
        ));
        // 对于 LittleSkin 等皮肤站，添加 no-verify 选项避免部分 SSL 问题
        args.push("-Dauthlibinjector.noVerify=true".to_string());
        args.push("-Dauthlibinjector.mojangNamespace=default".to_string());
    }

    if !prefetched_data.is_empty() {
        args.push(format!(
            "-Dauthlibinjector.yggdrasil.prefetched={}",
            prefetched_data
        ));
    }

    // ===== 关键修复：先加入 version_json.arguments.jvm 中的参数（参照 HMCL）=====
    // Forge 自己指定的 -p (--module-path) 和 -cp 参数必须被正确加入
    // 处理 -p 和 -cp 等带空格分隔值的参数
    // 同时去重：避免 extra_before_cp 和 jvm_args_from_version 的参数重复（尤其 `-p`/`-cp`）
    {
        // 先收集已在 extra_before_cp 中的参数键，避免重复
        let mut existing_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let mut ei = 0;
            while ei < extra_before_cp.len() {
                let p = &extra_before_cp[ei];
                let has_value = p.starts_with('-')
                    && ei + 1 < extra_before_cp.len()
                    && !extra_before_cp[ei + 1].starts_with('-');
                if p.starts_with('-') {
                    // 记录 key（如 -p, -cp, --module-path 等）
                    existing_keys.insert(p.clone());
                }
                if has_value {
                    ei += 2;
                } else {
                    ei += 1;
                }
            }
        }

        // 然后添加 jvm_args_from_version 中的参数（跳过已在 extra_before_cp 中的 key）
        let mut i = 0;
        while i < jvm_args_from_version.len() {
            let p = &jvm_args_from_version[i];
            let has_value = p.starts_with('-')
                && i + 1 < jvm_args_from_version.len()
                && !jvm_args_from_version[i + 1].starts_with('-');

            // 去重：如果 key 已存在于 extra_before_cp 中，跳过
            // 但 -p/--module-path、-cp/--class-path 是关键参数，始终优先使用 jvm_args_from_version 中的
            let is_module_or_class_path_key =
                p == "-p" || p == "--module-path" || p == "-cp" || p == "--class-path";
            if !is_module_or_class_path_key && existing_keys.contains(p) {
                if has_value {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }

                args.push(p.clone());
                if has_value {
                    // 对于 -p/--module-path，额外检查其中引用的 JAR 是否存在
                    let value = &jvm_args_from_version[i + 1];
                    if is_module_or_class_path_key {
                        debug!("[HMCL 模式] 使用 Forge 参数: {} 值长度: {}", p, value.len());
                        // 对 -cp/-p 的值做去重，避免 PCL 等生成的 json 里重复的
                        // jar 路径导致 UnionFileSystem "Duplicate key" 崩溃
                        args.push(dedup_path_list(value));
                        // 对于 -p，打印其中的每个路径用于调试
                        if p == "-p" || p == "--module-path" {
                            for piece in value.split(|c| c == ';' || c == ':') {
                                if !piece.trim().is_empty() {
                                    let path_buf = PathBuf::from(piece.trim());
                                    let exists = path_buf.exists();
                                    debug!("  module-path 项: {} (存在: {})", piece.trim(), exists);
                                }
                            }
                        }
                    } else {
                        args.push(value.clone());
                    }
                    i += 2;
                } else {
                    i += 1;
                }
        }
    }

    // 再处理独立加载器 JSON 的参数。version_name 可能指向原版 JSON，而
    // loadName 指向 Forge JSON；这种情况下 Forge 的 -p 只在这里出现。
    println!("处理extra_before_cp，长度: {}", extra_before_cp.len());
    append_loader_jvm_args(&mut args, &extra_before_cp);

    // 将 Wrapper JAR 也加入 classpath（不能用 -jar，否则 Java 会忽略 -cp）

    // 注意：class_path 在处理 jvm_args_from_version 之前已经构建好（用于 ${classpath} 替换）
    // 检查 Forge 是否已经指定了 -cp 参数
    // 如果 Forge 已经指定了 -cp，我们就不要重复添加自己的 classpath
    let forge_has_cp = args.iter().any(|a| a == "-cp" || a == "--class-path");
    let forge_has_module_path = args.iter().any(|a| a == "-p" || a == "--module-path");

    if uses_module_system {
        // 对于使用模块系统的 Forge（如 1.17+）
        // 优先使用 Forge 自己在 arguments.jvm 中指定的 -p 和 -cp 参数
        // 如果 Forge 没有指定 -cp，则使用我们自己构建的 classpath

        if !forge_has_cp {
            // Forge 没有指定 -cp，使用我们自己构建的 classpath
            args.push("-cp".to_string());
            args.push(class_path.clone());
            debug!("[HMCL 模式] 检测到 Forge 模块系统，使用自定义 classpath");
            debug!("  库总数量: {}", class_path_entries.len());
        } else {
            println!("[HMCL 模式] 检测到 Forge 模块系统，使用 Forge 指定的参数启动");
            println!(
                "  Forge 已提供 -p: {}, Forge 已提供 -cp: {}",
                forge_has_module_path, forge_has_cp
            );
        }

        // ===== 关键修复：确保 Java 内部 API 对 unnamed module 开放 =====
        // BootstrapLauncher 在启动时需要用 MethodHandles.lookup() 来访问一些内部字段
        // 如果这些模块没有对 unnamed module 开放，就会报 InaccessibleObjectException
        //
        // 我们已经在上面的固定参数列表添加了大量 --add-opens，这里再确认一下
        // 关键的 module/package 组合（Forge/NeoForge 实际运行需要的）。
        //
        // 如果 Forge 的 arguments.jvm 已经提供了自己的 --add-opens，它们会在
        // jvm_args_from_version 中被加入。但有些老版本的 Forge JSON 没有完整
        // 覆盖所有需要的 package，所以我们在上面的「标准 Java 模块访问权限」中
        // 加了一份兜底。
    } else {
        // 对于不使用模块系统的版本，仅使用传统的 classpath 方式
        if !forge_has_cp {
            args.push("-cp".to_string());
            args.push(class_path.clone());
        }
    }

    if !wrapper_path.is_empty() {
        let wrapper_abs = format_path(PathBuf::from(wrapper_path));
        args.push("-jar".to_string());
        args.push(wrapper_abs);
    }

    let mut game_args_vec: Vec<String> = Vec::new();

    // 处理原版游戏参数
    if let Some(game_args) = version_json
        .arguments
        .as_ref()
        .and_then(|a| a.game.as_ref())
    {
        for arg in game_args {
            match arg {
                serde_json::Value::String(s) => {
                    let replaced = replace_placeholders(s);
                    if !replaced.trim().is_empty() {
                        game_args_vec.push(replaced.trim().to_string());
                    }
                }
                serde_json::Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            let replaced = replace_placeholders(s);
                            if !replaced.trim().is_empty() {
                                game_args_vec.push(replaced.trim().to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    } else if let Some(minecraft_args) = &version_json.minecraft_arguments {
        for arg in minecraft_args.split_whitespace() {
            let replaced = replace_placeholders(arg);
            if !replaced.trim().is_empty() {
                game_args_vec.push(replaced.trim().to_string());
            }
        }
    }

    game_args_vec.extend(vec![
        "--width".to_string(),
        (if window_width.is_empty() {
            "873"
        } else {
            window_width
        })
        .to_string(),
        "--height".to_string(),
        (if window_height.is_empty() {
            "486"
        } else {
            window_height
        })
        .to_string(),
    ]);

    // 修复点2: 不进行去重检查，确保load中game里面的所有参数都被加入总启动参数中
    {
        let mut li = 0;
        while li < extra_after_cp.len() {
            let tok = &extra_after_cp[li];

            // 检查是否是占位符参数（如 ${auth_player_name}）
            let is_placeholder = tok.starts_with("${") && tok.ends_with("}");

            // 只过滤占位符参数，其他参数都保留
            if is_placeholder {
                li += 1;
                continue;
            }
            if !(game_args_vec.contains(tok) && tok.starts_with("--")) {
                game_args_vec.push(tok.clone());
            }
            li += 1;
        }
    }

    // 修复点3: 改进参数转发逻辑
    let mut forwarded_args: Vec<String> = Vec::new();
    let mut filtered_game_args: Vec<String> = Vec::new();
    let mut i = 0;

    while i < game_args_vec.len() {
        let arg = &game_args_vec[i];

        // 特殊处理 --tweakClass 参数
        if arg == "--tweakClass" && i + 1 < game_args_vec.len() {
            forwarded_args.push(arg.clone());
            forwarded_args.push(game_args_vec[i + 1].clone());
            i += 1; // 跳过值
        }
        // 特判：当原版参数中包含--assetsDir时，后面的值一定为minecraft文件夹路径/assets
        else if arg == "--assetsDir" && i + 1 < game_args_vec.len() {
            let assets_path = format_path(minecraft_path_buf.join("assets"));
            forwarded_args.push(arg.clone());
            forwarded_args.push(assets_path);
            i += 1; // 跳过原值
        }
        // 特判：处理 --game-dir 参数，确保游戏目录正确设置
        else if arg == "--game-dir" && i + 1 < game_args_vec.len() {
            let game_dir = format_path(game_directory.clone());
            forwarded_args.push(arg.clone());
            forwarded_args.push(game_dir);
            i += 1; // 跳过原值
        }
        // 处理其他 -- 参数
        else if arg.starts_with("--") {
            forwarded_args.push(arg.clone());
            if i + 1 < game_args_vec.len() && !game_args_vec[i + 1].starts_with("--") {
                forwarded_args.push(game_args_vec[i + 1].clone());
                i += 1;
            }
        }
        // 处理非 -- 参数
        else {
            filtered_game_args.push(arg.clone());
        }
        i += 1;
    }

    // game args = forwarded_args + filtered_game_args
    let mut game_app_args: Vec<String> = Vec::new();
    game_app_args.extend(forwarded_args.iter().cloned());
    game_app_args.extend(filtered_game_args.iter().cloned());

    // 处理option.txt文件
    let option_file_path = game_directory.join("options.txt");

    // 检查并创建option.txt文件
    if !option_file_path.exists() {
        if let Some(parent) = option_file_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&option_file_path, "").ok();
    }

    // 读取并修改option.txt文件
    if let Ok(content) = std::fs::read_to_string(&option_file_path) {
        let mut has_lang = false;
        let mut new_content = String::new();

        for line in content.lines() {
            if line.trim().starts_with("lang:") {
                new_content.push_str(&format!("lang:zh_cn\n"));
                has_lang = true;
            } else {
                new_content.push_str(&format!("{}\n", line));
            }
        }

        if !has_lang {
            new_content.push_str("lang:zh_cn\n");
        }

        std::fs::write(&option_file_path, new_content).ok();
    }

    // 如果有 Wrapper 则用 Wrapper 主类包裹原始主类，否则直接使用原始主类
    // 在此之前先注入用户自定义 JVM 参数，允许用户覆盖默认行为（如 GC 参数、系统属性等）
    if !custom_jvm_args.trim().is_empty() {
        let mut raw = custom_jvm_args.to_string();
        // 支持 Windows 风格的换行（\r\n）、Unix 换行（\n）、中文逗号分号统一当作分隔符
        raw = raw
            .replace("\r\n", " ")
            .replace('\r', " ")
            .replace('\n', " ");
        // 用 shell 风格的简单分词：支持空格分隔，支持单/双引号包裹（引号内保留空格）
        let tokens = shell_split(&raw);
        for tok in tokens {
            let trimmed = tok.trim().to_string();
            if !trimmed.is_empty() {
                // 最大堆由“最大内存”设置统一控制。允许自定义参数在末尾再次
                // 写入 -Xmx/-Xms 会覆盖上面的安全上限，重新引入 OOM 风险。
                if is_heap_size_argument(&trimmed) {
                    warn!(
                        "已忽略自定义 JVM 堆内存参数 {}；请在启动设置的最大内存中调整",
                        trimmed
                    );
                    continue;
                }
                // 这两个参数会把新版客户端的用户属性请求强制导向 Mojang。
                // 对 Yggdrasil/LittleSkin 账户，它们会绕过 authlib-injector 并导致
                // `/player/attributes` 返回 401；官方账户不受影响，保留原行为。
                if !yggdrasil_api.trim().is_empty()
                    && (trimmed.starts_with("-Dminecraft.api.env=")
                        || trimmed.starts_with("-Dminecraft.api.location="))
                {
                    warn!(
                        "第三方账户已忽略会覆盖 authlib-injector 的 JVM 参数: {}",
                        trimmed
                    );
                    continue;
                }
                args.push(trimmed);
            }
        }
    }

    if !wrapper_path.is_empty() {
        args.push(version_json.main_class.clone());
    } else {
        args.push(version_json.main_class.clone());
    }
    args.extend(game_app_args);

    // 调试: 分条打印参数，便于排查
    debug!("=== 启动参数列表 ({} 项) ===", args.len());
    for (i, a) in args.iter().enumerate() {
        debug!("  [{}] {}", i, a);
    }
    debug!("=== 参数列表结束 ===");

    println!("{}", args.join(" "));
    Ok(args)
}

/// 判断 Mojang arguments 中的一组规则是否允许当前参数。
///
/// RTLauncher 当前没有启用 demo、Quick Play 等可选 feature，因此 feature
/// 的实际值统一按 false 处理。这样 `is_demo_user: true` 对应的 `--demo`
/// 不会被误加到正常启动参数中。
pub(super) fn launcher_rules_allow(rules: Option<&serde_json::Value>) -> bool {
    let Some(rules) = rules.and_then(|value| value.as_array()) else {
        return true;
    };

    let mut allowed = false;
    for rule in rules {
        let os_matches = match rule.get("os").and_then(|value| value.as_object()) {
            Some(os) => match os.get("name").and_then(|value| value.as_str()) {
                Some("windows") => cfg!(windows),
                Some("osx") => cfg!(target_os = "macos"),
                Some("linux") => cfg!(target_os = "linux"),
                Some(_) => false,
                None => true,
            },
            None => true,
        };

        let features_match = rule
            .get("features")
            .and_then(|value| value.as_object())
            .map(|features| {
                features
                    .values()
                    .all(|expected| expected.as_bool() == Some(false))
            })
            .unwrap_or(true);

        if os_matches && features_match {
            match rule.get("action").and_then(|value| value.as_str()) {
                Some("allow") => allowed = true,
                Some("disallow") => allowed = false,
                _ => {}
            }
        }
    }

    allowed
}
