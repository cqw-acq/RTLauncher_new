use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── 数据结构 ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
    pub major_version: i32,
    pub vendor: String,
    pub architecture: String,
    /// "JDK" 或 "JRE"
    pub java_type: String,
}

/// 内部使用：验证结果附带 java.home 用于去重
struct ValidatedJava {
    installation: JavaInstallation,
    /// JVM 报告的 java.home（用于去重，同一 JDK 不同入口路径会返回相同 java.home）
    java_home: String,
}

// ─── 版本识别 ───────────────────────────────────────────────────────────────────

/// 使用 -XshowSettings:properties -version 获取 Java 详细信息。
/// 如果该方式失败，回退到 java -version 解析。
fn get_java_version(java_path: &str) -> Option<JavaInstallation> {
    get_java_version_full(java_path).map(|v| v.installation)
}

/// 内部版本：返回 ValidatedJava（含 java.home 用于去重）
fn get_java_version_full(java_path: &str) -> Option<ValidatedJava> {
    if !Path::new(java_path).exists() {
        return None;
    }

    // Windows: 优先用 javaw 避免弹出控制台窗口
    let detect_exe = pick_detect_exe(java_path);

    // 先尝试 -XshowSettings:properties -version（信息最完整）
    if let Some(v) = try_show_settings(&detect_exe, java_path) {
        return Some(v);
    }

    // 回退: java -version（stderr 输出）
    try_version_flag(&detect_exe, java_path)
}

/// Windows 上用 javaw 代替 java 执行检测，避免弹出黑窗口
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

/// 通过 -XshowSettings:properties -version 获取完整属性
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

    // java.home 是 JVM 报告的真实安装目录，用于去重
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

/// 回退方案: 解析 java -version 的 stderr 输出
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

    // 第一行匹配: (java|openjdk) version "..."
    let version_re = regex::Regex::new(r#"version "([^"]+)""#).ok()?;
    let version = version_re
        .captures(&text)?
        .get(1)?
        .as_str()
        .to_string();
    let major_version = parse_major_version(&version)?;

    // 供应商推断
    let vendor = infer_vendor_from_version_output(&text);

    // 架构推断
    let architecture = if text.contains("64-Bit") {
        "x64".to_string()
    } else {
        "Unknown".to_string()
    };

    let java_type = detect_java_type(java_path);

    // 回退方式没有 java.home，用 canonical path 代替
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

/// 从 java -version 输出推断供应商
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

/// 判断 JDK 还是 JRE：检查同级是否有 javac 可执行文件
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

// ─── 工具函数 ───────────────────────────────────────────────────────────────────

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
        // "1.8.0_xxx" → 8
        version
            .split('.')
            .nth(1)
            .and_then(|s| s.split('_').next())
            .and_then(|s| s.parse().ok())
    } else {
        // "17.0.x", "21-ea" → 17, 21
        version
            .split('.')
            .next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse().ok())
    }
}

// ─── 路径查找 ───────────────────────────────────────────────────────────────────

