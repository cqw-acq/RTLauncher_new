mod handler;
mod downloader;
use handler::launcher::build_jvm_arguments;
use downloader::original_dwl::download_task;
use downloader::version_fetcher::classify_minecraft_versions;
use downloader::decompression::extract_library_paths;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![build_jvm_arguments,:download_task,classify_minecraft_versions,extract_library_paths])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
