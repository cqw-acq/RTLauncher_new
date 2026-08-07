use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
    pub major_version: i32,
    pub vendor: String,
    pub architecture: String,
    pub java_type: String,
}
struct ValidatedJava {
    installation: JavaInstallation,
    java_home: String,
}
fn get_java_version(java_path: &str) -> Option<JavaInstallation> {
    get_java_version_full(java_path).map(|v| v.installation)
}
fn get_java_version_full(java_path: &str) -> Option<ValidatedJava> {
    if !Path::new(java_path).exists() {
        return None;
    }
    let detect_exe = pick_detect_exe(java_path);
    let mut result = try_show_settings(&detect_exe, java_path)
        .or_else(|| try_version_flag(&detect_exe, java_path))?;
    if cfg!(windows) {
        result.installation.path = prefer_javaw(&result.installation.path);
    }
    Some(result)
}
fn prefer_javaw(java_path: &str) -> String {
    let javaw_path = java_path
        .replace("java.exe", "javaw.exe")
        .replace("\\java\"", "\\javaw\"");
    if javaw_path != java_path && Path::new(&javaw_path).exists() {
        javaw_path
    } else {
        java_path.to_string()
    }
}
fn pick_detect_exe(java_path: &str) -> String {
    if cfg!(windows) {
        let candidate = java_path
            .replace("java.exe", "javaw.exe")
            .replace("\\java\"", "\\javaw\"");
        if Path::new(&candidate).exists() {
            return candidate;
        }
    }
    java_path.to_string()
}
fn try_show_settings(detect_exe: &str, java_path: &str) -> Option<ValidatedJava> {
    let output = Command::new(detect_exe)
        .args(["-XshowSettings:properties", "-version"])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stderr, stdout);
    let get_prop = |key: &str| -> Option<String> {
        combined
            .lines()
            .find(|line| {
                let t = line.trim();
                t.starts_with(key) && t[key.len()..].trim_start().starts_with('=')
            })
            .and_then(|line| line.splitn(2, '=').nth(1))
            .map(|v| v.trim().to_string())
    };
    let version = get_prop("java.version")?;
    let major_version = parse_major_version(&version)?;
    let vendor = get_prop("java.vendor")
        .map(|v| normalize_vendor(&v))
        .unwrap_or_else(|| "Unknown".to_string());
    let architecture = get_prop("os.arch")
        .map(|a| normalize_arch(&a))
        .unwrap_or_else(|| "Unknown".to_string());
    let java_type = detect_java_type(java_path);
    let java_home = get_prop("java.home").unwrap_or_default();
    Some(ValidatedJava {
        installation: JavaInstallation {
            path: java_path.to_string(),
            version,
            major_version,
            vendor,
            architecture,
            java_type,
        },
        java_home,
    })
}
fn try_version_flag(detect_exe: &str, java_path: &str) -> Option<ValidatedJava> {
    let output = Command::new(detect_exe)
        .arg("-version")
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.contains("version") {
        stderr.to_string()
    } else {
        stdout.to_string()
    };
    let version_re = regex::Regex::new(r#"version "([^"]+)""#).ok()?;
    let version = version_re.captures(&text)?.get(1)?.as_str().to_string();
    let major_version = parse_major_version(&version)?;
    let vendor = infer_vendor_from_version_output(&text);
    let architecture = if text.contains("64-Bit") {
        "x64".to_string()
    } else {
        "Unknown".to_string()
    };
    let java_type = detect_java_type(java_path);
    let java_home = canonical_path(java_path);
    Some(ValidatedJava {
        installation: JavaInstallation {
            path: java_path.to_string(),
            version,
            major_version,
            vendor,
            architecture,
            java_type,
        },
        java_home,
    })
}
fn infer_vendor_from_version_output(text: &str) -> String {
    let t = text.to_lowercase();
    if t.contains("temurin") || t.contains("adoptium") {
        "Eclipse Temurin".to_string()
    } else if t.contains("graalvm") {
        "GraalVM".to_string()
    } else if t.contains("corretto") {
        "Amazon Corretto".to_string()
    } else if t.contains("zulu") {
        "Azul Zulu".to_string()
    } else if t.contains("microsoft") {
        "Microsoft".to_string()
    } else if t.contains("liberica") || t.contains("bellsoft") {
        "BellSoft Liberica".to_string()
    } else if t.contains("semeru") {
        "IBM Semeru".to_string()
    } else if t.contains("java(tm)") || t.contains("java hotspot") {
        "Oracle".to_string()
    } else if t.contains("openjdk") {
        "OpenJDK".to_string()
    } else {
        "Unknown".to_string()
    }
}
fn detect_java_type(java_path: &str) -> String {
    let path = Path::new(java_path);
    if let Some(dir) = path.parent() {
        let javac = if cfg!(windows) {
            dir.join("javac.exe")
        } else {
            dir.join("javac")
        };
        if javac.exists() {
            return "JDK".to_string();
        }
    }
    "JRE".to_string()
}
fn normalize_vendor(vendor: &str) -> String {
    let v = vendor.to_lowercase();
    if v.contains("temurin") || v.contains("adoptium") {
        "Eclipse Temurin".to_string()
    } else if v.contains("graalvm") {
        "GraalVM".to_string()
    } else if v.contains("microsoft") {
        "Microsoft".to_string()
    } else if v.contains("amazon") || v.contains("corretto") {
        "Amazon Corretto".to_string()
    } else if v.contains("azul") || v.contains("zulu") {
        "Azul Zulu".to_string()
    } else if v.contains("bellsoft") || v.contains("liberica") {
        "BellSoft Liberica".to_string()
    } else if v.contains("semeru") || v.contains("ibm") {
        "IBM Semeru".to_string()
    } else if v.contains("oracle") {
        "Oracle".to_string()
    } else if v.contains("openjdk") || v.contains("red hat") {
        "OpenJDK".to_string()
    } else {
        vendor.to_string()
    }
}
fn normalize_arch(arch: &str) -> String {
    match arch.to_lowercase().as_str() {
        "amd64" | "x86_64" => "x64".to_string(),
        "aarch64" | "arm64" => "ARM64".to_string(),
        "x86" | "i386" | "i686" => "x86".to_string(),
        other => other.to_string(),
    }
}
fn parse_major_version(version: &str) -> Option<i32> {
    if version.starts_with("1.") {
        version
            .split('.')
            .nth(1)
            .and_then(|s| s.split('_').next())
            .and_then(|s| s.parse().ok())
    } else {
        version
            .split('.')
            .next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse().ok())
    }
}
fn find_java_exe(dir: &Path) -> Option<PathBuf> {
    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };
    let standard = dir.join("bin").join(bin_name);
    if standard.exists() {
        if cfg!(windows) {
            let javaw = dir.join("bin").join("javaw.exe");
            if javaw.exists() {
                return Some(javaw);
            }
        }
        return Some(standard);
    }
    #[cfg(target_os = "macos")]
    {
        let macos = dir.join("Contents").join("Home").join("bin").join(bin_name);
        if macos.exists() {
            return Some(macos);
        }
        let mojang = dir
            .join("jre.bundle")
            .join("Contents")
            .join("Home")
            .join("bin")
            .join(bin_name);
        if mojang.exists() {
            return Some(mojang);
        }
    }
    None
}
fn canonical_path(path: &str) -> String {
    fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}
