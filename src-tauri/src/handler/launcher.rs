use serde::Deserialize;
use std::{
    collections::{HashSet, HashMap},
    path::PathBuf,
};
use std::env::consts::OS;
use os_info::Type;
use anyhow::Context;
use regex::Regex;
use std::{
    process::Command,
    thread,
};

/// 为离线玩家生成稳定的 UUID v3（基于玩家名称）
fn offline_uuid(player_name: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // 使用 "OfflinePlayer:" + name 的 MD5 风格哈希生成确定性 UUID
    // 简化版：用两次哈希生成 128bit
    let input = format!("OfflinePlayer:{}", player_name);
    let mut h1 = DefaultHasher::new();
    input.hash(&mut h1);
    let hi = h1.finish();
    let mut h2 = DefaultHasher::new();
    format!("{}:salt", input).hash(&mut h2);
    let lo = h2.finish();
    // 设置版本位 (version 3) 和 variant 位
    let hi = (hi & 0xFFFFFFFF_FFFF0FFF) | 0x00000000_00003000; // version 3
    let lo = (lo & 0x3FFFFFFF_FFFFFFFF) | 0x80000000_00000000; // variant 10
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (hi >> 32) as u32,
        (hi >> 16) as u16 & 0xFFFF,
        hi as u16 & 0xFFFF,
        (lo >> 48) as u16 & 0xFFFF,
        lo & 0x0000FFFFFFFFFFFF
    )
}

