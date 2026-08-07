use crate::downloader::concurrent_download::{self, DownloadTask};
use crate::handler::config::{get_java_download_dir, get_launcher_paths_config};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use zip::ZipArchive;
pub struct LoaderInstallerConfig {
    pub installer_jar_path: PathBuf,
    pub java_executable_path: PathBuf,
    pub mc_version: String,        
    pub mc_version_id: String,     
    pub library_mirrors: Vec<String>, 
}
/// 将 Java 解析为可执行文件的绝对路径，避免后续检查时把 PATH 中的命令名 `java` 误判为文件缺失。
fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn find_system_java() -> Option<PathBuf> {
    let executable = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };

    if let Some(java_home) = env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join(executable);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(executable))
            .find(|candidate| is_executable_file(candidate))
    })
}

fn runtime_java_major(runtime_dir: &Path) -> Option<i32> {
    let release = [
        runtime_dir.join("release"),
        runtime_dir.join("Contents").join("Home").join("release"),
    ]
    .into_iter()
    .find_map(|path| fs::read_to_string(path).ok())?;
    let version = release
        .lines()
        .find_map(|line| line.strip_prefix("JAVA_VERSION="))?
        .trim_matches('"');
    let parts: Vec<&str> = version.split('.').collect();
    if parts.first().copied() == Some("1") {
        parts.get(1)?.parse().ok()
    } else {
        parts.first()?.parse().ok()
    }
}

fn runtime_java_executable(runtime_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let candidates = vec![runtime_dir.join("bin").join("java.exe")];
    #[cfg(not(target_os = "windows"))]
    let candidates = vec![
        runtime_dir.join("bin").join("java"),
        runtime_dir
            .join("Contents")
            .join("Home")
            .join("bin")
            .join("java"),
    ];

    candidates
        .into_iter()
        .find(|candidate| is_executable_file(candidate))
}

