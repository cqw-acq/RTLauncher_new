use crate::downloader::dwPatch::get_minecraft_dir;
use crate::downloader::optifine_installer::{
    self, OptiFineVersionInfo,
};
use crate::downloader::original_dwl::process_version;
use crate::downloader::shared_utils::{merge_version_jsons_to_instance, sanitize_instance_name};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize)]
pub struct OptifineInstallResult {
    pub message: String,
    pub version_folder: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptifineVersion {
    pub id: String,
    pub filename: String,
    pub self_version: String,
    pub full_version: String,
    pub download_url: String,
    pub official_url: String,
    pub is_pre: bool,
}

#[derive(Clone, Serialize)]
struct OptifineDownloadProgressPayload {
    task_id: u64,
    percent: f64,
}

#[derive(Clone, Serialize)]
struct OptifineDownloadFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
    version_folder: Option<String>,
}

static OPTIFINE_TASK_COUNTER: AtomicU64 = AtomicU64::new(1000000);

struct OptifineActiveTaskInfo {
    cancel: Arc<AtomicBool>,
}

fn optifine_active_tasks() -> &'static Mutex<HashMap<u64, OptifineActiveTaskInfo>> {
    static INSTANCE: OnceLock<Mutex<HashMap<u64, OptifineActiveTaskInfo>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub async fn get_optifine_versions(mc_version: String) -> Result<Vec<OptifineVersion>, String> {
    let versions = optifine_installer::get_optifine_versions(&mc_version)
        .await
        .map_err(|e| format!("获取 OptiFine 版本列表失败: {}", e))?;

    Ok(versions
        .into_iter()
        .map(|v: OptiFineVersionInfo| OptifineVersion {
            id: v.download_url.clone(),
            filename: format!(
                "OptiFine_{}_{}.jar",
                v.game_version, v.self_version
            ),
            self_version: v.self_version,
            full_version: v.full_version,
            download_url: v.download_url,
            official_url: v.official_url,
            is_pre: v.is_pre,
        })
        .collect())
}

#[tauri::command]
pub async fn get_optifine_version_names(mc_version: String) -> Result<Vec<String>, String> {
    let versions = optifine_installer::get_optifine_versions(&mc_version)
        .await
        .map_err(|e| format!("获取 OptiFine 版本列表失败: {}", e))?;
    Ok(versions
        .into_iter()
        .map(|v| format!("OptiFine_{}_{}.jar", v.game_version, v.self_version))
        .collect())
}

#[tauri::command]
pub async fn install_optifine(
    optifine_version: String,
    minecraft_dir: String,
    mc_version: String,
    optifine_fallback_url: Option<String>,
    _window: tauri::Window,
) -> Result<OptifineInstallResult, String> {
    let mc_dir = std::path::PathBuf::from(&minecraft_dir);
    let version_folder = if optifine_version.starts_with("http://")
        || optifine_version.starts_with("https://")
    {
        let fallback = optifine_fallback_url.unwrap_or_else(|| optifine_version.clone());
        optifine_installer::install_optifine_from_bmcl_with_fallback(
            &mc_version,
            &optifine_version,
            &fallback,
            &mc_dir,
        )
        .await
        .map_err(|e| format!("安装 OptiFine 失败: {}", e))?
    } else {
        let installer_path = std::path::PathBuf::from(&optifine_version);
        optifine_installer::install_optifine_from_local(
            &installer_path,
            &mc_dir,
            &mc_version,
        )
        .await
        .map_err(|e| format!("安装 OptiFine 失败: {}", e))?
    };

    Ok(OptifineInstallResult {
        message: format!("OptiFine {} 已成功安装", optifine_version),
        version_folder: Some(version_folder),
    })
}