fn collect_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    for search_path in get_search_paths() {
        if !search_path.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&search_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(exe) = find_java_exe(&path) {
                        if let Some(s) = exe.to_str() {
                            candidates.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        candidates.extend(scan_windows_registry());
    }
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let home = PathBuf::from(&java_home);
        if let Some(exe) = find_java_exe(&home) {
            if let Some(s) = exe.to_str() {
                candidates.push(s.to_string());
            }
        }
    }
    candidates.extend(find_java_in_path());
    #[cfg(unix)]
    {
        candidates.extend(resolve_symlinks_to_homes(&candidates));
    }
    dedup_candidates(candidates)
}
fn find_java_in_path() -> Vec<String> {
    let mut results = Vec::new();
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let args = if cfg!(windows) {
        vec!["java"]
    } else {
        vec!["-a", "java"]
    };
    if let Ok(output) = Command::new(which_cmd)
        .args(&args)
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = line.trim();
                if !path.is_empty() && Path::new(path).exists() {
                    results.push(path.to_string());
                }
            }
        }
    }
    results
}
#[cfg(unix)]
fn resolve_symlinks_to_homes(existing: &[String]) -> Vec<String> {
    let mut extra = Vec::new();
    for path in existing {
        if let Ok(resolved) = fs::canonicalize(path) {
            let resolved_str = resolved.to_string_lossy().to_string();
            if resolved_str != *path {
                extra.push(resolved_str);
            }
        }
    }
    extra
}
fn dedup_candidates(candidates: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for c in candidates {
        let key = canonical_path(&c);
        if seen.insert(key) {
            result.push(c);
        }
    }
    result
}
#[cfg(target_os = "windows")]
fn scan_windows_registry() -> Vec<String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let mut results = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let reg_paths = [
        r"SOFTWARE\JavaSoft\Java Runtime Environment",
        r"SOFTWARE\JavaSoft\Java Development Kit",
        r"SOFTWARE\JavaSoft\JRE",
        r"SOFTWARE\JavaSoft\JDK",
        r"SOFTWARE\WOW6432Node\JavaSoft\Java Runtime Environment",
        r"SOFTWARE\WOW6432Node\JavaSoft\Java Development Kit",
        r"SOFTWARE\WOW6432Node\JavaSoft\JRE",
        r"SOFTWARE\WOW6432Node\JavaSoft\JDK",
        r"SOFTWARE\Eclipse Adoptium\JDK",
        r"SOFTWARE\Eclipse Adoptium\JRE",
        r"SOFTWARE\Eclipse Foundation\JDK",
        r"SOFTWARE\Microsoft\JDK",
        r"SOFTWARE\Azul Systems\Zulu",
        r"SOFTWARE\Amazon.com\Corretto",
        r"SOFTWARE\BellSoft\Liberica",
    ];
    for reg_path in &reg_paths {
        if let Ok(key) = hklm.open_subkey_with_flags(reg_path, KEY_READ) {
            for subkey_name in key.enum_keys().filter_map(|r| r.ok()) {
                if let Ok(subkey) = key.open_subkey_with_flags(&subkey_name, KEY_READ) {
                    if let Ok(java_home) = subkey.get_value::<String, _>("JavaHome") {
                        let exe = PathBuf::from(&java_home).join("bin").join("java.exe");
                        if exe.exists() {
                            if let Some(s) = exe.to_str() {
                                results.push(s.to_string());
                            }
                        }
                    }
                    for val_name in &["Path", "InstallationPath"] {
                        if let Ok(install_path) = subkey.get_value::<String, _>(val_name) {
                            let exe = PathBuf::from(&install_path).join("bin").join("java.exe");
                            if exe.exists() {
                                if let Some(s) = exe.to_str() {
                                    results.push(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
            if let Ok(java_home) = key.get_value::<String, _>("JavaHome") {
                let exe = PathBuf::from(&java_home).join("bin").join("java.exe");
                if exe.exists() {
                    if let Some(s) = exe.to_str() {
                        results.push(s.to_string());
                    }
                }
            }
        }
    }
    results
}
fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(windows) {
        get_windows_search_paths(&mut paths);
    } else if cfg!(target_os = "macos") {
        get_macos_search_paths(&mut paths);
    } else {
        get_linux_search_paths(&mut paths);
    }
    paths
}
#[allow(unused)]
fn get_windows_search_paths(paths: &mut Vec<PathBuf>) {
    let vendor_dirs = [
        "Java",
        "Eclipse Adoptium",
        "Eclipse Foundation",
        "Zulu",
        "Amazon Corretto",
        "BellSoft",
        "Microsoft",
        "Semeru",
        "GraalVM",
    ];
    if let Ok(pf) = std::env::var("ProgramFiles") {
        for sub in &vendor_dirs {
            paths.push(PathBuf::from(&pf).join(sub));
        }
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        for sub in &["Java", "Eclipse Adoptium", "Zulu"] {
            paths.push(PathBuf::from(&pf86).join(sub));
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        paths.push(PathBuf::from(&profile).join(".jdks"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(&localappdata)
                .join("scoop")
                .join("apps")
                .join("java"),
        );
        paths.push(
            PathBuf::from(&localappdata)
                .join("scoop")
                .join("apps")
                .join("openjdk"),
        );
    }
    paths.push(PathBuf::from("./RTL/java"));
}
#[allow(unused)]
fn get_macos_search_paths(paths: &mut Vec<PathBuf>) {
    paths.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    paths.push(PathBuf::from("/System/Library/Java/JavaVirtualMachines"));
    for prefix in &["/opt/homebrew/opt", "/usr/local/opt"] {
        let opt_dir = PathBuf::from(prefix);
        if opt_dir.exists() {
            if let Ok(entries) = fs::read_dir(&opt_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("openjdk") {
                        paths.push(entry.path().join("libexec"));
                    }
                }
            }
        }
    }
    for prefix in &["/opt/homebrew/Caskroom", "/usr/local/Caskroom"] {
        let caskroom = PathBuf::from(prefix);
        if caskroom.exists() {
            if let Ok(entries) = fs::read_dir(&caskroom) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.contains("temurin")
                        || name_str.contains("zulu")
                        || name_str.contains("corretto")
                        || name_str.contains("graalvm")
                        || name_str.contains("liberica")
                        || name_str.contains("semeru")
                        || name_str.starts_with("java")
                    {
                        if let Ok(versions) = fs::read_dir(entry.path()) {
                            for ver_entry in versions.flatten() {
                                if let Ok(jdks) = fs::read_dir(ver_entry.path()) {
                                    for jdk_entry in jdks.flatten() {
                                        let jdk_path = jdk_entry.path();
                                        if jdk_path.is_dir() {
                                            paths.push(jdk_path);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(&home).join("Library/Java/JavaVirtualMachines"));
        paths.push(PathBuf::from(&home).join(".jdks"));
        let sdkman_java = PathBuf::from(&home).join(".sdkman/candidates/java");
        if sdkman_java.exists() {
            paths.push(sdkman_java);
        }
        paths.push(PathBuf::from(&home).join("Library/Application Support/RTLauncher/java"));
    }
}
#[allow(unused)]
fn get_linux_search_paths(paths: &mut Vec<PathBuf>) {
    paths.push(PathBuf::from("/usr/lib/jvm"));
    paths.push(PathBuf::from("/usr/java"));
    paths.push(PathBuf::from("/usr/local/java"));
    paths.push(PathBuf::from("/opt/java"));
    paths.push(PathBuf::from("/opt/jdk"));
    paths.push(PathBuf::from("/opt/jre"));
    paths.push(PathBuf::from("/usr/lib/jvm"));
    paths.push(PathBuf::from("/snap"));
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(&home).join(".jdks"));
        let sdkman_java = PathBuf::from(&home).join(".sdkman/candidates/java");
        if sdkman_java.exists() {
            paths.push(sdkman_java);
        }
        let jabba_java = PathBuf::from(&home).join(".jabba/jdk");
        if jabba_java.exists() {
            paths.push(jabba_java);
        }
        let asdf_java = PathBuf::from(&home).join(".asdf/installs/java");
        if asdf_java.exists() {
            paths.push(asdf_java);
        }
    }
    paths.push(PathBuf::from("./java"));
}
#[tauri::command]
pub async fn search_java_installations() -> Result<Vec<JavaInstallation>, String> {
    let candidates = tokio::task::spawn_blocking(collect_candidates)
        .await
        .map_err(|e| format!("搜索失败: {}", e))?;
    let handles: Vec<_> = candidates
        .into_iter()
        .map(|path| tokio::task::spawn_blocking(move || get_java_version_full(&path)))
        .collect();
    let mut home_map: std::collections::HashMap<String, JavaInstallation> =
        std::collections::HashMap::new();
    for handle in handles {
        if let Ok(Some(validated)) = handle.await {
            let key = if validated.java_home.is_empty() {
                canonical_path(&validated.installation.path)
            } else {
                validated.java_home
            };
            let existing = home_map.get(&key);
            if existing.is_none()
                || validated.installation.path.len() > existing.unwrap().path.len()
            {
                home_map.insert(key, validated.installation);
            }
        }
    }
    let mut installations: Vec<JavaInstallation> = home_map.into_values().collect();
    installations.sort_by(|a, b| b.major_version.cmp(&a.major_version));
    Ok(installations)
}
#[tauri::command]
pub fn validate_java_path(java_path: String) -> Result<JavaInstallation, String> {
    if java_path.is_empty() {
        return Err("Java路径不能为空".to_string());
    }
    let path = PathBuf::from(&java_path);
    let java_exe = if path.is_dir() {
        find_java_exe(&path)
            .ok_or_else(|| format!("未在目录中找到 Java 可执行文件: {}", path.display()))?
    } else {
        path
    };
    if !java_exe.exists() {
        return Err(format!("Java 可执行文件不存在: {}", java_exe.display()));
    }
    let java_str = java_exe
        .to_str()
        .ok_or_else(|| "无效的路径格式".to_string())?;
    get_java_version(java_str)
        .ok_or_else(|| "无法获取 Java 版本信息，请确保这是有效的 Java 安装".to_string())
}

/// 根据 major version 查找匹配的 Java 安装路径
pub fn find_java_by_major_version(target_major: u32) -> Option<JavaInstallation> {
    let candidates = collect_candidates();
    for path in candidates {
        if let Some(validated) = get_java_version_full(&path) {
            if validated.installation.major_version == target_major as i32 {
                return Some(validated.installation);
            }
        }
    }
    None
}