/// 检查是否是合法 UUID 格式
fn is_valid_uuid(s: &str) -> bool {
    // 支持 xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx 或无连字符 32位 hex
    let trimmed = s.replace('-', "");
    trimmed.len() == 32 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionJson {
    arguments: Option<Arguments>,
    main_class: String,
    libraries: Vec<Library>,
    #[serde(rename = "inheritsFrom")]
    parent_version: Option<String>,
    logging: Option<Logging>,
    minecraft_arguments: Option<String>,
    asset_index: Option<AssetIndex>,
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

#[derive(Debug, Deserialize)]
struct Arguments {
    jvm: Option<Vec<JvmArgument>>,
    game: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JvmArgument {
    String(String),
    Object { rules: Vec<Rule>, value: serde_json::Value },
}

#[derive(Debug, Deserialize)]
struct Rule {
    #[serde(rename = "action")]
    action: String,
    #[serde(default)]
    os: Option<OsRule>,
}

#[derive(Debug, Deserialize)]
struct OsRule {
    name: Option<String>,
    arch: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Library {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
    #[serde(default)]
    natives: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
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

pub struct LauncherConfig {
    pub minecraft_path: PathBuf,
    pub java_path: PathBuf,
    pub wrapper_path: PathBuf,
    pub launcher_version: String,
    pub max_memory: String,
}

pub fn run_command(args: Vec<String>, javaPath: PathBuf, MCPath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // 检查 Java 路径是否存在
    if !javaPath.exists() {
        return Err(format!("Java 路径不存在: {}", javaPath.display()).into());
    }

    // 校验 java_path 不是一个 .jar 文件
    if let Some(ext) = javaPath.extension() {
        if ext.eq_ignore_ascii_case("jar") {
            return Err(format!(
                "Java 路径指向了一个 .jar 文件而非 Java 可执行文件: {}\n请设置为 java 或 javaw 可执行文件的路径，例如 /usr/bin/java",
                javaPath.display()
            ).into());
        }
    }

    // 在 Unix 系统上检查并修复执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&javaPath)
            .map_err(|e| format!("无法读取 Java 文件信息: {}", e))?;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            println!("Java 缺少执行权限，正在修复: {}", javaPath.display());
            let mut new_perms = permissions.clone();
            new_perms.set_mode(permissions.mode() | 0o755);
            std::fs::set_permissions(&javaPath, new_perms)
                .map_err(|e| format!("无法设置 Java 执行权限: {}", e))?;
        }
    }

    // 确保工作目录存在
    if !MCPath.exists() {
        std::fs::create_dir_all(&MCPath)
            .map_err(|e| format!("无法创建游戏目录 {}: {}", MCPath.display(), e))?;
    }

    let mut command = match OS {
        "windows" | "linux" | "macos" => Command::new(&javaPath),
        _ => return Err("不支持的操作系统".to_string().into()),
    };
    command.current_dir(&MCPath);
    command.args(&args);

    match command.spawn() {
        Ok(child) => {
            let pid = child.id();
            println!("游戏启动成功，进程ID: {}", pid);
            // 在后台线程中等待进程结束，不阻塞前端
            thread::spawn(move || {
                let mut child = child;
                match child.wait() {
                    Ok(status) => println!("游戏进程 {} 已结束，退出状态: {}", pid, status),
                    Err(e) => println!("等待游戏进程 {} 时出错: {}", pid, e),
                }
            });
            Ok(())
        }
        Err(e) => {
            let msg = format!(
                "游戏启动失败 (Java: {}): {}",
                javaPath.display(),
                e
            );
            println!("{}", msg);
            Err(msg.into())
        }
    }
}
#[tauri::command]
pub fn build_jvm_arguments(
    minecraft_path: &str,
    java_path: &str,
    wrapper_path: &str,
    max_memory: &str,
    version_name: &str,
    player_name: &str,
    auth_token: &str,
    uuid: &str,
    authlib_injector_path: &str,
    yggdrasil_api: &str,
    prefetched_data: &str,
    loadType: &str,
    loadName: &str,
    window_width: &str,
    window_height: &str
) -> Result<String, String> {
    build_jvm_arguments_inner(
        minecraft_path, java_path, wrapper_path, max_memory, version_name,
        player_name, auth_token, uuid, authlib_injector_path, yggdrasil_api,
        prefetched_data, loadType, loadName, window_width, window_height,
    ).map_err(|e| e.to_string())
}

fn build_jvm_arguments_inner(
    minecraft_path: &str,
    java_path: &str,
    wrapper_path: &str,
    max_memory: &str,
    version_name: &str,
    player_name: &str,
    auth_token: &str,
    uuid: &str,
    authlib_injector_path: &str,
    yggdrasil_api: &str,
    prefetched_data: &str,
    loadType: &str,
    loadName: &str,
    window_width: &str,
    window_height: &str
) -> anyhow::Result<String> {
    let minecraft_path_buf = PathBuf::from(minecraft_path);

    // 如果 uuid 为空或不合法，根据玩家名生成离线 UUID
    let uuid = if uuid.is_empty() || !is_valid_uuid(uuid) {
        let generated = offline_uuid(player_name);
        println!("[启动器] UUID 无效 (\"{}\"), 已根据玩家名生成: {}", uuid, generated);
        generated
    } else {
        uuid.to_string()
    };
    let uuid = uuid.as_str();

    let version_path = minecraft_path_buf
        .join("versions")
        .join(version_name)
        .join(format!("{}.json", version_name));
    
    let mut load_library_paths: Vec<String> = Vec::new();
    let mut load_jvm_params: Vec<String> = Vec::new();
    let mut load_game_params: Vec<String> = Vec::new();
    
    let normalize = |p: &PathBuf| p.to_string_lossy().replace('\\', "/");
    
    if loadType != "0" {
        let load_path = minecraft_path_buf
            .join("versions")
            .join(loadName);
            
        if loadType == "1" {
            if load_path.is_dir() {
                for entry in std::fs::read_dir(&load_path).context("Failed to read load_path dir")? {
                    let entry = entry.context("Failed to read dir entry")?;
                    let path = entry.path();
                    if path.extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                    {
                        let value: serde_json::Value = serde_json::from_reader(
                            std::fs::File::open(&path)
                                .with_context(|| format!("Failed to open {}", path.display()))?
                        )?;
                        
                        let root: &serde_json::Value = if let Some(vinfo) = value.get("versionInfo") {
                            vinfo
                        } else {
                            &value
                        };
                        
                        if let Some(main_class) = root.get("mainClass").and_then(|v| v.as_str()) {
                            println!("mainClass: {}", main_class);
                            load_jvm_params.push(main_class.to_string());
                        }
                        
                        // 修复点1: 不再合并参数，保持独立元素
                        if let Some(mca) = root.get("minecraftArguments").and_then(|v| v.as_str()) {
                            println!("minecraftArguments: {}", mca);
                            for token in mca.split_whitespace() {
                                load_game_params.push(token.trim().to_string());
                            }
                        } else if let Some(args_obj) = root.get("arguments") {
                            let mut game_vals = Vec::new();
                            let mut jvm_vals = Vec::new();
                            
                            if let Some(game_arr) = args_obj.get("game").and_then(|v| v.as_array()) {
                                for el in game_arr {
                                    if let Some(s) = el.as_str() {
                                        let trimmed = s.trim();
                                        game_vals.push(trimmed.to_string());
                                        load_game_params.push(trimmed.to_string());
                                    }
                                }
                            }
                            
                            if let Some(jvm_arr) = args_obj.get("jvm").and_then(|v| v.as_array()) {
                                for el in jvm_arr {
                                    if let Some(s) = el.as_str() {
                                        let trimmed = s.trim();
                                        jvm_vals.push(trimmed.to_string());
                                        load_jvm_params.push(trimmed.to_string());
                                    }
                                }
                            }
                            
                            println!("arguments.game: {:?}", game_vals);
                            println!("arguments.jvm: {:?}", jvm_vals);
                        }
                        
                        if let Some(libraries) = root.get("libraries").and_then(|v| v.as_array()) {
                            for lib in libraries {
                                if let Some(downloads) = lib.get("downloads") {
                                    if let Some(artifact) = downloads.get("artifact") {
                                        if let Some(path_str) = artifact.get("path").and_then(|p| p.as_str()) {
                                            let abs = minecraft_path_buf.join("libraries").join(path_str);
                                            let norm = normalize(&abs);
                                            println!("library artifact path: {}", abs.display());
                                            load_library_paths.push(norm.clone());
                                            
                                            if let Some(name) = lib.get("name").and_then(|n| n.as_str()) {
                                                if name.starts_with("net.minecraftforge:forge") {
                                                    if let Some(folder) = abs.parent() {
                                                        if folder.is_dir() {
                                                            for jf in std::fs::read_dir(folder)? {
                                                                let jf = jf?;
                                                                let jfpath = jf.path();
                                                                if jfpath.extension().and_then(|s| s.to_str()) == Some("jar") {
                                                                    println!("forge jar: {}", jfpath.display());
                                                                    load_library_paths.push(normalize(&jfpath));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if let Some(classifiers) = downloads.get("classifiers").and_then(|v| v.as_object()) {
                                        for art in classifiers.values() {
                                            if let Some(path_str) = art.get("path").and_then(|p| p.as_str()) {
                                                let abs = minecraft_path_buf.join("libraries").join(path_str);
                                                let norm = normalize(&abs);
                                                println!("library classifier path: {}", abs.display());
                                                load_library_paths.push(norm);
                                            }
                                        }
                                    }
                                } else {
                                    if let (Some(name_val), Some(_url_val)) = (
                                        lib.get("name").and_then(|n| n.as_str()),
                                        lib.get("url").and_then(|u| u.as_str()),
                                    ) {
                                        let mpath = name_val.replace(':', "/") + ".jar";
                                        let abs = minecraft_path_buf.join("libraries").join(&mpath);
                                        let norm = normalize(&abs);
                                        println!("library artifact path: {}", abs.display());
                                        load_library_paths.push(norm);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                println!("load_path is not a directory: {}", load_path.display());
            }
        } else {
            if load_path.is_dir() {
                for entry in std::fs::read_dir(&load_path).context("Failed to read load_path dir")? {
                    let entry = entry.context("Failed to read dir entry")?;
                    let path = entry.path();
                    if path.extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                    {
                        let content = std::fs::read_to_string(&path)
                            .with_context(|| format!("Failed to read file {}", path.display()))?;
                        println!("Content of {}:\n{}", path.display(), content);
                    }
                }
            } else {
                println!("load_path is not a directory: {}", load_path.display());
            }
        }
    }
    
    let mut version_json: VersionJson = serde_json::from_reader(
        std::fs::File::open(version_path).context("Failed to open version json")?
    ).context("Failed to parse version json")?;
    
    if let Some(parent) = &version_json.parent_version {
        let parent_path = minecraft_path_buf
            .join("versions")
            .join(parent)
            .join(format!("{}.json", parent));
            
        let parent_json: VersionJson = serde_json::from_reader(
            std::fs::File::open(parent_path).context("Failed to open parent json")?
        )?;
        
        if version_json.asset_index.is_none() {
            version_json.asset_index = parent_json.asset_index;
        }
    }
    
    if loadType != "0" && !load_jvm_params.is_empty() {
        version_json.main_class = load_jvm_params[0].clone();
    }
    
    let os_info = os_info::get();
    let is_windows = os_info.os_type() == Type::Windows;
    let is_macos = os_info.os_type() == Type::Macos;
    let is_linux = os_info.os_type() == Type::Linux;
    
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
                _ => ()
            }
        }
        allowed
    }
    
    let format_path = |p: PathBuf| -> String {
        p.to_string_lossy().replace('\\', "/")
    };
    
    let replace_placeholders = |s: &str| -> String {
        let result = s.replace("${auth_player_name}", player_name)
         .replace("${auth_session}", uuid)
         .replace("${auth_access_token}", auth_token)
         .replace("${auth_uuid}", uuid)
         .replace("${version_name}", version_name)
         .replace("${natives_directory}", &format_path(
             minecraft_path_buf
                 .join("versions")
                 .join(version_name)
                 .join(format!("{}-natives", version_name))
         ))
         .replace("${game_directory}", &format_path(
             minecraft_path_buf
                 .join("instance")
                 .join(version_name)
         ))
         .replace("${assets_root}", &format_path(
             minecraft_path_buf.join("assets")
         ))
         .replace("${assets_index_name}", 
             &version_json.asset_index.as_ref().map(|a| a.id.trim()).unwrap_or(&String::new()))
         .replace("${user_type}", "msa")
         .replace("${version_type}", "RTL");
        
        // 将剩余的${}格式参数转换为{}格式
        let re = Regex::new(r"\$\{[^}]+\}").unwrap();
        re.replace_all(&result, "{}").to_string()
    };
    
    let mut class_path_entries: Vec<String> = version_json.libraries
        .iter()
        .filter_map(|lib| {
            if !check_rules(&lib.rules, &os_info) {
                return None;
            }
            let artifact_path = if !lib.downloads.classifiers.is_empty() {
                let classifier = lib.natives.get(match os_info.os_type() {
                    Type::Windows => "windows",
                    Type::Macos => "osx",
                    Type::Linux => "linux",
                    _ => return None,
                }).and_then(|s| s.strip_prefix("natives-"));
                lib.downloads.classifiers.get(classifier?)
                    .map(|a| minecraft_path_buf.join("libraries").join(&a.path))
            } else {
                lib.downloads.artifact.as_ref()
                    .map(|a| minecraft_path_buf.join("libraries").join(&a.path))
            };
            artifact_path.map(|p| format_path(p))
        })
        .collect();
    
    if !load_library_paths.is_empty() {
        for p in &load_library_paths {
            if !class_path_entries.contains(p) {
                class_path_entries.push(p.clone());
            }
        }
    }
    
    let vanilla_jar = format_path(
        minecraft_path_buf
            .join("versions")
            .join(version_name)
            .join(format!("{}.jar", version_name))
    );
    class_path_entries.push(vanilla_jar);
    
    let mut args: Vec<String> = vec![
        "-Xmn768m".to_string(),
        format!("-Xmx{}m", max_memory),
    ];
    
    let extra_before_cp: Vec<String> = if load_jvm_params.len() > 1 {
        load_jvm_params[1..].to_vec()
    } else {
        Vec::new()
    };
    
    let extra_after_cp: Vec<String> = load_game_params;
    
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
    
    if let Some(logging) = &version_json.logging {
        if let Some(client) = &logging.client {
            let log_path = format_path(
                minecraft_path_buf
                    .join("versions")
                    .join(version_name)
                    .join(&client.file.id)
            );
            args.push(format!("-Dlog4j.configurationFile={}", log_path));
        }
    }
    
    let fixed_params = vec![
        // 新版 macOS 的 os.name 返回 "Mac OS" 而非 "Mac OS X"，旧版 LWJGL 不识别
        // 强制设为 "Mac OS X" 以确保 LWJGL 正确识别平台
        if OS == "macos" {
            "-Dos.name=Mac OS X".to_string()
        } else {
            format!("-Dos.name={}", os_info.os_type())
        },
        format!("-Dos.version={}", os_info.version()),
        format!("-Djava.library.path={}", format_path(minecraft_path_buf
                 .join("versions")
                 .join(version_name)
                 .join(format!("{}-natives", version_name)))),
        format!("-DlibraryDirectory={}", format_path(minecraft_path_buf.join("libraries"))),
    ];
    
    let existing_params: HashSet<String> = version_json.arguments
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
    
    if !authlib_injector_path.is_empty() && !yggdrasil_api.is_empty() {
        args.push(format!("-javaagent:{}={}", authlib_injector_path, yggdrasil_api));
    }
    
    if !prefetched_data.is_empty() {
        args.push(format!("-Dauthlibinjector.yggdrasil.prefetched={}", prefetched_data));
    }
    
    // 将 Wrapper JAR 也加入 classpath（不能用 -jar，否则 Java 会忽略 -cp）
    if !wrapper_path.is_empty() {
        let wrapper_abs = format_path(PathBuf::from(wrapper_path));
        if !class_path_entries.contains(&wrapper_abs) {
            class_path_entries.push(wrapper_abs);
        }
    }

    let sep = if is_windows { ";" } else { ":" };
    let class_path = class_path_entries.join(sep);
    args.push("-cp".to_string());
    args.push(class_path);
    
    {
        let mut ei = 0;
        while ei < extra_before_cp.len() {
            let p = &extra_before_cp[ei];
            let has_value = p.starts_with('-')
                && ei + 1 < extra_before_cp.len()
                && !extra_before_cp[ei + 1].starts_with('-');
                
            let already_exists = args.contains(p) || {
                let key = p.split('=').next().unwrap_or(p);
                args.iter().any(|a| a.split('=').next().unwrap_or(a) == key)
            };
            
            if already_exists {
                ei += if has_value { 2 } else { 1 };
                continue;
            }
            
            args.push(p.clone());
            if has_value {
                args.push(extra_before_cp[ei + 1].clone());
                ei += 2;
            } else {
                ei += 1;
            }
        }
    }
    
    let mut game_args_vec: Vec<String> = Vec::new();
    if let Some(game_args) = version_json.arguments.as_ref().and_then(|a| a.game.as_ref()) {
        let mut iter = game_args.iter().filter_map(|v| v.as_str());
        while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
            game_args_vec.push(replace_placeholders(a).trim().to_string());
            game_args_vec.push(replace_placeholders(b).trim().to_string());
        }
    } else if let Some(minecraft_args) = &version_json.minecraft_arguments {
        for arg in minecraft_args.split(' ') {
            game_args_vec.push(replace_placeholders(arg).trim().to_string());
        }
    }
    
    game_args_vec.extend(vec![
        "--width".to_string(),
        (if window_width.is_empty() { "873" } else { window_width }).to_string(),
        "--height".to_string(),
        (if window_height.is_empty() { "486" } else { window_height }).to_string(),
    ]);
    
    // 修复点2: 优化参数去重逻辑，保留关键参数
    {
        let mut vanilla_keys: HashSet<String> = HashSet::new();
        let mut vi = 0;
        while vi < game_args_vec.len() {
            if game_args_vec[vi].starts_with("--") {
                // 收集所有--开头的参数
                let key = &game_args_vec[vi];
                vanilla_keys.insert(key.clone());
                
                if vi + 1 < game_args_vec.len() && !game_args_vec[vi + 1].starts_with("--") {
                    vi += 1;
                }
            }
            vi += 1;
        }
        
        let mut filtered_load: Vec<String> = Vec::new();
        let mut li = 0;
        while li < extra_after_cp.len() {
            let tok = &extra_after_cp[li];
            
            // 检查是否是占位符参数（如 ${auth_player_name}）
            let is_placeholder = tok.starts_with("${") && tok.ends_with("}");
            
            // 检查是否与原版参数重复
            if is_placeholder || (tok.starts_with("--") && vanilla_keys.contains(tok)) {
                if is_placeholder {
                    println!("去重: load参数 {} 是占位符，已忽略", tok);
                } else {
                    println!("去重: load参数 {} 与原版参数重复，已忽略", tok);
                }
                li += 1;
                // 如果有值，也跳过
                if li < extra_after_cp.len() && !extra_after_cp[li].starts_with("--") && !extra_after_cp[li].starts_with("${") {
                    li += 1;
                }
                continue;
            }
            
            filtered_load.push(tok.clone());
            li += 1;
        }
        game_args_vec.extend(filtered_load);
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
    let instance_dir = minecraft_path_buf
        .join("instance")
        .join(version_name);
    let option_file_path = instance_dir.join("options.txt");
    
    // 检查并创建option.txt文件
    if !option_file_path.exists() {
        if let Some(parent) = option_file_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&option_file_path, "").ok();
    }
    
    // 读取并修改option.txt文件
    if let Ok(mut content) = std::fs::read_to_string(&option_file_path) {
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
    if !wrapper_path.is_empty() {
        args.push("oolloo.jlw.Wrapper".to_string());
        args.push(version_json.main_class.clone());
    } else {
        args.push(version_json.main_class.clone());
    }
    args.extend(game_app_args);
    
    // 调试: 分条打印参数，便于排查
    println!("=== 启动参数列表 ({} 项) ===", args.len());
    for (i, a) in args.iter().enumerate() {
        println!("  [{}] {}", i, a);
    }
    println!("=== 参数列表结束 ===");
    
    let arg = args.join(" ");
    println!("{}", arg);
    run_command(args, PathBuf::from(java_path), minecraft_path_buf.clone())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(arg)
}
