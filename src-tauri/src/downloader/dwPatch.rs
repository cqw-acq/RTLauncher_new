use crate::downloader::original_dwl::process_version;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

static TASK_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
struct DownloadProgressPayload {
    task_id: u64,
    percent: f64,
}

#[derive(Clone, Serialize)]
struct DownloadFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
    /// 失败的文件数量
    failed_count: usize,
}

/// 获取平台对应的 Minecraft 数据目录
/// - macOS:   ~/Library/Application Support/RTLauncher
/// - Linux:   ~/.minecraft
/// - Windows: %APPDATA%\.minecraft
pub fn get_minecraft_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "无法获取 HOME 环境变量".to_string())?;
        Ok(PathBuf::from(home).join("Library/Application Support/RTLauncher/version"))
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").map_err(|_| "无法获取 HOME 环境变量".to_string())?;
        Ok(PathBuf::from(home).join(".minecraft"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA 环境变量".to_string())?;
        Ok(PathBuf::from(appdata).join(".minecraft"))
    }
}

#[tauri::command]
pub async fn download_patcher(app: AppHandle, mc_version: String) -> Result<u64, String> {
    let task_id = TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let minecraft_path = get_minecraft_dir()?;
    std::fs::create_dir_all(&minecraft_path).map_err(|e| format!("创建目录失败: {}", e))?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);

    // 接收进度并通过 Tauri 事件发送到前端（带 task_id）
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit("download-progress", DownloadProgressPayload { task_id, percent });
        }
    });

    let app_finish = app.clone();
    let version = mc_version.clone();
    tokio::spawn(async move {
        let result = process_version(&version, &minecraft_path, tx).await;
        match result {
            Ok(warnings) => {
                let failed_count = warnings.len();
                let _ = app_finish.emit("download-finished", DownloadFinishedPayload {
                    task_id,
                    success: true,
                    error: if failed_count > 0 {
                        Some(format!("{} 个文件下载失败", failed_count))
                    } else {
                        None
                    },
                    failed_count,
                });
            }
            Err(e) => {
                let _ = app_finish.emit("download-finished", DownloadFinishedPayload {
                    task_id,
                    success: false,
                    error: Some(e.to_string()),
                    failed_count: 0,
                });
            }
        }
    });

    // 立即返回 task_id，不阻塞前端
    Ok(task_id)
}