/// 在目录中查找 java 可执行文件，兼容标准布局和 macOS bundle 布局
fn find_java_exe(dir: &Path) -> Option<PathBuf> {
    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };

    // 标准布局: dir/bin/java
    let standard = dir.join("bin").join(bin_name);
    if standard.exists() {
        return Some(standard);
    }

    // macOS JDK bundle: dir/Contents/Home/bin/java
    #[cfg(target_os = "macos")]
    {
        let macos = dir
            .join("Contents")
            .join("Home")
            .join("bin")
            .join(bin_name);
        if macos.exists() {
            return Some(macos);
        }

        // Mojang 下载的 Java: dir/jre.bundle/Contents/Home/bin/java
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

/// 将路径规范化为 canonical 形式（解析符号链接），用于去重
fn canonical_path(path: &str) -> String {
    fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

// ─── 候选路径收集 ───────────────────────────────────────────────────────────────

/// 收集所有候选 java 可执行文件路径
fn collect_candidates() -> Vec<String> {
    let mut candidates = Vec::new();

    // 1. 平台已知目录扫描
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

    // 2. Windows 注册表扫描
    #[cfg(target_os = "windows")]
    {
        candidates.extend(scan_windows_registry());
    }

    // 3. JAVA_HOME 环境变量
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let home = PathBuf::from(&java_home);
        if let Some(exe) = find_java_exe(&home) {
            if let Some(s) = exe.to_str() {
                candidates.push(s.to_string());
            }
        }
    }

    // 4. PATH 中的 java（通过 which / where）
    candidates.extend(find_java_in_path());

    // 5. Unix: 解析符号链接获取真实路径
    #[cfg(unix)]
    {
        candidates.extend(resolve_symlinks_to_homes(&candidates));
    }

    // 6. 去重（基于 canonical path）
    dedup_candidates(candidates)
}

/// 通过 which / where 查找 PATH 中的 java
fn find_java_in_path() -> Vec<String> {
    let mut results = Vec::new();
    let which_cmd = if cfg!(windows) { "where" } else { "which" };

    // which -a / where 可以返回多个结果
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

/// Unix: 解析符号链接，找到真实 Java home 并加入候选
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

/// 基于 canonical path 去重
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

// ─── Windows 注册表扫描 ────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn scan_windows_registry() -> Vec<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let mut results = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 注册表路径列表（含 WOW6432Node 32位兼容层）
    let reg_paths = [
        r"SOFTWARE\JavaSoft\Java Runtime Environment",
        r"SOFTWARE\JavaSoft\Java Development Kit",
        r"SOFTWARE\JavaSoft\JRE",
        r"SOFTWARE\JavaSoft\JDK",
        r"SOFTWARE\WOW6432Node\JavaSoft\Java Runtime Environment",
        r"SOFTWARE\WOW6432Node\JavaSoft\Java Development Kit",
        r"SOFTWARE\WOW6432Node\JavaSoft\JRE",
        r"SOFTWARE\WOW6432Node\JavaSoft\JDK",
        // Eclipse Adoptium / Temurin
        r"SOFTWARE\Eclipse Adoptium\JDK",
        r"SOFTWARE\Eclipse Adoptium\JRE",
        r"SOFTWARE\Eclipse Foundation\JDK",
        // Microsoft
        r"SOFTWARE\Microsoft\JDK",
        // Azul Zulu
        r"SOFTWARE\Azul Systems\Zulu",
        // Amazon Corretto
        r"SOFTWARE\Amazon.com\Corretto",
        // BellSoft Liberica
        r"SOFTWARE\BellSoft\Liberica",
    ];

    for reg_path in &reg_paths {
        if let Ok(key) = hklm.open_subkey_with_flags(reg_path, KEY_READ) {
            // 遍历版本子项
            for subkey_name in key.enum_keys().filter_map(|r| r.ok()) {
                if let Ok(subkey) = key.open_subkey_with_flags(&subkey_name, KEY_READ) {
                    // 尝试 JavaHome 值
                    if let Ok(java_home) = subkey.get_value::<String, _>("JavaHome") {
                        let exe = PathBuf::from(&java_home).join("bin").join("java.exe");
                        if exe.exists() {
                            if let Some(s) = exe.to_str() {
                                results.push(s.to_string());
                            }
                        }
                    }
                    // 某些注册表项用 Path 或 InstallationPath
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
            // 也检查当前项自身是否有 JavaHome（某些安装直接放在父项）
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

// ─── 平台搜索路径 ──────────────────────────────────────────────────────────────

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
    // Program Files 下的常见 Java 供应商目录
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
        // scoop 安装的 Java
        paths.push(PathBuf::from(&localappdata).join("scoop").join("apps").join("java"));
        paths.push(
            PathBuf::from(&localappdata)
                .join("scoop")
                .join("apps")
                .join("openjdk"),
        );
    }
    // RTLauncher 内置 Java 下载目录
    paths.push(PathBuf::from("./RTL/java"));
}

#[allow(unused)]
fn get_macos_search_paths(paths: &mut Vec<PathBuf>) {
    // 系统级安装
    paths.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    paths.push(PathBuf::from("/System/Library/Java/JavaVirtualMachines"));

    // Homebrew (Apple Silicon + Intel)
    for prefix in &["/opt/homebrew/opt", "/usr/local/opt"] {
        let opt_dir = PathBuf::from(prefix);
        if opt_dir.exists() {
            if let Ok(entries) = fs::read_dir(&opt_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    // openjdk, openjdk@17, openjdk@21 等
                    if name_str.starts_with("openjdk") {
                        paths.push(entry.path().join("libexec"));
                    }
                }
            }
        }
    }
    // Homebrew Caskroom (含第三方 Java 如 temurin, zulu 等)
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
                        // Caskroom/<cask>/<version>/<app>.jdk
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
        // SDKMAN
        let sdkman_java = PathBuf::from(&home).join(".sdkman/candidates/java");
        if sdkman_java.exists() {
            paths.push(sdkman_java);
        }
        // RTLauncher 内置 Java 下载目录
        paths.push(
            PathBuf::from(&home).join("Library/Application Support/RTLauncher/java"),
        );
    }
}

#[allow(unused)]
fn get_linux_search_paths(paths: &mut Vec<PathBuf>) {
    // 标准系统目录
    paths.push(PathBuf::from("/usr/lib/jvm"));
    paths.push(PathBuf::from("/usr/java"));
    paths.push(PathBuf::from("/usr/local/java"));
    paths.push(PathBuf::from("/opt/java"));
    paths.push(PathBuf::from("/opt/jdk"));
    paths.push(PathBuf::from("/opt/jre"));

    // 供应商特定目录
    // Amazon Corretto
    paths.push(PathBuf::from("/usr/lib/jvm"));
    // Snap 安装
    paths.push(PathBuf::from("/snap"));

    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(&home).join(".jdks"));
        // SDKMAN
        let sdkman_java = PathBuf::from(&home).join(".sdkman/candidates/java");
        if sdkman_java.exists() {
            paths.push(sdkman_java);
        }
        // Jabba
        let jabba_java = PathBuf::from(&home).join(".jabba/jdk");
        if jabba_java.exists() {
            paths.push(jabba_java);
        }
        // asdf
        let asdf_java = PathBuf::from(&home).join(".asdf/installs/java");
        if asdf_java.exists() {
            paths.push(asdf_java);
        }
    }
    // RTLauncher 内置 Java 下载目录
    paths.push(PathBuf::from("./java"));
}

