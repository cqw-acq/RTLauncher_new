use crate::downloader::original_dwl::process_version;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

static TASK_COUNTER: AtomicU64 = AtomicU64::new(1);

struct ActiveTaskInfo {
    cancel: Arc<AtomicBool>,
    #[allow(dead_code)]
    mc_version: String,
    #[allow(dead_code)]
    minecraft_path: PathBuf,
}

fn active_tasks() -> &'static Mutex<HashMap<u64, ActiveTaskInfo>> {
    static INSTANCE: OnceLock<Mutex<HashMap<u64, ActiveTaskInfo>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

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

    let cancel = Arc::new(AtomicBool::new(false));

    // 注册活跃任务
    {
        let mut tasks = active_tasks().lock().unwrap();
        tasks.insert(task_id, ActiveTaskInfo {
            cancel: cancel.clone(),
            mc_version: mc_version.clone(),
            minecraft_path: minecraft_path.clone(),
        });
    }

    // 接收进度并通过 Tauri 事件发送到前端（带 task_id）
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit("download-progress", DownloadProgressPayload { task_id, percent });
        }
    });

    let app_finish = app.clone();
    let version = mc_version.clone();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        let result = process_version(&version, &minecraft_path, tx, cancel_clone.clone()).await;

        // 移除活跃任务
        {
            let mut tasks = active_tasks().lock().unwrap();
            tasks.remove(&task_id);
        }

        let was_cancelled = cancel_clone.load(Ordering::SeqCst);

        if was_cancelled {
            // 清理版本专属文件
            let version_dir = minecraft_path.join("versions").join(&version);
            let instance_dir = minecraft_path.join("instance").join(&version);
            let _ = std::fs::remove_dir_all(&version_dir);
            let _ = std::fs::remove_dir_all(&instance_dir);

            let _ = app_finish.emit("download-finished", DownloadFinishedPayload {
                task_id,
                success: false,
                error: Some("已取消".to_string()),
                failed_count: 0,
            });
        } else {
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
        }
    });

    // 立即返回 task_id，不阻塞前端
    Ok(task_id)
}

#[tauri::command]
pub async fn cancel_download(task_id: u64) -> Result<(), String> {
    let tasks = active_tasks().lock().map_err(|e| e.to_string())?;
    if let Some(info) = tasks.get(&task_id) {
        info.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}
