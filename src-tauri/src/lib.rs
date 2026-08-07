// Platform-specific paths expose a few helpers used by packaging integrations.
#[macro_use]
extern crate log;

#[allow(dead_code)]
mod app_paths;
mod auth;
mod downloader;
mod handler;
mod http_client;
mod mutiplayer;
mod updater;
mod version_management;
use auth::littleskinLoader::{useMethod, use_method_with_credentials};
use auth::official::{
    delete_cached_skin, get_skin_base64, ms_activate_skin, ms_cancel_login, ms_delete_skin,
    ms_get_skins_and_capes, ms_has_account_in_db, ms_poll_and_login, ms_request_device_code,
    ms_set_active_cape, ms_silent_refresh_account, ms_upload_skin, redownload_littleskin_skin,
};
use auth::yissadrail::{getAccountList, getPlayerSkin, thirdPartyLogin};
use downloader::decompression::extract_library_paths;
use downloader::dwPatch::{cancel_download, download_patcher};
use downloader::version_fetcher::classify_minecraft_versions;
use handler::cache_paths::{
    cache_to_instance, get_cache_dir, get_cache_dir_by_version, get_cache_root,
    get_mod_cache_dir_cmd, init_cache_dirs, instance_to_cache, list_cache_dirs, list_cached_files,
    list_cached_mods,
};
use handler::chinese_search::{get_moddata_info, search_moddata};
use handler::config::{
    get_java_download_dir, get_launcher_paths_config, save_launcher_paths_config,
};
use handler::fabric_handler::{
    cancel_fabric_download, download_and_install_fabric, get_fabric_api_versions,
    get_fabric_loader_versions,
};
use handler::forge_handler::{
    cancel_forge_download, download_and_install_forge, get_forge_versions,
};
use handler::java_downloader::{download_java_runtime, get_java_versions};
use handler::java_scanner::{search_java_installations, validate_java_path};
use handler::launcher::build_jvm_arguments;
use handler::launcher::kill_game_process;
use handler::launcher::launch_game;
use handler::liteloader_handler::{
    cancel_liteloader_download, download_and_install_liteloader, get_liteloader_versions,
};
use handler::mod_links::{
    cancel_mod_download, download_mod_file, download_resource_file, get_curseforge_mod_files,
    get_mod_files_by_slug, get_mod_links, get_modrinth_mod_files, search_curseforge_projects,
    search_modrinth_projects,
};
use handler::mod_parser::{parse_mod, parse_mods, parse_mods_in_dir, save_incompatible_mods};
use handler::modpack_builder::{
    delete_modpack_instance, export_modpack_instance, get_modpack_dir, list_modpack_instances,
    load_modpack_instance, rename_modpack_instance, save_modpack_instance,
};
use handler::modpack_installer_handler::{
    cancel_modpack_install, delete_cached_modpack_cmd, delete_version_dir_cmd,
    detect_modpack_format_cmd, install_modpack_from_zip_cmd, list_cached_modpacks_cmd,
    parse_modpack_cmd, save_modpack_to_cache_cmd,
};
use handler::neoforge_handler::{
    cancel_neoforge_download, download_and_install_neoforge, get_neoforge_versions,
};
use handler::optifine_handler::{
    cancel_optifine_download, download_and_install_optifine, get_optifine_version_names,
    get_optifine_versions, install_optifine,
};
use handler::quilt_handler::{
    cancel_quilt_download, download_and_install_quilt, get_quilt_api_versions,
    get_quilt_loader_versions,
};
use handler::system::{
    get_system_memory, open_external, optimize_memory_usage, read_file_base64, write_file,
};
use handler::diagnostics::{
    get_mod_dependencies_analysis, get_system_info, auto_download_missing_dependency,
    auto_download_all_missing_dependencies, search_missing_dependency,
    auto_download_with_dependencies, analyze_loader_logs, deep_analyze_with_api,
    export_launch_report, get_modrinth_required_dependencies, get_curseforge_required_dependencies,
    check_mod_installed,
};
use mutiplayer::{
    ensure_openp2p_stopped, mp_check_openp2p, mp_encode_room_info, mp_get_openp2p_dir,
    mp_get_openp2p_path, mp_install_openp2p, mp_is_openp2p_running, mp_poll_log,
    mp_start_openp2p_host, mp_start_openp2p_join, mp_stop_openp2p,
};
use updater::handler::{
    cancel_update, can_check_update, check_for_updates, create_updater_state, download_update,
    get_update_status, install_update,
};
use version_management::{
    vm_delete_cached_file, vm_delete_file, vm_ensure_instance_dirs, vm_find_resource_packs,
    vm_list_dir, vm_modify_game_rule, vm_parse_level_dat, vm_rename_file, vm_scan_instances,
    vm_write_file_base64,
};

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSColor, NSWindow};
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;