#[tauri::command]
pub async fn download_and_install_optifine(
    app: AppHandle,
    optifine_version: String,
    mc_version: String,
    instance_name: Option<String>,
    optifine_fallback_url: Option<String>,
) -> Result<u64, String> {
    let task_id = OPTIFINE_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let minecraft_path = get_minecraft_dir()?;
    std::fs::create_dir_all(&minecraft_path)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut tasks = optifine_active_tasks().lock().unwrap();
        tasks.insert(
            task_id,
            OptifineActiveTaskInfo {
                cancel: cancel.clone(),
            },
        );
    }

    let app_clone = app.clone();
    let task_id_clone = task_id;
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit(
                "optifine-download-progress",
                OptifineDownloadProgressPayload {
                    task_id: task_id_clone,
                    percent,
                },
            );
        }
    });

    let _ = app.emit(
        "optifine-download-progress",
        OptifineDownloadProgressPayload {
            task_id,
            percent: 0.0,
        },
    );

    let app_finish = app.clone();
    let version = mc_version.clone();
    let optifine_ver = optifine_version.clone();
    let cancel_clone = cancel.clone();
    let minecraft_path_clone = minecraft_path.clone();
    let instance_name_cloned = instance_name.clone();

    tokio::spawn(async move {
        let _ = tx.send(1.0).await;

        let result = {
            let (tx1, mut rx1) = tokio::sync::mpsc::channel::<f64>(64);
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                while let Some(percent) = rx1.recv().await {
                    let _ = tx_clone.send(1.0 + percent * 0.44).await;
                }
            });
            process_version(&version, &minecraft_path_clone, tx1, cancel_clone.clone()).await
        };

        if cancel_clone.load(Ordering::SeqCst) {
            emit_finish(
                &app_finish,
                task_id,
                false,
                Some("已取消".to_string()),
                None,
            );
            return;
        }

        if let Err(e) = result {
            emit_finish(
                &app_finish,
                task_id,
                false,
                Some(format!("原版下载失败: {}", e)),
                None,
            );
            return;
        }

        let _ = tx.send(48.0).await;

        let install_result = {
            let minecraft_path_str = minecraft_path_clone.to_string_lossy().to_string();
            let _ = tx.send(50.0).await;
            let of_ver = optifine_ver.clone();
            let of_fallback = optifine_fallback_url.clone();
            let v = version.clone();
            let install_task = tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    if of_ver.starts_with("http://") || of_ver.starts_with("https://") {
                        let fallback = of_fallback.unwrap_or_else(|| of_ver.clone());
                        optifine_installer::install_optifine_from_bmcl_with_fallback(
                            &v,
                            &of_ver,
                            &fallback,
                            std::path::Path::new(&minecraft_path_str),
                        )
                        .await
                    } else {
                        optifine_installer::install_optifine_from_local(
                            std::path::Path::new(&of_ver),
                            std::path::Path::new(&minecraft_path_str),
                            &v,
                        )
                        .await
                    }
                })
            });
            match install_task.await {
                Ok(Ok(vf)) => Ok(vf),
                Ok(Err(e)) => Err(format!("安装 OptiFine 失败: {}", e)),
                Err(e) => Err(format!("安装任务异常: {}", e)),
            }
        };

        {
            let mut tasks = optifine_active_tasks().lock().unwrap();
            tasks.remove(&task_id);
        }

        let was_cancelled = cancel_clone.load(Ordering::SeqCst);
        if was_cancelled {
            emit_finish(
                &app_finish,
                task_id,
                false,
                Some("已取消".to_string()),
                None,
            );
            return;
        }

        match install_result {
            Ok(version_folder) => {
                let _ = tx.send(100.0).await;
                println!("[OptiFine] 安装成功，版本目录: {}", version_folder);

                if let Some(inst_name) = instance_name_cloned {
                    let clean_name = sanitize_instance_name(&inst_name);
                    let default_name = format!("{}-optifine-{}", version, optifine_ver);
                    let final_name = if clean_name.trim().is_empty() {
                        sanitize_instance_name(&default_name)
                    } else {
                        clean_name
                    };
                    match merge_version_jsons_to_instance(
                        &final_name,
                        &version,
                        &version_folder,
                        "optifine",
                        &minecraft_path_clone,
                    ) {
                        Ok(_) => println!("[OptiFine] 实例 JSON 合并完成: {}", final_name),
                        Err(e) => println!("[OptiFine] 警告: 合并实例 JSON 失败: {}", e),
                    }
                }

                emit_finish(&app_finish, task_id, true, None, Some(version_folder));
            }
            Err(e) => {
                println!("[OptiFine] 安装失败: {}", e);
                emit_finish(&app_finish, task_id, false, Some(e), None);
            }
        }
    });

    Ok(task_id)
}

fn emit_finish(
    app: &AppHandle,
    task_id: u64,
    success: bool,
    error: Option<String>,
    version_folder: Option<String>,
) {
    let _ = app.emit(
        "optifine-download-finished",
        OptifineDownloadFinishedPayload {
            task_id,
            success,
            error,
            version_folder,
        },
    );
}

#[tauri::command]
pub async fn cancel_optifine_download(taskId: u64) -> Result<(), String> {
    let tasks = optifine_active_tasks()
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some(info) = tasks.get(&taskId) {
        info.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}