// ─── Tauri 命令 ─────────────────────────────────────────────────────────────────

/// 搜索系统中的 Java 安装（并行验证）
#[tauri::command]
pub async fn search_java_installations() -> Result<Vec<JavaInstallation>, String> {
    // collect_candidates 内部调用 Command（同步阻塞），放到 blocking 线程池
    let candidates = tokio::task::spawn_blocking(collect_candidates)
        .await
        .map_err(|e| format!("搜索失败: {}", e))?;

    // 并行验证所有候选路径（使用内部版本获取 java.home）
    let handles: Vec<_> = candidates
        .into_iter()
        .map(|path| tokio::task::spawn_blocking(move || get_java_version_full(&path)))
        .collect();

    // 基于 java.home 去重：同一 JDK 的不同入口（如 /usr/bin/java 和真实路径）
    // 返回相同的 java.home，只保留路径更具体（更长）的那个
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
            // 优先保留路径更具体的（更长的路径通常是真实安装位置，而非 /usr/bin/java 之类的 shim）
            if existing.is_none() || validated.installation.path.len() > existing.unwrap().path.len()
            {
                home_map.insert(key, validated.installation);
            }
        }
    }

    let mut installations: Vec<JavaInstallation> = home_map.into_values().collect();
    installations.sort_by(|a, b| b.major_version.cmp(&a.major_version));
    Ok(installations)
}

/// 验证 Java 路径是否有效
#[tauri::command]
pub fn validate_java_path(java_path: String) -> Result<JavaInstallation, String> {
    if java_path.is_empty() {
        return Err("Java路径不能为空".to_string());
    }

    let path = PathBuf::from(&java_path);

    let java_exe = if path.is_dir() {
        find_java_exe(&path)
            .ok_or_else(|| format!("未在目录中找到Java可执行文件: {}", path.display()))?
    } else {
        path
    };

    if !java_exe.exists() {
        return Err(format!("Java可执行文件不存在: {}", java_exe.display()));
    }

    let java_str = java_exe
        .to_str()
        .ok_or_else(|| "无效的路径格式".to_string())?;

    get_java_version(java_str)
        .ok_or_else(|| "无法获取Java版本信息，请确保这是有效的Java安装".to_string())
}