#[cfg(target_os = "macos")]
const NS_WINDOW_TITLE_HIDDEN: i64 = 1;
#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_FULL_SIZE_CONTENT_VIEW: u64 = 1 << 15;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 给 WebView、磁盘 I/O 和前台交互留下 CPU，避免低配设备在启动时争用。
    let cpu_count = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    let worker_threads = (cpu_count + 1).saturating_div(2).max(4);
    std::env::set_var("TOKIO_WORKER_THREADS", worker_threads.to_string());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            build_jvm_arguments,
            launch_game,
            kill_game_process,
            download_patcher,
            cancel_download,
            classify_minecraft_versions,
            extract_library_paths,
            useMethod,
            use_method_with_credentials,
            thirdPartyLogin,
            getAccountList,
            getPlayerSkin,
            ms_request_device_code,
            ms_poll_and_login,
            ms_cancel_login,
            get_skin_base64,
            redownload_littleskin_skin,
            ms_get_skins_and_capes,
            ms_upload_skin,
            ms_activate_skin,
            ms_delete_skin,
            ms_set_active_cape,
            ms_silent_refresh_account,
            ms_has_account_in_db,
            delete_cached_skin,
            mp_check_openp2p,
            mp_install_openp2p,
            mp_start_openp2p_host,
            mp_start_openp2p_join,
            mp_encode_room_info,
            mp_stop_openp2p,
            mp_is_openp2p_running,
            mp_poll_log,
            mp_get_openp2p_dir,
            mp_get_openp2p_path,
            vm_scan_instances,
            vm_find_resource_packs,
            vm_parse_level_dat,
            vm_modify_game_rule,
            vm_list_dir,
            vm_ensure_instance_dirs,
            vm_delete_file,
            vm_rename_file,
            vm_write_file_base64,
            vm_delete_cached_file,
            get_system_memory,
            optimize_memory_usage,
            open_external,
            read_file_base64,
            get_mod_dependencies_analysis,
            get_system_info,
            search_missing_dependency,
            auto_download_missing_dependency,
            auto_download_all_missing_dependencies,
            auto_download_with_dependencies,
            analyze_loader_logs,
            deep_analyze_with_api,
            check_mod_installed,
            get_launcher_paths_config,
            save_launcher_paths_config,
            write_file,
            get_java_versions,
            download_java_runtime,
            search_java_installations,
            validate_java_path,
            get_java_download_dir,
            get_optifine_versions,
            get_optifine_version_names,
            install_optifine,
            download_and_install_optifine,
            cancel_optifine_download,
            get_fabric_loader_versions,
            get_fabric_api_versions,
            download_and_install_fabric,
            cancel_fabric_download,
            get_quilt_loader_versions,
            get_quilt_api_versions,
            download_and_install_quilt,
            cancel_quilt_download,
            get_forge_versions,
            download_and_install_forge,
            cancel_forge_download,
            get_neoforge_versions,
            download_and_install_neoforge,
            cancel_neoforge_download,
            get_liteloader_versions,
            download_and_install_liteloader,
            cancel_liteloader_download,
            search_moddata,
            get_moddata_info,
            get_mod_links,
            get_curseforge_mod_files,
            get_mod_files_by_slug,
            get_modrinth_mod_files,
            download_mod_file,
            download_resource_file,
            cancel_mod_download,
            get_modpack_dir,
            save_modpack_instance,
            list_modpack_instances,
            load_modpack_instance,
            delete_modpack_instance,
            rename_modpack_instance,
            export_modpack_instance,
            detect_modpack_format_cmd,
            install_modpack_from_zip_cmd,
            cancel_modpack_install,
            parse_modpack_cmd,
            save_modpack_to_cache_cmd,
            list_cached_modpacks_cmd,
            delete_cached_modpack_cmd,
            delete_version_dir_cmd,
            get_cache_root,
            get_cache_dir,
            get_cache_dir_by_version,
            init_cache_dirs,
            list_cache_dirs,
            list_cached_files,
            get_mod_cache_dir_cmd,
            list_cached_mods,
            cache_to_instance,
            instance_to_cache,
            parse_mod,
            parse_mods,
            parse_mods_in_dir,
            save_incompatible_mods,
            search_curseforge_projects,
            search_modrinth_projects,
            export_launch_report,
            get_modrinth_required_dependencies,
            get_curseforge_required_dependencies,
            get_update_status,
            check_for_updates,
            download_update,
            install_update,
            cancel_update,
            can_check_update,
        ])
        .manage(create_updater_state())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(not(target_os = "macos"))]
            app.handle().plugin(tauri_plugin_single_instance::init(
                |app: &tauri::AppHandle, _args, _cwd| {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                },
            ))?;
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let window = if let Some(window) = app.get_webview_window("main") {
                window
            } else {
                let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("RTLauncher")
                    .inner_size(1280.0, 800.0)
                    .min_inner_size(1024.0, 640.0)
                    .center()
                    .resizable(true)
                    .fullscreen(false)
                    .shadow(true);

                #[cfg(target_os = "macos")]
                let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

                #[cfg(not(target_os = "macos"))]
                let win_builder = win_builder.decorations(false);

                win_builder.build()?
            };

            // 注册窗口关闭事件：关闭窗口时立即停止 openp2p 进程
            // 注意：这里只做一次快速的 killall（避免阻塞窗口关闭流程）
            // 完整的多层清理会在 tauri 退出后（下方）再做一次
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    #[cfg(target_os = "windows")]
                    {
                        use std::os::windows::process::CommandExt;
                        use winapi::um::winbase::CREATE_NO_WINDOW;
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/T", "/IM", "openp2p.exe"])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output();
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/T", "/IM", "openp2p"])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output();
                        let _ = std::process::Command::new("wmic")
                            .args(["process", "where", "name='openp2p.exe'", "delete"])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output();
                    }
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    {
                        // Linux / macOS: 快速 killall + pkill -9
                        let _ = std::process::Command::new("killall")
                            .args(["-9", "openp2p"])
                            .output();
                        let _ = std::process::Command::new("pkill")
                            .args(["-9", "-f", "openp2p"])
                            .output();
                    }
                }
            });

            #[cfg(not(target_os = "macos"))]
            let _ = &window;

            #[cfg(target_os = "macos")]
            unsafe {
                let ns_window_ptr = window.ns_window().unwrap() as *mut AnyObject;
                let ns_window = &*(ns_window_ptr as *const NSWindow);

                let () = msg_send![ns_window_ptr, setTitlebarAppearsTransparent: true];
                let () = msg_send![ns_window_ptr, setTitleVisibility: NS_WINDOW_TITLE_HIDDEN];

                let style_mask: u64 = msg_send![ns_window_ptr, styleMask];
                let style_mask = style_mask | NS_WINDOW_STYLE_MASK_FULL_SIZE_CONTENT_VIEW;
                let () = msg_send![ns_window_ptr, setStyleMask: style_mask];
                let () = msg_send![ns_window_ptr, setMovableByWindowBackground: false];

                let bg_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.0);
                ns_window.setBackgroundColor(Some(&bg_color));
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // tauri 退出后，确保 openp2p 也被停止（防止有保护线程残留）
    ensure_openp2p_stopped();
}