fn required_java_major_for_mc(mc_version: &str) -> i32 {
    // 25w 快照仍使用 Java 21；26.x（以及 26w 快照）已切换到
    // java-runtime-epsilon / Java 25。
    if let Some(snapshot_year) = mc_version
        .split_once('w')
        .and_then(|(year, _)| year.parse::<i32>().ok())
    {
        return if snapshot_year >= 26 { 25 } else { 21 };
    }
    let parts: Vec<&str> = mc_version.split('.').collect();
    let major = parts
        .first()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(1);
    // Minecraft 已改用以年份为基础的版本号（例如 26.2）。
    if major >= 26 {
        return 25;
    }
    if major >= 25 {
        return 21;
    }
    let minor = parts
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    if minor <= 16 {
        8
    } else if minor == 17 {
        16
    } else if minor <= 20 && !(minor == 20 && patch >= 5) {
        17
    } else {
        21
    }
}
pub fn pick_java_executable(mc_version: &str) -> String {
    let required = required_java_major_for_mc(mc_version);
    println!(
        "[JavaPicker] MC {} 需要 Java {}+，正在从配置中查找...",
        mc_version, required
    );
    let launcher_config = get_launcher_paths_config();
    let java_paths = launcher_config.java_paths;
    let java_installations = launcher_config.java_installations;
    let selected_java = launcher_config.selected_java_path;
    if !java_installations.is_empty() {
        let exact_key = required.to_string();
        if let Some(info) = java_installations.get(&exact_key) {
            if is_executable_file(Path::new(&info.path)) {
                println!("[JavaPicker] 使用精确匹配 Java {}: {}", required, info.path);
                return info.path.clone();
            }
        }
        let mut best: Option<(i32, String)> = None;
        for info in java_installations.values() {
            if info.major_version >= required {
                if let Some((cur, _)) = best {
                    if info.major_version < cur {
                        best = Some((info.major_version, info.path.clone()));
                    }
                } else {
                    best = Some((info.major_version, info.path.clone()));
                }
            }
        }
        if let Some((v, p)) = best {
            if is_executable_file(Path::new(&p)) {
                println!("[JavaPicker] 使用兼容 Java {}: {}", v, p);
                return p;
            }
        }
        for info in java_installations.values() {
            if is_executable_file(Path::new(&info.path)) {
                println!(
                    "[JavaPicker] 回退到已安装 Java {}: {}",
                    info.major_version, info.path
                );
                return info.path.clone();
            }
        }
    }
    for p in &java_paths {
        if is_executable_file(Path::new(p)) {
            println!("[JavaPicker] 使用 java_paths 中的: {}", p);
            return p.clone();
        }
    }
    if !selected_java.is_empty() && is_executable_file(Path::new(&selected_java)) {
        println!("[JavaPicker] 使用 selected_java_path: {}", selected_java);
        return selected_java;
    }
    let java_download_dir = get_java_download_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    if java_download_dir.exists() {
        if let Ok(read_dir) = fs::read_dir(&java_download_dir) {
            let mut compatible = Vec::new();
            for entry in read_dir.flatten() {
                let dir = entry.path();
                if let Some(exe) = runtime_java_executable(&dir) {
                    if let Some(major) = runtime_java_major(&dir) {
                        if major >= required {
                            compatible.push((major, exe));
                        }
                    }
                }
            }
            compatible.sort_by_key(|(major, _)| *major);
            if let Some((major, exe)) = compatible.into_iter().next() {
                let java = exe.to_string_lossy().to_string();
                println!("[JavaPicker] 使用已下载的兼容 Java {}: {}", major, java);
                return java;
            }
        }
    }
    if let Some(java) = find_system_java() {
        let java = java.to_string_lossy().to_string();
        println!("[JavaPicker] 从系统 PATH 找到 Java: {}", java);
        return java;
    }
    println!("[JavaPicker] 未找到可用 Java，回退到系统默认 'java'");
    "java".to_string()
}
fn resolve_maven(coord: &str) -> Result<(String, String)> {
    let (coord_clean, ext) = match coord.rsplit_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (coord, "jar".to_string()),
    };
    let parts: Vec<&str> = coord_clean.split(':').collect();
    if parts.len() < 3 {
        bail!("无效 Maven 坐标: {}", coord);
    }
    let group = parts[0];
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).copied();
    let group_path = group.replace('.', "/");
    let mut file_name = format!("{}-{}", artifact, version);
    if let Some(c) = classifier {
        if !c.is_empty() {
            file_name.push_str(&format!("-{}", c));
        }
    }
    file_name.push_str(&format!(".{}", ext));
    let base_path = format!("{}/{}/{}/", group_path, artifact, version);
    Ok((base_path, file_name))
}
fn maven_to_full_path(root: &Path, coord: &str) -> Result<PathBuf> {
    let (base, fname) = resolve_maven(coord)?;
    let mut path = root.join("libraries");
    for comp in base.split('/').filter(|s| !s.is_empty()) {
        path = path.join(comp);
    }
    path = path.join(fname);
    Ok(path)
}
fn maven_to_relative_path(coord: &str) -> Result<String> {
    let (base, fname) = resolve_maven(coord)?;
    Ok(format!("{}{}", base, fname))
}
fn sha1_of_file(path: &Path) -> Result<String> {
    let mut f =
        fs::File::open(path).with_context(|| format!("打开文件失败: {}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()).to_uppercase())
}
struct RawLibrary {
    name: Option<String>,
    path: Option<String>,
    url: Option<String>,
    sha1: Option<String>,
    _size: Option<u64>,
}
fn parse_library(v: &Value) -> RawLibrary {
    let name = v
        .get("name")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let (path, url, sha1, size) =
        if let Some(artifact) = v.get("downloads").and_then(|d| d.get("artifact")) {
            let path = artifact
                .get("path")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let url = artifact
                .get("url")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let sha1 = artifact
                .get("sha1")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let size = artifact.get("size").and_then(|x| x.as_u64());
            (path, url, sha1, size)
        } else {
            (None, None, None, None)
        };
    if path.is_none() && name.is_some() {
        let nm = name.as_ref().unwrap();
        if let Ok(rel) = maven_to_relative_path(nm) {
            let has_url = url.is_some();
            return RawLibrary {
                name: name.clone(),
                path: Some(rel.clone()),
                url: if has_url {
                    url
                } else {
                    v.get("url").and_then(|x| x.as_str()).map(|s| s.to_string())
                },
                sha1: sha1.or_else(|| {
                    v.get("checksums")
                        .and_then(|c| c.as_array())
                        .and_then(|a| a.first())
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                }),
                _size: size,
            };
        }
    }
    RawLibrary {
        name,
        path,
        url,
        sha1,
        _size: size,
    }
}
struct ProcessorEntry {
    jar: String,
    classpath: Vec<String>,
    sides: Vec<String>,
    arguments: Vec<String>,
    _outputs: Option<Vec<(String, String)>>,
}
fn flatten_arguments(arguments_val: &Value) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    if let Some(obj) = arguments_val.as_object() {
        for (k, v) in obj {
            result.push(k.clone());
            match v {
                Value::String(s) => result.push(s.clone()),
                Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            result.push(s.to_string());
                        }
                    }
                }
                Value::Bool(b) => result.push(b.to_string()),
                Value::Number(n) => result.push(n.to_string()),
                _ => {
                    result.push(v.to_string());
                }
            }
        }
    } else if let Some(arr) = arguments_val.as_array() {
        for item in arr {
            if let Some(s) = item.as_str() {
                result.push(s.to_string());
            } else if let Some(obj) = item.as_object() {
                for (k, v) in obj {
                    result.push(k.clone());
                    if let Some(s) = v.as_str() {
                        result.push(s.to_string());
                    } else if let Some(sub_arr) = v.as_array() {
                        for sub_item in sub_arr {
                            if let Some(s) = sub_item.as_str() {
                                result.push(s.to_string());
                            }
                        }
                    } else {
                        result.push(v.to_string());
                    }
                }
            }
        }
    }
    result
}
fn parse_processors(ip: &Value) -> Vec<ProcessorEntry> {
    let mut result = Vec::new();
    if let Some(arr) = ip.get("processors").and_then(|a| a.as_array()) {
        for p in arr {
            let jar = p
                .get("jar")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let classpath = p
                .get("classpath")
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let sides = p
                .get("sides")
                .and_then(|s| s.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let arguments = p
                .get("args")
                .or_else(|| p.get("arguments"))
                .map(|a| flatten_arguments(a))
                .unwrap_or_default();
            let outputs = p.get("outputs").and_then(|o| o.as_object()).map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });
            result.push(ProcessorEntry {
                jar,
                classpath,
                sides,
                arguments,
                _outputs: outputs,
            });
        }
    }
    result
}
fn read_main_class_from_jar(jar_path: &Path) -> Result<String> {
    let file = fs::File::open(jar_path)
        .with_context(|| format!("打开 processor JAR 失败: {}", jar_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("解析 processor JAR ZIP 失败: {}", jar_path.display()))?;
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .collect();
    let target_idx = names
        .iter()
        .position(|name| name.eq_ignore_ascii_case("META-INF/MANIFEST.MF"))
        .ok_or_else(|| {
            anyhow!(
                "processor JAR 中未找到 META-INF/MANIFEST.MF: {}",
                jar_path.display()
            )
        })?;
    let mut entry = archive.by_index(target_idx)?;
    let mut buf: Vec<u8> = Vec::new();
    entry.read_to_end(&mut buf)?;
    let content = if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&buf[3..]).to_string()
    } else {
        String::from_utf8_lossy(&buf).to_string()
    };
    for line in content.lines() {
        if let Some((k, v)) = line.split_once(": ") {
            if k.trim().eq_ignore_ascii_case("Main-Class") {
                return Ok(v.trim().to_string());
            }
        }
    }
    bail!("MANIFEST.MF 中未找到 Main-Class: {}", jar_path.display())
}
async fn install_legacy_forge(
    archive: &mut ZipArchive<fs::File>,
    _installer_jar_full: &Path,
    root: &Path,
    cfg: &LoaderInstallerConfig,
    _progress_tx: Option<tokio::sync::mpsc::Sender<f64>>,
    _wait_for_original: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<String> {
    fn find_entry_idx(archive: &mut ZipArchive<fs::File>, target: &str) -> Option<usize> {
        for i in 0..archive.len() {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name.eq_ignore_ascii_case(target) {
                return Some(i);
            }
        }
        None
    }
    fn read_entry_bytes(
        archive: &mut ZipArchive<fs::File>,
        idx: usize,
    ) -> std::io::Result<Vec<u8>> {
        let mut entry = archive.by_index(idx)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    }
    fn parse_maven_name(name: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        let group_path = parts[0].replace('.', "/");
        let artifact = parts[1].to_string();
        let version = parts[2].to_string();
        Some((group_path, artifact, version))
    }
    println!("[Legacy] 解析 install_profile.json");
    let ipidx = find_entry_idx(archive, "install_profile.json")
        .ok_or_else(|| anyhow!("[Legacy] 安装器 JAR 中未找到 install_profile.json"))?;
    let ip_buf = read_entry_bytes(archive, ipidx)
        .with_context(|| "[Legacy] 读取 install_profile.json 失败")?;
    let ip_text = if ip_buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&ip_buf[3..]).to_string()
    } else {
        String::from_utf8_lossy(&ip_buf).to_string()
    };
    let ip_model: Value = serde_json::from_str(&ip_text)
        .with_context(|| "[Legacy] 解析 install_profile.json 失败")?;
    println!(
        "[Legacy] install_profile.json 顶层键: {:?}",
        ip_model
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    );
    let version_info_val = if let Some(vi) = ip_model.get("versionInfo") {
        vi.clone()
    } else if ip_model.get("id").is_some() && ip_model.get("libraries").is_some() {
        ip_model.clone()
    } else {
        bail!("[Legacy] install_profile.json 中未找到 versionInfo 或 versionId");
    };
    let id = version_info_val
        .get("id")
        .and_then(|x| x.as_str())
        .map(|s| s.replace("-forge-", "-"))
        .unwrap_or_else(|| format!("{}-Forge{}", cfg.mc_version, cfg.mc_version));
    println!("[Legacy] id = {}", id);
    let mut forge_version_str: Option<String> = None;
    if let Some(libs) = version_info_val.get("libraries").and_then(|l| l.as_array()) {
        for lib in libs {
            if let Some(name) = lib.get("name").and_then(|n| n.as_str()) {
                let parts: Vec<&str> = name.split(':').collect();
                if parts.len() >= 3
                    && parts[0].eq_ignore_ascii_case("net.minecraftforge")
                    && parts[1].eq_ignore_ascii_case("forge")
                {
                    forge_version_str = Some(parts[2].to_string());
                    break;
                }
            }
        }
    }
    let forge_ver = forge_version_str
        .clone()
        .unwrap_or_else(|| cfg.mc_version.clone());
    println!("[Legacy] forge version from libraries: {}", forge_ver);
    let mut universal_jar_idx: Option<usize> = None;
    let try_names = vec![
        format!("forge-{}-universal.jar", forge_ver),
        format!("forge-{}-{}-universal.jar", cfg.mc_version, forge_ver),
        format!("forge-{}-{}-universal.jar", forge_ver, cfg.mc_version),
    ];
    for candidate in &try_names {
        if let Some(idx) = find_entry_idx(archive, candidate) {
            universal_jar_idx = Some(idx);
            println!("[Legacy] 精确匹配 universal jar: {}", candidate);
            break;
        }
    }
    if universal_jar_idx.is_none() {
        for i in 0..archive.len() {
            let name = match archive.by_index(i) {
                Ok(e) => e.name().to_string(),
                Err(_) => continue,
            };
            let lower = name.to_lowercase();
            if lower.contains("forge") && lower.ends_with("-universal.jar") {
                universal_jar_idx = Some(i);
                println!("[Legacy] 宽松匹配 universal jar: {}", name);
                break;
            }
        }
    }
    let universal_jar_idx =
        universal_jar_idx.ok_or_else(|| anyhow!("[Legacy] 未找到 forge-*-universal.jar"))?;
    let target_jar_path = {
        let mut path: Option<PathBuf> = None;
        if let Some(libs) = version_info_val.get("libraries").and_then(|l| l.as_array()) {
            for lib in libs {
                if let Some(name) = lib.get("name").and_then(|n| n.as_str()) {
                    let parts: Vec<&str> = name.split(':').collect();
                    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("net.minecraftforge") {
                        let group_path = parts[0].replace('.', "/");
                        let artifact = parts[1];
                        let version = parts[2];
                        let lib_sub_path = PathBuf::from(group_path)
                            .join(artifact)
                            .join(version)
                            .join(format!("{}-{}.jar", artifact, version));
                        path = Some(root.join("libraries").join(lib_sub_path));
                        break;
                    }
                }
            }
        }
        path.ok_or_else(|| anyhow!("[Legacy] 未能从 libraries 解析 forge 库路径"))?
    };
    println!(
        "[Legacy] 将 universal jar 写入: {}",
        target_jar_path.display()
    );
    if let Some(parent) = target_jar_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    {
        let mut entry = archive.by_index(universal_jar_idx)?;
        let mut file = fs::File::create(&target_jar_path)?;
        std::io::copy(&mut entry, &mut file)?;
    }
    println!("[Legacy] 开始下载 libraries");
    let mc_version = &cfg.mc_version;
    let vanilla_json_path = root
        .join("versions")
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    let mut vanilla_lib_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    if vanilla_json_path.exists() {
        match std::fs::read_to_string(&vanilla_json_path) {
            Ok(content) => {
                if let Ok(vanilla_json) = serde_json::from_str::<Value>(&content) {
                    if let Some(libs) = vanilla_json.get("libraries").and_then(|v| v.as_array()) {
                        for lib in libs {
                            if let Some(name) = lib.get("name").and_then(|n| n.as_str()) {
                                let parts: Vec<&str> = name.split(':').collect();
                                if parts.len() >= 3 {
                                    let artifact = parts[1];
                                    let version = parts[2];
                                    vanilla_lib_names
                                        .insert(format!("{}-{}.jar", artifact, version));
                                    if let Some(natives_obj) =
                                        lib.get("natives").and_then(|n| n.as_object())
                                    {
                                        let os_keys = ["windows", "osx", "linux"];
                                        for os_key in os_keys {
                                            if let Some(classifier) =
                                                natives_obj.get(os_key).and_then(|v| v.as_str())
                                            {
                                                vanilla_lib_names.insert(format!(
                                                    "{}-{}-{}.jar",
                                                    artifact, version, classifier
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }
    if !vanilla_lib_names.is_empty() {
        println!(
            "[Legacy] 从原版 {} 读取到 {} 个库（跳过重复下载）",
            mc_version,
            vanilla_lib_names.len()
        );
    }
    let mut all_libs: Vec<Value> = Vec::new();
    if let Some(libs) = version_info_val.get("libraries").and_then(|v| v.as_array()) {
        all_libs.extend(libs.iter().cloned());
    }
    if let Some(install_obj) = ip_model.get("install").and_then(|v| v.as_object()) {
        if let Some(libs) = install_obj.get("libraries").and_then(|v| v.as_array()) {
            all_libs.extend(libs.iter().cloned());
        }
    }
    if all_libs.is_empty() {
        bail!("[Legacy] install_profile.json 中找不到任何 libraries 字段");
    }
    println!(
        "[Legacy] install_profile 中共 {} 个库条目（含 forge 自身和去重前）",
        all_libs.len()
    );
    let mut download_tasks: Vec<concurrent_download::DownloadTask> = Vec::new();
    let libraries_dir = root.join("libraries");
    let mut seen_file_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let default_mirrors: Vec<&str> = vec![
        "https://bmclapi2.bangbang93.com/maven/",
        "https://files.minecraftforge.net/maven/",
        "https://libraries.minecraft.net/",
        "https://maven.aliyun.com/repository/public/",
        "https://repo.spongepowered.org/maven/",
        "https://maven.neoforged.net/releases/",
        "https://maven.fabricmc.net/",
        "https://repo1.maven.org/maven2/",
    ];
    for lib in &all_libs {
        let name = match lib.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() >= 2
            && parts[0].eq_ignore_ascii_case("net.minecraftforge")
            && parts[1].eq_ignore_ascii_case("forge")
        {
            continue;
        }
        if let Some(clientreq) = lib.get("clientreq").and_then(|v| v.as_bool()) {
            if !clientreq {
                println!("[Legacy] 跳过（clientreq=false）: {}", name);
                continue;
            }
        }
        let (group_path, artifact, version) = match parse_maven_name(name) {
            Some(p) => p,
            None => {
                if parts.len() == 2 {
                    let art = parts[0].to_string();
                    let ver = parts[1].to_string();
                    (format!("net/minecraft"), art.clone(), ver)
                } else {
                    println!("[Legacy] 跳过（无法解析 name）: {}", name);
                    continue;
                }
            }
        };
        let has_natives = lib.get("natives").is_some();
        let custom_url = lib
            .get("url")
            .and_then(|u| u.as_str())
            .map(|u| u.trim_end_matches('/').to_string());
        let mut base_urls: Vec<String> = Vec::new();
        if let Some(cu) = &custom_url {
            base_urls.push(cu.clone());
        }
        for m in &default_mirrors {
            base_urls.push(m.to_string());
        }
        {
            let file_subpath = PathBuf::from(&group_path)
                .join(&artifact)
                .join(&version)
                .join(format!("{}-{}.jar", artifact, version));
            let target_dir = libraries_dir.join(file_subpath.parent().unwrap());
            let file_name = format!("{}-{}.jar", artifact, version);
            let unique_key = format!("main:{}", file_subpath.display());
            if !seen_file_names.insert(unique_key) {
                continue;
            }
            if vanilla_lib_names.contains(&file_name) {
                println!("[Legacy] 跳过（原版已含）: {}", file_name);
                continue;
            }
            let urls: Vec<String> = base_urls
                .iter()
                .map(|base| {
                    format!(
                        "{}/{}/{}/{}/{}",
                        base, group_path, artifact, version, file_name
                    )
                })
                .collect();
            let full_path = target_dir.join(&file_name);
            let needs_download = !full_path.exists()
                || match std::fs::metadata(&full_path) {
                    Ok(m) => m.len() == 0,
                    Err(_) => true,
                };
            if needs_download {
                println!(
                    "[Legacy] 准备下载: {} (尝试 {} 个镜像源)",
                    file_name,
                    urls.len()
                );
                println!(
                    "[Legacy]   → 首个 URL: {}",
                    urls.first().unwrap_or(&"".to_string())
                );
                download_tasks.push(concurrent_download::DownloadTask {
                    file_name: file_name.clone(),
                    target_dir: target_dir.clone(),
                    urls: urls.clone(),
                    sha1: None,
                });
            }
        }
        if has_natives {
            let os_type = match std::env::consts::OS {
                "windows" => Some("windows"),
                "macos" => Some("osx"),
                "linux" => Some("linux"),
                _ => None,
            };
            if let Some(os_name) = os_type {
                let classifier_key = lib
                    .get("natives")
                    .and_then(|n| n.as_object())
                    .and_then(|o| o.get(os_name))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| match os_name {
                        "windows" => "natives-windows",
                        "osx" => "natives-osx",
                        "linux" => "natives-linux",
                        _ => "natives-linux",
                    });
                let file_subpath = PathBuf::from(&group_path)
                    .join(&artifact)
                    .join(&version)
                    .join(format!("{}-{}-{}.jar", artifact, version, classifier_key));
                let target_dir = libraries_dir.join(file_subpath.parent().unwrap());
                let file_name = format!("{}-{}-{}.jar", artifact, version, classifier_key);
                let unique_key = format!("native:{}", file_subpath.display());
                if !seen_file_names.insert(unique_key) {
                    continue;
                }
                if vanilla_lib_names.contains(&file_name) {
                    println!("[Legacy] 跳过（原版已含 natives）: {}", file_name);
                    continue;
                }
                let urls: Vec<String> = base_urls
                    .iter()
                    .map(|base| {
                        format!(
                            "{}/{}/{}/{}/{}",
                            base, group_path, artifact, version, file_name
                        )
                    })
                    .collect();
                let full_path = target_dir.join(&file_name);
                let needs_download = !full_path.exists()
                    || match std::fs::metadata(&full_path) {
                        Ok(m) => m.len() == 0,
                        Err(_) => true,
                    };
                if needs_download {
                    println!(
                        "[Legacy] 准备下载 natives: {} (尝试 {} 个镜像源)",
                        file_name,
                        urls.len()
                    );
                    download_tasks.push(concurrent_download::DownloadTask {
                        file_name: file_name.clone(),
                        target_dir: target_dir.clone(),
                        urls: urls.clone(),
                        sha1: None,
                    });
                }
            }
        }
    }
    println!("[Legacy] 实际需要下载 {} 个库", download_tasks.len());
    if !download_tasks.is_empty() {
        let result = concurrent_download::download_all(download_tasks, None).await;
        println!(
            "[Legacy] 下载完成: 成功 {} / 失败 {}",
            result.success_count,
            result.failures.len()
        );
        for f in &result.failures {
            println!("[Legacy] 失败: {} ({})", f.file_name, f.error);
            if !f.urls_tried.is_empty() {
                println!("[Legacy]   尝试的 URL:");
                for u in &f.urls_tried {
                    println!("[Legacy]     - {}", u);
                }
            }
        }
        if !result.failures.is_empty() {
            println!(
                "[Legacy] 警告: 有 {} 个库下载失败，启动时可能报'找不到主类'",
                result.failures.len()
            );
        }
    }
    let mut final_version_json = version_info_val.clone();
    {
        let obj = final_version_json
            .as_object_mut()
            .ok_or_else(|| anyhow!("[Legacy] VersionInfo 不是 JSON 对象"))?;
        obj.insert("id".to_string(), Value::String(id.clone()));
        if !obj.contains_key("inheritsFrom") {
            obj.insert(
                "inheritsFrom".to_string(),
                Value::String(cfg.mc_version.clone()),
            );
        }
        if !obj.contains_key("mainClass") {
            obj.insert(
                "mainClass".to_string(),
                Value::String("net.minecraft.launchwrapper.Launch".to_string()),
            );
        }
    }
    let versions_dir = root.join("versions").join(&id);
    fs::create_dir_all(&versions_dir).ok();
    let json_path = versions_dir.join(format!("{}.json", id));
    let json_out = serde_json::to_string_pretty(&final_version_json)?;
    fs::write(&json_path, json_out)?;
    println!("[Legacy] version.json 已写入: {}", json_path.display());
    println!("[Legacy] 完成！");
    Ok(id)
}
pub async fn install(
    cfg: &LoaderInstallerConfig,
    mc_dir: &Path,
    progress_tx: Option<tokio::sync::mpsc::Sender<f64>>,
    wait_for_original: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<String> {
    if !cfg.installer_jar_path.exists() {
        bail!(
            "找不到 Loader 安装器 JAR: {}",
            cfg.installer_jar_path.display()
        );
    }
    let root = mc_dir;
    let installer_jar_full = cfg.installer_jar_path.canonicalize()?;
    let archive_file = fs::File::open(&installer_jar_full)
        .with_context(|| format!("打开安装器 JAR 失败: {}", installer_jar_full.display()))?;
    let mut archive = ZipArchive::new(archive_file).context("解析安装器 JAR (ZIP) 失败")?;
    println!("=== installer JAR 内容 (前 50 项) ===");
    for (i, name) in archive.file_names().enumerate().take(50) {
        println!("  [{:3}] {}", i, name);
    }
    println!("=== 共 {} 个文件 ===", archive.len());
    fn find_entry_idx(archive: &mut ZipArchive<fs::File>, target: &str) -> Option<usize> {
        for i in 0..archive.len() {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name.eq_ignore_ascii_case(target) {
                return Some(i);
            }
        }
        None
    }
    fn read_entry_bytes(
        archive: &mut ZipArchive<fs::File>,
        idx: usize,
    ) -> std::io::Result<Vec<u8>> {
        let mut entry = archive.by_index(idx)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    }
    let has_version_json = find_entry_idx(&mut archive, "version.json").is_some();
    if !has_version_json {
        println!("检测到 Legacy 模式（低版本 Forge，无 version.json）");
        return install_legacy_forge(
            &mut archive,
            &installer_jar_full,
            root,
            cfg,
            progress_tx,
            wait_for_original,
        )
        .await;
    }
    if !cfg.java_executable_path.exists() {
        bail!(
            "找不到 Java 可执行文件: {}（现代 Forge/NeoForge 需要 Java 运行 processor）",
            cfg.java_executable_path.display()
        );
    }
    println!("检测到现代模式（有 version.json）");
    println!("解析 version.json");
    let vidx = find_entry_idx(&mut archive, "version.json")
        .ok_or_else(|| anyhow!("安装器 JAR 中未找到 version.json"))?;
    let v_buf = read_entry_bytes(&mut archive, vidx).context("读取 version.json 失败")?;
    let v_text = if v_buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&v_buf[3..]).to_string()
    } else {
        String::from_utf8_lossy(&v_buf).to_string()
    };
    let version_json: Value = serde_json::from_str(&v_text).context("解析 version.json 失败")?;
    let id = version_json
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("version.json 缺少 id 字段"))?
        .to_string();
    let forge_version = id.replace("-forge-", "-");
    // 从 libraries 中解析真正的 forge/neoforge 版本号（用于 maven 路径匹配）
    // id 可能是 "1.21.4-neoforge-26.2.0.9-beta"，但我们真正需要的版本号是 "26.2.0.9-beta"
    let mut real_loader_version: Option<String> = None;
    if let Some(libs) = version_json.get("libraries").and_then(|l| l.as_array()) {
        for lib in libs {
            if let Some(name) = lib.get("name").and_then(|n| n.as_str()) {
                let parts: Vec<&str> = name.split(':').collect();
                if parts.len() >= 3 {
                    if parts[0].eq_ignore_ascii_case("net.neoforged")
                        && parts[1].eq_ignore_ascii_case("neoforge")
                    {
                        real_loader_version = Some(parts[2].to_string());
                        break;
                    }
                    if parts[0].eq_ignore_ascii_case("net.minecraftforge")
                        && parts[1].eq_ignore_ascii_case("forge")
                    {
                        real_loader_version = Some(parts[2].to_string());
                        break;
                    }
                }
            }
        }
    }
    let forge_version = real_loader_version.unwrap_or(forge_version);
    let id = if !id.starts_with(&format!("{}-", cfg.mc_version)) && id != cfg.mc_version {
        let new_id = format!("{}-{}", cfg.mc_version, id);
        println!("修复: 将 id {} 添加 mc 版本前缀 → {}", id, new_id);
        new_id
    } else {
        id
    };
    let mut version_json = version_json;
    if let Some(obj) = version_json.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.clone()));
    }
    println!("解析: id={}, forge_version={}", id, forge_version);
    let versions_dir = root.join("versions").join(&id);
    fs::create_dir_all(&versions_dir).ok();
    let json_path = versions_dir.join(format!("{}.json", id));
    let json_out = serde_json::to_string_pretty(&version_json)?;
    fs::write(&json_path, json_out)?;
    println!("解析 install_profile.json");
    let ipidx = find_entry_idx(&mut archive, "install_profile.json")
        .ok_or_else(|| anyhow!("安装器 JAR 中未找到 install_profile.json"))?;
    let ip_buf = read_entry_bytes(&mut archive, ipidx).context("读取 install_profile.json 失败")?;
    let ip_text = if ip_buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&ip_buf[3..]).to_string()
    } else {
        String::from_utf8_lossy(&ip_buf).to_string()
    };
    let ip_model: Value =
        serde_json::from_str(&ip_text).context("解析 install_profile.json 失败")?;
    if ip_model.is_null() {
        bail!("install_profile.json 内容为空");
    }
    if let Some(obj) = ip_model.as_object() {
        let keys: Vec<&String> = obj.keys().collect();
        println!("  install_profile.json 顶层键: {:?}", keys);
        if let Some(args) = obj.get("arguments") {
            let type_str = if args.is_object() {
                "Object"
            } else if args.is_array() {
                "Array"
            } else {
                "Other"
            };
            let content =
                serde_json::to_string_pretty(args).unwrap_or_else(|_| "<序列化失败>".to_string());
            println!("  [顶层 arguments] 类型={}", type_str);
            println!(
                "    内容: {}",
                content.chars().take(800).collect::<String>()
            );
        }
        if let Some(procs) = obj.get("processors").and_then(|p| p.as_array()) {
            println!("  processors 共 {} 个:", procs.len());
            for (i, p) in procs.iter().enumerate() {
                let jar = p.get("jar").and_then(|j| j.as_str()).unwrap_or("<无 jar>");
                let sides = p
                    .get("sides")
                    .and_then(|s| s.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let (args_type, args_str) = match p.get("arguments") {
                    Some(a) if a.is_object() => (
                        "Object".to_string(),
                        serde_json::to_string_pretty(a)
                            .unwrap_or_else(|_| "<序列化失败>".to_string()),
                    ),
                    Some(a) if a.is_array() => (
                        "Array".to_string(),
                        serde_json::to_string_pretty(a)
                            .unwrap_or_else(|_| "<序列化失败>".to_string()),
                    ),
                    Some(a) => ("Other".to_string(), a.to_string()),
                    None => ("None".to_string(), "<无 arguments>".to_string()),
                };
                println!(
                    "    [{}/{}] jar={}, sides={:?}, args_type={}",
                    i + 1,
                    procs.len(),
                    jar,
                    sides,
                    args_type
                );
                println!(
                    "      args={}",
                    args_str.chars().take(500).collect::<String>()
                );
            }
        } else {
            println!("  未找到 processors 字段!");
        }
    }
    let mut ip_model_mut = ip_model.clone();
    let is_neoforge = forge_version.to_lowercase().contains("neoforge");
    let (maven_group, maven_artifact) = if is_neoforge {
        ("net.neoforged".to_string(), "neoforge".to_string())
    } else {
        ("net.minecraftforge".to_string(), "forge".to_string())
    };
    println!(
        "Loader 类型: group={}, artifact={}, version={}",
        maven_group, maven_artifact, forge_version
    );
    fn extract_entry_to_file(
        archive: &mut ZipArchive<fs::File>,
        idx: usize,
        target_path: &std::path::Path,
    ) -> std::io::Result<String> {
        let mut entry = archive.by_index(idx)?;
        let entry_name = entry.name().to_string();
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut out = fs::File::create(target_path)?;
        std::io::copy(&mut entry, &mut out)?;
        Ok(entry_name)
    }
    println!("解析 Lzma");
    let server_lzma_idx = find_entry_idx(&mut archive, "data/server.lzma");
    if let Some(sidx) = server_lzma_idx {
        let server_maven = format!(
            "{}:{}:{}:serverdata@lzma",
            maven_group, maven_artifact, forge_version
        );
        let target_path = maven_to_full_path(root, &server_maven)?;
        extract_entry_to_file(&mut archive, sidx, &target_path)
            .with_context(|| format!("提取 server.lzma 失败"))?;
        println!("[Lzma] server → {}", target_path.display());
        if let Some(data) = ip_model_mut.get_mut("data") {
            if let Some(bp) = data.get_mut("BINPATCH") {
                if let Some(obj) = bp.as_object_mut() {
                    let key = obj
                        .keys()
                        .find(|k| k.eq_ignore_ascii_case("Server"))
                        .cloned();
                    let key_str = key.unwrap_or_else(|| "Server".to_string());
                    obj.insert(key_str, Value::String(format!("[{}]", server_maven)));
                }
            }
        }
    } else {
        println!("[Lzma] 未找到 data/server.lzma");
    }
    let client_lzma_idx = find_entry_idx(&mut archive, "data/client.lzma");
    if let Some(cidx) = client_lzma_idx {
        let client_maven = format!(
            "{}:{}:{}:clientdata@lzma",
            maven_group, maven_artifact, forge_version
        );
        let target_path = maven_to_full_path(root, &client_maven)?;
        extract_entry_to_file(&mut archive, cidx, &target_path)
            .with_context(|| format!("提取 client.lzma 失败"))?;
        println!("[Lzma] client → {}", target_path.display());
        if let Some(data) = ip_model_mut.get_mut("data") {
            if let Some(bp) = data.get_mut("BINPATCH") {
                if let Some(obj) = bp.as_object_mut() {
                    let key = obj
                        .keys()
                        .find(|k| k.eq_ignore_ascii_case("Client"))
                        .cloned();
                    let key_str = key.unwrap_or_else(|| "Client".to_string());
                    obj.insert(key_str, Value::String(format!("[{}]", client_maven)));
                }
            }
        }
    } else {
        println!("[Lzma] 未找到 data/client.lzma");
    }
    println!("解压 Loader JAR");
    let group_path = maven_group.replace('.', "/");
    let forge_jar_expected = format!(
        "maven/{}/{}/{}/{}-{}.jar",
        group_path, maven_artifact, forge_version, maven_artifact, forge_version
    );
    let universal_jar_expected = format!(
        "maven/{}/{}/{}/{}-{}-universal.jar",
        group_path, maven_artifact, forge_version, maven_artifact, forge_version
    );
    let forge_jar_idx = find_entry_idx(&mut archive, &forge_jar_expected);
    let universal_jar_idx = find_entry_idx(&mut archive, &universal_jar_expected);
    let mut forge_jar_idx = forge_jar_idx;
    let mut universal_jar_idx = universal_jar_idx;
    {
        let mut names: Vec<(usize, String)> = Vec::new();
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                names.push((i, entry.name().to_string()));
            }
        }
        // 1. 先尝试精准查找：maven/.../{artifact}/{version}/<artifact>-<version>.jar
        if forge_jar_idx.is_none() {
            for (i, name) in &names {
                if name.starts_with("maven/")
                    && name.contains(&maven_artifact)
                    && name.ends_with(&format!("{}-{}.jar", maven_artifact, forge_version))
                    && !name.ends_with("-universal.jar")
                {
                    forge_jar_idx = Some(*i);
                    println!("  [宽松匹配] forge jar: {}", name);
                    break;
                }
            }
        }
        // 2. 再尝试查找 universal 变体
        if universal_jar_idx.is_none() {
            for (i, name) in &names {
                if name.starts_with("maven/")
                    && name.ends_with(&format!(
                        "{}-{}-universal.jar",
                        maven_artifact, forge_version
                    ))
                {
                    universal_jar_idx = Some(*i);
                    println!("  [宽松匹配] universal jar: {}", name);
                    break;
                }
            }
        }
        // 3. 更宽松：任何 maven/.../neoforge-<version>* 模式（兼容 NeoForge 所有版本命名）
        if forge_jar_idx.is_none() {
            for (i, name) in &names {
                if name.starts_with("maven/")
                    && name.contains(&format!("/{}/", forge_version))
                    && name.ends_with(".jar")
                    && (name.contains(&format!("{}-{}", maven_artifact, forge_version))
                        || name.contains(&maven_artifact))
                {
                    forge_jar_idx = Some(*i);
                    println!("  [超宽松匹配] forge jar: {}", name);
                    break;
                }
            }
        }

        // 4. 提取 main/universal 到 libraries/ 目录
        //    同时将文件复制一份到另一种命名（xxx.jar ↔ xxx-universal.jar），
        //    确保后续 processor/collect_libs 无论用哪种路径都能找到。
        fn copy_jar_alias(lib_path: &std::path::Path) {
            let file_name = lib_path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            let alias_name = if file_name.ends_with("-universal.jar") {
                file_name.replace("-universal.jar", ".jar")
            } else if file_name.ends_with(".jar") {
                file_name.replace(".jar", "-universal.jar")
            } else {
                String::new()
            };
            if !alias_name.is_empty() {
                if let Some(parent) = lib_path.parent() {
                    let alias_path = parent.join(&alias_name);
                    if !alias_path.exists() {
                        if let Ok(_) = fs::copy(lib_path, &alias_path) {
                            println!("[Jar]  别名 → {}", alias_path.display());
                        }
                    }
                }
            }
        }
        if let Some(u_idx) = universal_jar_idx {
            let entry_name = &names[u_idx].1;
            let sub_path = match entry_name.find('/') {
                Some(idx) => &entry_name[idx + 1..],
                None => entry_name.as_str(),
            };
            let lib_path = root.join("libraries").join(sub_path);
            extract_entry_to_file(&mut archive, u_idx, &lib_path)
                .with_context(|| format!("提取 universal JAR 失败"))?;
            println!("[Jar] universal → {}", lib_path.display());
            copy_jar_alias(&lib_path);
        }
        if let Some(f_idx) = forge_jar_idx {
            let entry_name = &names[f_idx].1;
            let sub_path = match entry_name.find('/') {
                Some(idx) => &entry_name[idx + 1..],
                None => entry_name.as_str(),
            };
            let lib_path = root.join("libraries").join(sub_path);
            extract_entry_to_file(&mut archive, f_idx, &lib_path)
                .with_context(|| format!("提取 main JAR 失败"))?;
            println!("[Jar] main → {}", lib_path.display());
            copy_jar_alias(&lib_path);
        } else {
            println!("[Jar] WARN: 未找到 main JAR（将尝试通过 maven/ 目录的全部条目兜底）");
        }

        // 5. 兜底：把 installer 内 maven/ 目录下的所有 JAR 都提取到 libraries/
        //    这能保证无论 NeoForge/Forge 用什么命名方式，所有依赖和主 JAR 都能到位
        let mut extracted_any = false;
        for (i, name) in &names {
            if !name.starts_with("maven/") || !name.ends_with(".jar") {
                continue;
            }
            // 跳过前面已经提取过的（避免重复 IO）
            let already = match (forge_jar_idx, universal_jar_idx) {
                (Some(fi), Some(ui)) if *i == fi || *i == ui => true,
                (Some(fi), None) if *i == fi => true,
                (None, Some(ui)) if *i == ui => true,
                _ => false,
            };
            if already {
                continue;
            }
            let sub_path = match name.find('/') {
                Some(idx) => &name[idx + 1..],
                None => name.as_str(),
            };
            let lib_path = root.join("libraries").join(sub_path);
            if lib_path.exists() {
                continue;
            }
            if let Err(e) = extract_entry_to_file(&mut archive, *i, &lib_path) {
                eprintln!("[Jar] 提取失败（忽略）: {} → {}", name, e);
                continue;
            }
            extracted_any = true;
            println!("[Jar] 补充提取: {}", lib_path.display());

            // 对 loader 自身的 JAR（neoforge/forge）同时生成别名
            let file_name = lib_path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            let is_loader_file = file_name.contains("neoforge") || file_name.contains("forge");
            if is_loader_file {
                copy_jar_alias(&lib_path);
            }
        }
        if extracted_any {
            println!("[Jar] maven/ 目录下的其他 JAR 已补充提取完毕");
        }
    }
    drop(archive);
    let mut is_mojmap_downloaded = false;
    if let Some(data) = ip_model_mut.get("data") {
        if let Some(data_obj) = data.as_object() {
            if let Some(mojmaps) = data_obj.get("MOJMAPS").or_else(|| {
                data_obj
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("MOJMAPS"))
                    .map(|(_, v)| v)
            }) {
                let client_val = mojmaps
                    .get("Client")
                    .or_else(|| mojmaps.get("client"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !client_val.is_empty() {
                    let inner = client_val.trim_start_matches('[').trim_end_matches(']');
                    if let Ok(target_path) = maven_to_full_path(root, inner) {
                        let target_dir = target_path
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("."));
                        fs::create_dir_all(&target_dir).ok();
                        let file_name = target_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if target_path.exists() {
                            println!("[MOJMAPS] 文件已存在，跳过: {}", target_path.display());
                            is_mojmap_downloaded = true;
                        } else {
                            let mut urls = Vec::new();
                            if let Ok(rel) = maven_to_relative_path(inner) {
                                for mirror in &cfg.library_mirrors {
                                    urls.push(format!("{}/{}", mirror.trim_end_matches('/'), rel));
                                }
                            }
                            if !urls.is_empty() {
                                println!("[MOJMAPS] 下载 {}, URL: {}", file_name, urls[0]);
                                let task = DownloadTask {
                                    file_name,
                                    target_dir: target_dir.clone(),
                                    urls,
                                    sha1: None,
                                };
                                match concurrent_download::download_one(task).await {
                                    Ok(_) => {
                                        is_mojmap_downloaded = true;
                                        println!("[MOJMAPS] 下载成功");
                                    }
                                    Err(e) => {
                                        println!("[MOJMAPS] 下载失败（忽略）: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("解析 Processor");
    let mut variables: HashMap<String, String> = HashMap::new();
    let mc_jar_path = root
        .join("versions")
        .join(&cfg.mc_version_id)
        .join(format!("{}.jar", cfg.mc_version_id));
    variables.insert(
        "MINECRAFT_JAR".to_string(),
        mc_jar_path.to_string_lossy().to_string(),
    );
    let library_dir = root.join("libraries");
    variables.insert("SIDE".to_string(), "client".to_string());
    variables.insert("ROOT".to_string(), root.to_string_lossy().to_string());
    variables.insert("MINECRAFT_VERSION".to_string(), cfg.mc_version.clone());
    variables.insert(
        "INSTALLER".to_string(),
        installer_jar_full.to_string_lossy().to_string(),
    );
    variables.insert(
        "LIBRARY_DIR".to_string(),
        library_dir.to_string_lossy().to_string(),
    );
    if let Some(data) = ip_model_mut.get("data") {
        if let Some(obj) = data.as_object() {
            for (k, v) in obj {
                let client_str = v
                    .get("Client")
                    .or_else(|| v.get("client"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let resolved_client = resolve_bracket_value(&client_str, root);
                if resolved_client.trim().is_empty() {
                    continue;
                }
                variables.insert(k.clone(), resolved_client);
            }
        }
    }
    let library_dir = root.join("libraries");
    let args_replace: HashMap<String, String> = vec![
        ("{SIDE}".to_string(), "client".to_string()),
        (
            "{MINECRAFT_JAR}".to_string(),
            mc_jar_path.to_string_lossy().to_string(),
        ),
        ("{MINECRAFT_VERSION}".to_string(), cfg.mc_version.clone()),
        ("{ROOT}".to_string(), root.to_string_lossy().to_string()),
        (
            "{INSTALLER}".to_string(),
            installer_jar_full.to_string_lossy().to_string(),
        ),
        (
            "{LIBRARY_DIR}".to_string(),
            library_dir.to_string_lossy().to_string(),
        ),
    ]
    .into_iter()
    .collect();
    let raw_processors = parse_processors(&ip_model_mut);
    let mut proc_list: Vec<(String, Vec<String>, Vec<String>)> = Vec::new(); 
    for (p_idx, proc) in raw_processors.iter().enumerate() {
        println!(
            "  [Raw {}/{}] jar={}, 原始 args={:?}",
            p_idx + 1,
            raw_processors.len(),
            proc.jar,
            proc.arguments
        );
    }
    for proc in raw_processors {
        if !proc.sides.is_empty() {
            let has_client = proc.sides.iter().any(|s| s.eq_ignore_ascii_case("client"));
            if !has_client {
                continue;
            }
        }
        if is_mojmap_downloaded
            && proc
                .arguments
                .iter()
                .any(|a| a.contains("DOWNLOAD_MOJMAPS"))
        {
            continue;
        }
        let resolved_args: Vec<String> = proc
            .arguments
            .iter()
            .map(|arg| {
                let mut s = arg.clone();
                for (k, v) in &args_replace {
                    s = s.replace(k, v);
                }
                {
                    let trimmed = s.trim().to_string();
                    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
                        let inner = &trimmed[1..trimmed.len() - 1];
                        if let Ok(path) = maven_to_full_path(root, inner) {
                            s = path.to_string_lossy().to_string();
                        }
                    }
                }
                {
                    let trimmed = s.trim().to_string();
                    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
                        let key = &trimmed[1..trimmed.len() - 1];
                        if let Some(val) = variables.get(key) {
                            s = val.clone();
                        } else {
                            let matched_key = variables
                                .keys()
                                .find(|k| k.eq_ignore_ascii_case(key))
                                .cloned();
                            if let Some(real_key) = matched_key {
                                s = variables[&real_key].clone();
                            }
                        }
                    }
                }
                fix_path_argument(&s)
            })
            .collect();
        println!(
            "  [Resolved {}] 解析后 args={:?}",
            proc_list.len() + 1,
            resolved_args
        );
        proc_list.push((proc.jar, proc.classpath, resolved_args));
    }
    println!("共解析到 {} 个 Processor", proc_list.len());
    println!("下载 Libraries");
    let mut lib_tasks: Vec<DownloadTask> = Vec::new();
    fn collect_libs(
        json_val: &Value,
        field: &str,
        root: &Path,
        _loader_artifact: &str,
        mirrors: &[String],
        tasks: &mut Vec<DownloadTask>,
    ) {
        let Some(arr) = json_val.get(field).and_then(|a| a.as_array()) else {
            return;
        };
        for lib_val in arr {
            let lib = parse_library(lib_val);
            let name = lib.name.clone().unwrap_or_default();
            let url = lib.url.clone().unwrap_or_default();
            let url_empty = url.trim().is_empty();
            let Some(path) = lib.path.clone() else {
                continue;
            };
            if path.trim().is_empty() {
                continue;
            }
            let mut path = path;
            let (dir_part, file_name) = match path.rfind('/') {
                Some(i) => (path[..i].to_string(), path[i + 1..].to_string()),
                None => (String::new(), path.clone()),
            };
            let target_dir = root.join("libraries").join(&dir_part);
            let _full_file = target_dir.join(&file_name);
            // NeoForge 自身的 JAR 需要特殊处理：
            // maven.neoforged.net 上发布的是 neoforge-{version}-universal.jar
            // 而不是标准的 neoforge-{version}.jar。如果当前条目是
            // net.neoforged:neoforge:VERSION 且文件名不含 -universal，
            // 就把 path 改为带 -universal.jar 后缀。
            let is_neoforge_self = name.starts_with("net.neoforged:neoforge:");
            if is_neoforge_self {
                let neoforge_parts: Vec<&str> = name.split(':').collect();
                if neoforge_parts.len() >= 3 {
                    let nf_version = neoforge_parts[2];
                    let new_file_name = format!("neoforge-{}-universal.jar", nf_version);
                    let new_path = if dir_part.is_empty() {
                        new_file_name.clone()
                    } else {
                        format!("{}/{}", dir_part, new_file_name)
                    };
                    path = new_path;
                }
            }
            // 重新解析（因为 path 可能被修改了）
            let (dir_part, file_name) = match path.rfind('/') {
                Some(i) => (path[..i].to_string(), path[i + 1..].to_string()),
                None => (String::new(), path.clone()),
            };
            let target_dir = root.join("libraries").join(&dir_part);
            let full_file = target_dir.join(&file_name);
            // client/server 分类器是 Forge 安装处理器生成的产物，并不发布在 Maven，
            // 因此不能把它们当作普通依赖下载。NeoForge/Forge 的 universal JAR
            // 则可能真实发布，仍保留下载。
            let is_loader_self = name.starts_with("net.minecraftforge:forge:")
                || name.starts_with("net.neoforged:neoforge:");
            if file_name.ends_with("-client.jar")
                || file_name.ends_with("-server.jar")
                || (!is_loader_self && file_name.ends_with("-universal.jar"))
            {
                continue;
            }
            if full_file.exists() {
                if let Some(sha) = &lib.sha1 {
                    if let Ok(calc) = sha1_of_file(&full_file) {
                        if calc.eq_ignore_ascii_case(sha) {
                            continue;
                        }
                    }
                } else {
                    continue;
                }
            }
            if let Some(parent) = full_file.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut urls: Vec<String> = Vec::new();
            if !url_empty {
                urls.push(url.clone());
            }
            for mirror in mirrors {
                urls.push(format!("{}/{}", mirror.trim_end_matches('/'), path));
            }
            if !name.is_empty() {
                if let Ok(rel) = maven_to_relative_path(&name) {
                    if name.contains("minecraftforge") {
                        urls.push(format!("https://files.minecraftforge.net/maven/{}", rel));
                        urls.push(format!("https://bmclapi2.bangbang93.com/maven/{}", rel));
                    } else if name.contains("neoforged") {
                        // NeoForge 的主 JAR 带 -universal 分类器发布
                        // rel = "net/neoforged/neoforge/VERSION/neoforge-VERSION.jar"
                        // 需要改成 ".../neoforge-VERSION-universal.jar"
                        let neoforge_rel = if name.starts_with("net.neoforged:neoforge:")
                            && !rel.contains("-universal.jar")
                        {
                            rel.replace(".jar", "-universal.jar")
                        } else {
                            rel
                        };
                        urls.push(format!(
                            "https://maven.neoforged.net/releases/{}",
                            neoforge_rel
                        ));
                        urls.push(format!(
                            "https://bmclapi2.bangbang93.com/maven/{}",
                            neoforge_rel
                        ));
                    }
                }
            }
            if urls.is_empty() {
                continue;
            }
            tasks.push(DownloadTask {
                file_name,
                target_dir,
                urls,
                sha1: lib.sha1.clone(),
            });
        }
    }
    collect_libs(
        &ip_model_mut,
        "libraries",
        root,
        &maven_artifact,
        &cfg.library_mirrors,
        &mut lib_tasks,
    );
    collect_libs(
        &version_json,
        "libraries",
        root,
        &maven_artifact,
        &cfg.library_mirrors,
        &mut lib_tasks,
    );
    println!("需要下载 {} 个库文件", lib_tasks.len());
    let result = concurrent_download::download_all(lib_tasks, progress_tx.clone()).await;
    println!(
        "下载完成: 成功 {} / 失败 {}",
        result.success_count,
        result.failures.len()
    );
    if !result.failures.is_empty() {
        for f in &result.failures {
            eprintln!("  失败: {} - {}", f.file_name, f.error);
        }
        bail!("未能下载全部依赖库，{} 个文件失败", result.failures.len());
    }
    if let Some(ready) = wait_for_original {
        while !ready.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
    println!("运行 Processors (共 {} 个)", proc_list.len());
    for (idx, (jar, cp, args)) in proc_list.iter().enumerate() {
        let jar_path = maven_to_full_path(root, jar)
            .with_context(|| format!("解析 processor JAR 路径失败: {}", jar))?;
        println!(
            "[Processor {}/{}] jar={}",
            idx + 1,
            proc_list.len(),
            jar_path.display()
        );
        let main_class = read_main_class_from_jar(&jar_path)?;
        println!("  Main-Class: {}", main_class);
        let mut cp_entries: Vec<String> = Vec::new();
        for c in cp {
            if let Ok(p) = maven_to_full_path(root, c) {
                cp_entries.push(p.to_string_lossy().to_string());
            }
        }
        cp_entries.push(jar_path.to_string_lossy().to_string());
        #[cfg(target_os = "windows")]
        let cp_sep = ";";
        #[cfg(not(target_os = "windows"))]
        let cp_sep = ":";
        let cp_str = cp_entries.join(cp_sep);
        let mut cmd = Command::new(&cfg.java_executable_path);
        cmd.arg("-cp");
        cmd.arg(&cp_str);
        cmd.arg(&main_class);
        for a in args {
            cmd.arg(a);
        }
        cmd.current_dir(root);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        println!(
            "  Java: {} -cp \"...\" {} <args...>",
            cfg.java_executable_path.display(),
            main_class
        );
        for (i, a) in args.iter().enumerate() {
            println!("    Arg[{}]: {}", i, a);
        }
        let output = cmd
            .output()
            .with_context(|| format!("启动 processor 子进程失败: {}", main_class))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let proc_log_dir = root.join("versions").join(&id);
        fs::create_dir_all(&proc_log_dir).ok();
        if !stdout.trim().is_empty() {
            let log_path = proc_log_dir.join(format!("PROCESSOR_{}_Logs.log", idx + 1));
            fs::write(&log_path, stdout.as_bytes()).ok();
            println!(
                "  stdout (last): {}",
                stdout
                    .lines()
                    .last()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect::<String>()
            );
        }
        if !stderr.trim().is_empty() {
            let err_path = proc_log_dir.join(format!("PROCESSOR_{}_Errors.log", idx + 1));
            fs::write(&err_path, stderr.as_bytes()).ok();
            eprintln!(
                "  stderr (first): {}",
                stderr
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect::<String>()
            );
        }
        if !output.status.success() {
            bail!(
                "Processor {}/{} 执行失败 (退出码 {:?}, main_class={}):\n--- STDOUT ---\n{}\n--- STDERR ---\n{}",
                idx + 1,
                proc_list.len(),
                output.status.code(),
                main_class,
                stdout,
                stderr
            );
        }
        println!("[Processor {}/{}] ✓ 成功", idx + 1, proc_list.len());
    }
    // 确保 options.txt 存在并设置语言为中文
    let versions_dir = root.join("versions").join(&id);
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
    
    println!("\n✓ 安装完成: {}", id);
    Ok(id)
}
fn resolve_bracket_value(val: &str, root: &Path) -> String {
    let trimmed = val.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        if let Ok(path) = maven_to_full_path(root, inner) {
            return path.to_string_lossy().to_string();
        }
    }
    val.to_string()
}
fn fix_path_argument(val: &str) -> String {
    let mut s = val.trim().to_string();
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    while s.contains("\\\\") {
        s = s.replace("\\\\", "\\");
    }
    s
}
