use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
    pub major_version: i32,
    pub vendor: String,
    pub architecture: String,
}

/// 使用单次 -XshowSettings:properties -version 调用获取所有信息
/// Windows 上使用 javaw 避免弹出控制台窗口
fn get_java_version(java_path: &str) -> Option<JavaInstallation> {
    if !Path::new(java_path).exists() {
        return None;
    }

    // Windows: 用 javaw 代替 java 执行检测，避免弹出黑窗口
    let detect_path = if cfg!(windows) {
        java_path
            .replace("java.exe", "javaw.exe")
            .replace("\\java\"", "\\javaw\"")
    } else {
        java_path.to_string()
    };

    let detect_exe = if cfg!(windows) && Path::new(&detect_path).exists() {
        detect_path.as_str()
    } else {
        java_path
    };

    let output = Command::new(detect_exe)
        .args(["-XshowSettings:properties", "-version"])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .ok()?;

    // Java 将 -version 和 properties 都输出到 stderr
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

    Some(JavaInstallation {
        path: java_path.to_string(),
        version,
        major_version,
        vendor,
        architecture,
    })
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
    } else if v.contains("oracle") {
        "Oracle".to_string()
    } else if v.contains("openjdk") {
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
        let macos = dir.join("Contents").join("Home").join("bin").join(bin_name);
        if macos.exists() {
            return Some(macos);
        }

        // Mojang 下载的 Java: dir/jre.bundle/Contents/Home/bin/java
        let mojang = dir.join("jre.bundle").join("Contents").join("Home").join("bin").join(bin_name);
        if mojang.exists() {
            return Some(mojang);
        }
    }

    None
}

/// 收集所有候选 java 可执行文件路径（纯同步，供 spawn_blocking 调用）
fn collect_candidates() -> Vec<String> {
    let mut candidates = Vec::new();

    // 搜索平台已知目录
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

    // JAVA_HOME 环境变量
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let home = PathBuf::from(java_home);
        if let Some(exe) = find_java_exe(&home) {
            if let Some(s) = exe.to_str() {
                candidates.push(s.to_string());
            }
        }
    }

    // PATH 中的 java
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    if let Ok(output) = Command::new(which_cmd)
        .arg("java")
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = line.trim();
                if !path.is_empty() && Path::new(path).exists() {
                    candidates.push(path.to_string());
                }
            }
        }
    }

    candidates
}

/// 搜索系统中的Java安装（并行验证）
#[tauri::command]
pub async fn search_java_installations() -> Result<Vec<JavaInstallation>, String> {
    // collect_candidates 内部调用 Command（同步阻塞），放到 blocking 线程池
    let candidates = tokio::task::spawn_blocking(collect_candidates)
        .await
        .map_err(|e| format!("搜索失败: {}", e))?;

    // 并行验证所有候选路径
    let handles: Vec<_> = candidates
        .into_iter()
        .map(|path| tokio::task::spawn_blocking(move || get_java_version(&path)))
        .collect();

    let mut installations: Vec<JavaInstallation> = Vec::new();
    for handle in handles {
        if let Ok(Some(inst)) = handle.await {
            if !installations.iter().any(|i| i.path == inst.path) {
                installations.push(inst);
            }
        }
    }

    installations.sort_by(|a, b| b.major_version.cmp(&a.major_version));
    Ok(installations)
}

fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if cfg!(windows) {
        if let Ok(pf) = std::env::var("ProgramFiles") {
            for sub in &["Java", "Eclipse Adoptium", "Eclipse Foundation", "Zulu", "Amazon Corretto", "BellSoft", "Microsoft"] {
                paths.push(PathBuf::from(&pf).join(sub));
            }
        }
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            paths.push(PathBuf::from(&pf86).join("Java"));
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            paths.push(PathBuf::from(&profile).join(".jdks"));
        }
        // RTLauncher 内置 Java 下载目录
        paths.push(PathBuf::from("./RTL/java"));
    } else if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        paths.push(PathBuf::from("/System/Library/Java/JavaVirtualMachines"));
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join("Library/Java/JavaVirtualMachines"));
            paths.push(PathBuf::from(&home).join(".jdks"));
            // RTLauncher 内置 Java 下载目录
            paths.push(PathBuf::from(&home).join("Library/Application Support/RTLauncher/java"));
        }
    } else {
        paths.push(PathBuf::from("/usr/lib/jvm"));
        paths.push(PathBuf::from("/usr/java"));
        paths.push(PathBuf::from("/opt/java"));
        paths.push(PathBuf::from("/opt/jdk"));
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join(".jdks"));
            paths.push(PathBuf::from(&home).join(".sdkman/candidates/java"));
        }
        // RTLauncher 内置 Java 下载目录
        paths.push(PathBuf::from("./java"));
    }

    paths
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
