use crate::downloader::dwPatch::get_minecraft_dir;
use crate::downloader::forge_installer;
use crate::downloader::mod_loader_installer_shared::pick_java_executable;
use crate::downloader::original_dwl::process_version;
use crate::downloader::shared_utils::{
    cleanup_loader_version_dir, merge_version_jsons_to_instance, sanitize_instance_name,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
#[derive(Debug, Serialize, Deserialize)]
pub struct ForgeInstallResult {
    pub message: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ForgeVersion {
    pub id: String,
    pub version: String,
}
#[derive(Clone, Serialize)]
struct ForgeDownloadProgressPayload {
    task_id: u64,
    percent: f64,
}
#[derive(Clone, Serialize)]
struct ForgeDownloadFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
}
static FORGE_TASK_COUNTER: AtomicU64 = AtomicU64::new(4000000);
struct ForgeActiveTaskInfo {
    cancel: Arc<AtomicBool>,
}
fn forge_active_tasks() -> &'static Mutex<HashMap<u64, ForgeActiveTaskInfo>> {
    static INSTANCE: OnceLock<Mutex<HashMap<u64, ForgeActiveTaskInfo>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}
#[tauri::command]
pub async fn get_forge_versions(mc_version: String) -> Result<Vec<ForgeVersion>, String> {
    let version_names = forge_installer::get_forge_versions(&mc_version)
        .await
        .map_err(|e| format!("获取 Forge 版本失败: {}", e))?;
    Ok(version_names
        .into_iter()
        .map(|v| ForgeVersion {
            id: v.clone(),
            version: v,
        })
        .collect())
}
#[tauri::command]
pub async fn download_and_install_forge(
    app: AppHandle,
    mc_version: String,
    forge_version: String,
    instance_name: Option<String>,
) -> Result<u64, String> {
    let task_id = FORGE_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let minecraft_path = get_minecraft_dir()?;
    std::fs::create_dir_all(&minecraft_path).map_err(|e| format!("创建目录失败: {}", e))?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut tasks = forge_active_tasks().lock().unwrap();
        tasks.insert(
            task_id,
            ForgeActiveTaskInfo {
                cancel: cancel.clone(),
            },
        );
    }
    let app_clone = app.clone();
    let task_id_clone = task_id;
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit(
                "forge-download-progress",
                ForgeDownloadProgressPayload {
                    task_id: task_id_clone,
                    percent,
                },
            );
        }
    });
    let _ = app.emit(
        "forge-download-progress",
        ForgeDownloadProgressPayload {
            task_id,
            percent: 0.0,
        },
    );
    let app_finish = app.clone();
    let version = mc_version.clone();
    let forge_ver = forge_version.clone();
    let cancel_clone = cancel.clone();
    let minecraft_path_clone = minecraft_path.clone();
    let instance_name_cloned = instance_name.clone();
    tokio::spawn(async move {
        let original_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let original_ready_clone = original_ready.clone();
        let cancel_clone_a = cancel_clone.clone();
        let version_a = version.clone();
        let mc_path_a = minecraft_path_clone.clone();
        let tx_for_original = tx.clone();
        let original_handle = tokio::spawn(async move {
            let (tx1, mut rx1) = tokio::sync::mpsc::channel::<f64>(64);
            tokio::spawn(async move {
                while let Some(percent) = rx1.recv().await {
                    let _ = tx_for_original.send(percent * 0.8).await;
                }
            });
            let result = process_version(&version_a, &mc_path_a, tx1, cancel_clone_a.clone())
                .await
                .map(|_| ())
                .map_err(|e| e.to_string());
            original_ready_clone.store(true, Ordering::SeqCst);
            result
        });
        let tx_for_forge = tx.clone();
        let version_b = version.clone();
        let forge_ver_b = forge_ver.clone();
        let mc_path_b = minecraft_path_clone.clone();
        let forge_handle = tokio::spawn(async move {
            let (tx2, mut rx2) = tokio::sync::mpsc::channel::<f64>(64);
            tokio::spawn(async move {
                while let Some(percent) = rx2.recv().await {
                    let _ = tx_for_forge.send(80.0 + percent * 0.2).await;
                }
            });
            let mc_path_str = mc_path_b.to_string_lossy().to_string();
            let java = pick_java_executable(&version_b);
            forge_installer::install_forge(
                &version_b,
                &forge_ver_b,
                &mc_path_str,
                &java,
                Some(tx2),
                Some(original_ready),
            )
            .await
            .map_err(|e| e.to_string())
        });
        let (original_result, forge_result) = tokio::join!(original_handle, forge_handle);
        let original_result = original_result.unwrap_or_else(|e| Err(e.to_string()));
        let forge_result = forge_result.unwrap_or_else(|e| Err(e.to_string()));
        {
            let mut tasks = forge_active_tasks().lock().unwrap();
            tasks.remove(&task_id);
        }
        let was_cancelled = cancel_clone.load(Ordering::SeqCst);
        if was_cancelled {
            let _ = app_finish.emit(
                "forge-download-finished",
                ForgeDownloadFinishedPayload {
                    task_id,
                    success: false,
                    error: Some("已取消".to_string()),
                },
            );
            return;
        }
        if let Err(e) = original_result {
            let _ = app_finish.emit(
                "forge-download-finished",
                ForgeDownloadFinishedPayload {
                    task_id,
                    success: false,
                    error: Some(format!("原版下载失败: {}", e)),
                },
            );
            return;
        }
        match forge_result {
            Ok(loader_version) => {
                let _ = tx.send(100.0).await;
                println!("Forge 安装成功: {}", forge_ver);
                
                if let Some(inst_name) = instance_name_cloned {
                    let clean_name = sanitize_instance_name(&inst_name);
                    println!("[Forge] 创建实例目录: {}", clean_name);
                    let final_name = if clean_name.trim().is_empty() {
                        sanitize_instance_name(&loader_version)
                    } else {
                        clean_name
                    };
                    match merge_version_jsons_to_instance(
                        &final_name,
                        &version,
                        &loader_version,
                        "forge",
                        &minecraft_path_clone,
                    ) {
                        Ok(_) => {
                            println!("[Forge] 实例 JSON 合并完成: {}", final_name);
                            if final_name != loader_version {
                                cleanup_loader_version_dir(&loader_version, &minecraft_path_clone);
                            }
                        }
                        Err(e) => println!("[Forge] 警告: 合并实例 JSON 失败: {}", e),
                    }
                }
                
                let _ = app_finish.emit(
                    "forge-download-finished",
                    ForgeDownloadFinishedPayload {
                        task_id,
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                println!("Forge 安装失败: {}", e);
                let _ = app_finish.emit(
                    "forge-download-finished",
                    ForgeDownloadFinishedPayload {
                        task_id,
                        success: false,
                        error: Some(e),
                    },
                );
            }
        }
    });
    Ok(task_id)
}
#[tauri::command]
pub async fn cancel_forge_download(taskId: u64) -> Result<(), String> {
    let tasks = forge_active_tasks().lock().map_err(|e| e.to_string())?;
    if let Some(info) = tasks.get(&taskId) {
        info.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}
