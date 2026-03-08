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

/// 获取Java版本信息
fn get_java_version(java_path: &str) -> Option<JavaInstallation> {
    // 在Windows上，确保路径存在
    if !std::path::Path::new(java_path).exists() {
        return None;
    }

    let output = Command::new(java_path)
        .arg("-version")
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_output = if !stderr.is_empty() { stderr } else { stdout };

    // 解析版本号
    let version_line = version_output.lines().next()?;

    // 提取完整版本字符串
    let version = if let Some(start) = version_line.find('"') {
        if let Some(end) = version_line[start + 1..].find('"') {
            version_line[start + 1..start + 1 + end].to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    // 解析主版本号
    let major_version = parse_major_version(&version)?;

    // 提取供应商信息
    let vendor = version_output
        .lines()
        .find(|line| line.contains("Runtime") || line.contains("VM"))
        .and_then(|line| {
            if line.contains("OpenJDK") {
                Some("OpenJDK")
            } else if line.contains("Oracle") {
                Some("Oracle")
            } else if line.contains("Azul") {
                Some("Azul Zulu")
            } else if line.contains("Amazon") {
                Some("Amazon Corretto")
            } else if line.contains("Eclipse") {
                Some("Eclipse Temurin")
            } else {
                Some("Unknown")
            }
        })
        .unwrap_or("Unknown")
        .to_string();

    // 检测架构 - 使用更安全的方式
    let architecture = if let Ok(arch_output) = Command::new(java_path)
        .arg("-XshowSettings:properties")
        .arg("-version")
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
    {
        let arch_stderr = String::from_utf8_lossy(&arch_output.stderr);
        if arch_stderr.contains("amd64") || arch_stderr.contains("x86_64") {
            "x64"
        } else if arch_stderr.contains("aarch64") || arch_stderr.contains("arm64") {
            "ARM64"
        } else if arch_stderr.contains("x86") {
            "x86"
        } else {
            "Unknown"
        }
    } else {
        "Unknown"
    }
    .to_string();

    Some(JavaInstallation {
        path: java_path.to_string(),
        version,
        major_version,
        vendor,
        architecture,
    })
}

/// 解析主版本号
fn parse_major_version(version: &str) -> Option<i32> {
    // 处理 "1.8.0_xxx" 格式
    if version.starts_with("1.") {
        version
            .split('.')
            .nth(1)
            .and_then(|s| s.split('_').next())
            .and_then(|s| s.parse().ok())
    } else {
        // 处理 "11.0.x", "17.0.x" 等格式
        version
            .split('.')
            .next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse().ok())
    }
}

/// 搜索系统中的Java安装
#[tauri::command]
pub async fn search_java_installations() -> Result<Vec<JavaInstallation>, String> {
    let mut installations = Vec::new();
    let search_paths = get_search_paths();

    // 搜索指定目录
    for search_path in search_paths {
        if !search_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&search_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // 查找 bin/java 或 bin/java.exe
                let java_exe = if cfg!(windows) {
                    path.join("bin").join("java.exe")
                } else {
                    path.join("bin").join("java")
                };

                if java_exe.exists() {
                    if let Some(java_str) = java_exe.to_str() {
                        if let Some(installation) = get_java_version(java_str) {
                            // 避免重复
                            if !installations.iter().any(|i: &JavaInstallation| i.path == installation.path) {
                                installations.push(installation);
                            }
                        }
                    }
                }
            }
        }
    }

    // 检查环境变量中的Java
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_exe = if cfg!(windows) {
            PathBuf::from(&java_home).join("bin").join("java.exe")
        } else {
            PathBuf::from(&java_home).join("bin").join("java")
        };

        if java_exe.exists() {
            if let Some(java_str) = java_exe.to_str() {
                if let Some(installation) = get_java_version(java_str) {
                    if !installations.iter().any(|i| i.path == installation.path) {
                        installations.push(installation);
                    }
                }
            }
        }
    }

    // 检查PATH中的java
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
                let java_path = line.trim();
                if !java_path.is_empty() && Path::new(java_path).exists() {
                    if let Some(installation) = get_java_version(java_path) {
                        if !installations.iter().any(|i| i.path == installation.path) {
                            installations.push(installation);
                        }
                    }
                }
            }
        }
    }

    // 按主版本号排序
    installations.sort_by(|a, b| b.major_version.cmp(&a.major_version));

    Ok(installations)
}

/// 获取不同平台的搜索路径
fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if cfg!(windows) {
        // Windows搜索路径
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            paths.push(PathBuf::from(&program_files).join("Java"));
            paths.push(PathBuf::from(&program_files).join("Eclipse Adoptium"));
            paths.push(PathBuf::from(&program_files).join("Eclipse Foundation"));
            paths.push(PathBuf::from(&program_files).join("Zulu"));
            paths.push(PathBuf::from(&program_files).join("Amazon Corretto"));
            paths.push(PathBuf::from(&program_files).join("BellSoft"));
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            paths.push(PathBuf::from(&program_files_x86).join("Java"));
        }
        // 用户目录下的Java
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            paths.push(PathBuf::from(&user_profile).join(".jdks"));
        }
    } else if cfg!(target_os = "macos") {
        // macOS搜索路径
        paths.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        paths.push(PathBuf::from("/System/Library/Java/JavaVirtualMachines"));
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join("Library/Java/JavaVirtualMachines"));
            paths.push(PathBuf::from(&home).join(".jdks"));
        }
    } else {
        // Linux搜索路径
        paths.push(PathBuf::from("/usr/lib/jvm"));
        paths.push(PathBuf::from("/usr/java"));
        paths.push(PathBuf::from("/opt/java"));
        paths.push(PathBuf::from("/opt/jdk"));
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join(".jdks"));
            paths.push(PathBuf::from(&home).join(".sdkman/candidates/java"));
        }
    }

    paths
}

/// 验证Java路径是否有效
#[tauri::command]
pub fn validate_java_path(java_path: String) -> Result<JavaInstallation, String> {
    if java_path.is_empty() {
        return Err("Java路径不能为空".to_string());
    }

    let path = PathBuf::from(&java_path);

    // 如果是目录，尝试找到java可执行文件
    let java_exe = if path.is_dir() {
        if cfg!(windows) {
            path.join("bin").join("java.exe")
        } else {
            path.join("bin").join("java")
        }
    } else {
        path
    };

    if !java_exe.exists() {
        return Err(format!("Java可执行文件不存在: {}", java_exe.display()));
    }

    let java_str = java_exe.to_str()
        .ok_or_else(|| "无效的路径格式".to_string())?;

    get_java_version(java_str)
        .ok_or_else(|| "无法获取Java版本信息，请确保这是有效的Java安装".to_string())
}
