mod arguments;
mod identity;
mod java_runtime;
mod memory;
mod process;

use std::path::PathBuf;
use tauri::Manager;

use arguments::{build_jvm_arguments_inner, VersionJson};
use java_runtime::{
    get_java_major_version, major_version_to_runtime_name, required_java_major_from_jar,
};
use process::run_command;

pub use arguments::build_jvm_arguments;
pub use process::kill_game_process;

/// 启动游戏（构建参数并执行 Java 进程）
#[tauri::command]
pub fn launch_game(
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
    crate::handler::system::schedule_launcher_profiles_check();

    let mut resolved_java_path = java_path.to_string();
    let version_json_path = PathBuf::from(minecraft_path)
        .join("versions")
        .join(version_name)
        .join(format!("{}.json", version_name));

    let metadata_major: Option<u32> = (|| {
        let file = std::fs::File::open(&version_json_path).ok()?;
        let vj: VersionJson = serde_json::from_reader(file).ok()?;
        vj.java_version.map(|jv| jv.major_version)
    })();

    let version_jar_path = PathBuf::from(minecraft_path)
        .join("versions")
        .join(version_name)
        .join(format!("{}.jar", version_name));
    let jar_major = required_java_major_from_jar(&version_jar_path);
    let target_major = match (metadata_major, jar_major) {
        (Some(metadata), Some(actual)) if actual > metadata => {
            println!(
                "[启动器] 版本 JSON 声明 Java {}，但游戏 JAR 字节码需要 Java {}，以 JAR 为准",
                metadata, actual
            );
            Some(actual)
        }
        (Some(metadata), _) => Some(metadata),
        (None, actual) => actual,
    };

    if let Some(major) = target_major {
        println!(
            "[启动器] 版本 {} 的 javaVersion.majorVersion = {}",
            version_name, major
        );

        let current_major = crate::handler::java_scanner::validate_java_path(java_path.to_string())
            .ok()
            .map(|inst| inst.major_version);

        let needs_swap = match current_major {
            Some(m) => {
                println!("[启动器] 当前 Java 路径 {} 的大版本号 = {}", java_path, m);
                m != major as i32
            }
            None => {
                println!("[启动器] 当前 Java 路径 {} 无效或无法获取版本号", java_path);
                true
            }
        };

        if needs_swap {
            println!(
                "[启动器] Java 版本不匹配（需要 {}），正在搜索匹配的 Java 安装...",
                major
            );
            if let Some(matched) = crate::handler::java_scanner::find_java_by_major_version(major) {
                println!(
                    "[启动器] 已自动切换到匹配的 Java: {} (版本 {})",
                    matched.path, matched.version
                );
                resolved_java_path = matched.path;
            } else {
                let runtime_name = major_version_to_runtime_name(major);
                if let Some(rt_name) = runtime_name {
                    println!(
                        "[启动器] 未找到匹配的 Java {}，开始自动下载 {}...",
                        major, rt_name
                    );
                    let java_download_dir = crate::handler::config::get_java_download_dir()
                        .unwrap_or_else(|_| "./RTL/java".to_string());
                    let task_id = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let window = app
                        .get_webview_window("main")
                        .ok_or_else(|| "无法获取主窗口".to_string())?;
                    let rt_name_owned = rt_name.to_string();
                    let base_path = java_download_dir.clone();
                    let download_result = tokio::runtime::Handle::current().block_on(async move {
                        crate::handler::java_downloader::download_java_runtime(
                            rt_name_owned,
                            base_path,
                            task_id,
                            window,
                        )
                        .await
                    });
                    match download_result {
                        Ok(result) => {
                            println!("[启动器] Java 下载成功: {}", result.message);
                            resolved_java_path = result.java_path;
                        }
                        Err(e) => {
                            eprintln!(
                                "[启动器] Java 下载失败: {}，将继续使用当前路径 {}",
                                e, java_path
                            );
                        }
                    }
                } else {
                    println!(
                        "[启动器] Java {} 无对应运行时名称映射，将继续使用当前路径 {}",
                        major, java_path
                    );
                }
            }
        }
    } else {
        println!(
            "[启动器] 版本 {} 的版本 json 中未找到 javaVersion 字段",
            version_name
        );
        let java_major = get_java_major_version(java_path);
        println!(
            "[启动器] Java 路径 {} 的大版本号 = {}",
            java_path, java_major
        );
    }

    // 先构建参数
    let args = build_jvm_arguments_inner(
        app.clone(),
        minecraft_path,
        &resolved_java_path,
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
    .map_err(|e| e.to_string())?;

    // 再启动游戏
    run_command(
        args.clone(),
        PathBuf::from(&resolved_java_path),
        PathBuf::from(minecraft_path),
        app,
    )
    .map_err(|e| e.to_string())?;

    Ok(args.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{
        arguments::{append_loader_jvm_args, dedup_path_list, launcher_rules_allow},
        identity::launch_auth_identity,
        java_runtime::is_plausible_minecraft_version,
        memory::{safe_max_memory_mb, SafeMemoryLimit},
        process::display_exit_code,
    };
    use serde_json::json;

    #[test]
    fn removes_duplicate_jars_from_a_loader_classpath() {
        // PCL CE 生成的 NeoForge 实例 json 会携带字面量 -cp，其中同一
        // jar 可能因原版库与加载器库重复拼接而出现两次。
        let mut args = Vec::new();
        let classpath = format!(
            "{p}{p}{p2}{p3}",
            p = "C:/Users/Hill233/AppData/Roaming/.minecraft/libraries/com/google/code/gson/gson/2.10.1/gson-2.10.1.jar;",
            p2 = "C:/Users/Hill233/AppData/Roaming/.minecraft/libraries/com/mojang/logging/1.2.7/logging-1.2.7.jar;",
            p3 = "C:/Users/Hill233/AppData/Roaming/.minecraft/libraries/com/google/code/gson/gson/2.10.1/gson-2.10.1.jar"
        );

        append_loader_jvm_args(&mut args, &["-cp".to_string(), classpath.clone()]);

        let idx = args.iter().position(|a| a == "-cp").unwrap();
        let value = &args[idx + 1];
        assert_eq!(value.matches("gson-2.10.1.jar").count(), 1);
        assert_eq!(value.matches("logging-1.2.7.jar").count(), 1);
    }

    #[test]
    fn dedup_path_list_keeps_order_and_case_insensitive_duplicates() {
        let deduped = dedup_path_list(
            "C:/a.jar;C:/b.jar;c:/A.JAR;C:/c.jar;C:/b.jar",
        );
        assert_eq!(deduped, "C:/a.jar;C:/b.jar;C:/c.jar");
    }

    #[test]
    fn dedup_path_list_handles_unix_style_separator() {
        let deduped = dedup_path_list("/l/a.jar:/l/b.jar:/l/a.jar");
        assert_eq!(deduped, "/l/a.jar:/l/b.jar");
    }

    #[test]
    fn keeps_loader_module_path_when_vanilla_supplies_classpath() {
        let mut args = vec!["-cp".to_string(), "C:/Minecraft/libraries/*".to_string()];
        let loader_args = vec![
            "-p".to_string(),
            "C:/Minecraft/libraries/bootstrap.jar;C:/Minecraft/libraries/asm-commons.jar"
                .to_string(),
            "--add-modules".to_string(),
            "ALL-MODULE-PATH".to_string(),
        ];

        append_loader_jvm_args(&mut args, &loader_args);

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-p" && pair[1].contains("asm-commons.jar")));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--add-modules" && pair[1] == "ALL-MODULE-PATH"));
    }

    #[test]
    fn preserves_a_module_path_containing_spaces_as_one_argument() {
        let mut args = Vec::new();
        let module_path = "C:/Users/Test User/.minecraft/libraries/bootstrap.jar".to_string();

        append_loader_jvm_args(&mut args, &["-p".to_string(), module_path.clone()]);

        assert_eq!(args, vec!["-p".to_string(), module_path]);
    }

    #[test]
    fn keeps_an_existing_effective_module_path() {
        let mut args = vec!["-p".to_string(), "effective-module-path".to_string()];

        append_loader_jvm_args(
            &mut args,
            &["-p".to_string(), "loader-module-path".to_string()],
        );

        assert_eq!(args, vec!["-p", "effective-module-path"]);
    }

    #[test]
    fn skips_demo_argument_rule_for_a_normal_launch() {
        let rules = json!([
            {
                "action": "allow",
                "features": { "is_demo_user": true }
            }
        ]);

        assert!(!launcher_rules_allow(Some(&rules)));
    }

    #[test]
    fn accepts_an_unconditional_allow_rule() {
        let rules = json!([{ "action": "allow" }]);

        assert!(launcher_rules_allow(Some(&rules)));
    }

    #[test]
    fn validates_base_minecraft_version_formats() {
        for version in ["1.20.1", "1.21-rc1", "25w42a", "26", "26.3-snapshot-5"] {
            assert!(is_plausible_minecraft_version(version), "{version}");
        }
        for instance_name in ["PVZ_Survive", "SHser-Basic-Package", "fabric-loader-0.15.0"] {
            assert!(
                !is_plausible_minecraft_version(instance_name),
                "{instance_name}"
            );
        }
    }

    #[test]
    fn chooses_auth_identity_that_matches_the_account_type() {
        assert_eq!(
            launch_auth_identity("third-party-token", "https://example.invalid/api/yggdrasil"),
            ("mojang", "{}")
        );
        assert_eq!(launch_auth_identity("0", ""), ("legacy", "{}"));
        assert_eq!(launch_auth_identity("microsoft-token", ""), ("msa", "{}"));
    }

    #[test]
    fn limits_heap_to_memory_available_after_system_reserve() {
        // 4GB 设备、约 2.3GB 当前可用时，不能预分配 4GB 堆。
        assert_eq!(
            safe_max_memory_mb(4096, 3819, 2335),
            SafeMemoryLimit::Limited(1823)
        );
        assert_eq!(
            safe_max_memory_mb(1024, 3819, 2335),
            SafeMemoryLimit::Limited(1024)
        );
    }

    #[test]
    fn distinguishes_unknown_and_insufficient_memory() {
        assert_eq!(safe_max_memory_mb(4096, 0, 2048), SafeMemoryLimit::Unknown);
        assert_eq!(
            safe_max_memory_mb(4096, 3819, 800),
            SafeMemoryLimit::Insufficient {
                min_available_mb: 1024
            }
        );
    }

    #[test]
    #[cfg(unix)]
    fn preserves_unix_termination_signal_in_exit_code() {
        use std::os::unix::process::ExitStatusExt;

        assert_eq!(display_exit_code(std::process::ExitStatus::from_raw(0)), 0);
        assert_eq!(display_exit_code(std::process::ExitStatus::from_raw(6)), -6);
    }
}
