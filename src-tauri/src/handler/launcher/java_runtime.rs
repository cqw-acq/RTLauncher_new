use regex::Regex;
use std::{io::Read, sync::OnceLock};

/// 运行 java -version 并提取大版本号（第一个 . 之前的数字）
pub(super) fn get_java_major_version(java_path: &str) -> String {
    let output = std::process::Command::new(java_path)
        .arg("-version")
        // 清除可能注入 --add-opens 等参数的全局 JAVA_* 选项环境变量：
        // 这些选项会让部分 JVM（尤其是 Java 8，不识别 --add-opens）在
        // `java -version` 时直接启动失败，导致 JVM 识别不可靠。
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stderr);
            let text = text.trim();
            // 典型输出: openjdk version "17.0.9" 或 java version "1.8.0_301"
            for line in text.lines() {
                let lower = line.to_lowercase();
                if let Some(ver_start) = lower.find("version") {
                    let after = &line[ver_start + "version".len()..].trim();
                    let after = after.trim_matches(|c: char| c == '"' || c == ' ');
                    // 提取第一个 . 之前的数字
                    if let Some(dot) = after.find('.') {
                        let major = &after[..dot];
                        if major.chars().all(|c| c.is_ascii_digit()) {
                            return major.to_string();
                        }
                    }
                    // 没有 . 的情况，整段作为版本号
                    if after.chars().all(|c| c.is_ascii_digit()) {
                        return after.to_string();
                    }
                }
            }
            "未知".to_string()
        }
        Err(e) => format!("获取失败: {}", e),
    }
}

pub(super) fn major_version_to_runtime_name(major: u32) -> Option<&'static str> {
    match major {
        8 => Some("java-runtime-alpha"),
        11 => Some("java-runtime-beta"),
        17 => Some("java-runtime-gamma"),
        21 => Some("java-runtime-delta"),
        _ => None,
    }
}

/// 从游戏 JAR 的 class 文件推断真实 Java 需求，避免整合包中过期的
/// `javaVersion` 导致启动器选择过旧的 Java。
pub(super) fn required_java_major_from_jar(jar_path: &std::path::Path) -> Option<u32> {
    let file = std::fs::File::open(jar_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    // 扫描整个 JAR 取最大字节码版本：整合包 JAR 可能混有不同 javac 编译的类，
    // 只读第一个类文件会低估所需 Java 版本（如先扫到 Java 25 的类、后遇到 Java 26 的类）。
    let mut max_major = 0_u32;
    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.name().ends_with(".class") || entry.name().starts_with("META-INF/versions/") {
            continue;
        }
        let mut header = [0_u8; 8];
        if entry.read_exact(&mut header).is_err() || header[..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
            continue;
        }
        let major = (u16::from_be_bytes([header[6], header[7]]) as u32).checked_sub(44);
        if let Some(m) = major {
            max_major = max_major.max(m);
        }
    }
    (max_major > 0).then_some(max_major)
}

pub(super) fn is_plausible_minecraft_version(version: &str) -> bool {
    static VERSION_PATTERN: OnceLock<Regex> = OnceLock::new();
    VERSION_PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:\d+\.\d+(?:\.\d+)?(?:-(?:pre|rc)\d+)?|\d{2}w\d{2}[a-z]|\d{2,}(?:\.\d+)?(?:-snapshot(?:-\d+)?)?)$",
            )
            .expect("valid Minecraft version regex")
        })
        .is_match(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn unique_temp_path(name: &str) -> std::path::PathBuf {
        let unique = format!(
            "rtlauncher-test-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after the Unix epoch")
                .as_nanos(),
        );
        std::env::temp_dir().join(unique)
    }

    /// 构造一个最小的 8 字节 class 文件头：CAFEBABE + minor(0) + major。
    fn class_header(major_version: u16) -> [u8; 8] {
        let mut header = [0u8; 8];
        header[0..4].copy_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        header[4..6].copy_from_slice(&0u16.to_be_bytes());
        header[6..8].copy_from_slice(&major_version.to_be_bytes());
        header
    }

    fn write_jar(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("create temp jar");
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in entries {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(*name, options).expect("start_file");
            zip.write_all(data).expect("write class bytes");
        }
        zip.finish().expect("finish zip");
    }

    #[test]
    fn major_version_to_runtime_name_maps_known_versions() {
        assert_eq!(major_version_to_runtime_name(8), Some("java-runtime-alpha"));
        assert_eq!(major_version_to_runtime_name(11), Some("java-runtime-beta"));
        assert_eq!(major_version_to_runtime_name(17), Some("java-runtime-gamma"));
        assert_eq!(major_version_to_runtime_name(21), Some("java-runtime-delta"));
    }

    #[test]
    fn major_version_to_runtime_name_returns_none_for_unmapped_versions() {
        assert_eq!(major_version_to_runtime_name(9), None);
        assert_eq!(major_version_to_runtime_name(0), None);
        assert_eq!(major_version_to_runtime_name(25), None);
    }

    #[test]
    fn is_plausible_minecraft_version_accepts_common_formats() {
        for version in ["1.20.1", "1.7", "1.21-rc1", "1.21-pre3", "24w45a", "26.1-snapshot-2"] {
            assert!(is_plausible_minecraft_version(version), "{version}");
        }
    }

    #[test]
    fn is_plausible_minecraft_version_rejects_instance_and_loader_names() {
        for name in ["Forge-1.20.1", "my modpack", "quilt-loader-0.20.0", ""] {
            assert!(!is_plausible_minecraft_version(name), "{name}");
        }
    }

    #[test]
    fn get_java_major_version_reports_failure_for_a_missing_binary() {
        let result = get_java_major_version("/definitely/not/a/real/java-binary-xyz");
        assert!(result.starts_with("获取失败"), "{result}");
    }

    #[test]
    fn required_java_major_from_jar_returns_none_for_a_missing_file() {
        let path = unique_temp_path("missing.jar");
        assert_eq!(required_java_major_from_jar(&path), None);
    }

    #[test]
    fn required_java_major_from_jar_returns_none_for_a_non_zip_file() {
        let path = unique_temp_path("not-a-zip.jar");
        std::fs::write(&path, b"this is not a zip archive").unwrap();
        let result = required_java_major_from_jar(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, None);
    }

    #[test]
    fn required_java_major_from_jar_picks_the_highest_bytecode_version() {
        let path = unique_temp_path("mixed-major.jar");
        write_jar(
            &path,
            &[
                ("com/example/Java8Class.class", &class_header(52)), // Java 8
                ("com/example/Java21Class.class", &class_header(65)), // Java 21
                ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n"),
            ],
        );
        let result = required_java_major_from_jar(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Some(21));
    }

    #[test]
    fn required_java_major_from_jar_ignores_multi_release_classes() {
        let path = unique_temp_path("multi-release.jar");
        write_jar(
            &path,
            &[
                ("com/example/Base.class", &class_header(52)), // Java 8
                (
                    "META-INF/versions/17/com/example/Base.class",
                    &class_header(61), // Java 17, should be ignored due to the path prefix
                ),
            ],
        );
        let result = required_java_major_from_jar(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn required_java_major_from_jar_returns_none_when_no_class_entries_are_valid() {
        let path = unique_temp_path("no-classes.jar");
        write_jar(
            &path,
            &[
                ("README.txt", b"just a text file"),
                ("com/example/Corrupt.class", b"not-a-real-class-header"),
            ],
        );
        let result = required_java_major_from_jar(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, None);
    }
}
