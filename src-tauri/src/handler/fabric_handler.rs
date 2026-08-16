use crate::downloader::dwPatch::get_minecraft_dir;
use crate::downloader::fabric_installer;
use crate::downloader::shared_utils::{
    copy_version_mods_to_instance, merge_version_jsons_to_instance, sanitize_instance_name,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize)]
pub struct FabricInstallResult {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FabricVersion {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Serialize)]
struct FabricDownloadProgressPayload {
    task_id: u64,
    percent: f64,
}

#[derive(Clone, Serialize)]
struct FabricDownloadFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
}

static FABRIC_TASK_COUNTER: AtomicU64 = AtomicU64::new(2000000);

struct FabricActiveTaskInfo {
    cancel: Arc<AtomicBool>,
}

fn fabric_active_tasks() -> &'static Mutex<HashMap<u64, FabricActiveTaskInfo>> {
    static INSTANCE: OnceLock<Mutex<HashMap<u64, FabricActiveTaskInfo>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取指定Minecraft版本的Fabric Loader版本列表
#[tauri::command]
pub async fn get_fabric_loader_versions(
    mc_version: String,
    use_mirror: bool,
) -> Result<Vec<FabricVersion>, String> {
    let version_names = fabric_installer::get_fabric_loader_versions(&mc_version, use_mirror)
        .await
        .map_err(|e| format!("获取 Fabric Loader 版本失败: {}", e))?;
    Ok(version_names
        .into_iter()
        .map(|v| FabricVersion {
            id: v.clone(),
            version: v,
        })
        .collect())
}

/// 获取指定Minecraft版本的Fabric API版本列表
#[tauri::command]
pub async fn get_fabric_api_versions(mc_version: String) -> Result<Vec<FabricVersion>, String> {
    let version_names = fabric_installer::get_fabric_api_versions(&mc_version)
        .await
        .map_err(|e| format!("获取 Fabric API 版本失败: {}", e))?;
    Ok(version_names
        .into_iter()
        .map(|v| FabricVersion {
            id: v.clone(),
            version: v,
        })
        .collect())
}

/// 安装Fabric Loader
#[tauri::command]
pub async fn install_fabric_loader(
    mc_version: String,
    loader_version: String,
    use_mirror: bool,
    _window: tauri::Window,
) -> Result<FabricInstallResult, String> {
    let minecraft_path = get_minecraft_dir()?;

    fabric_installer::install_fabric_loader(
        &mc_version,
        &loader_version,
        &minecraft_path.to_string_lossy().to_string(),
        use_mirror,
    )
    .await
    .map_err(|e| format!("安装 Fabric Loader 失败: {}", e))?;

    Ok(FabricInstallResult {
        message: format!(
            "Fabric Loader {} 已成功安装到 Minecraft {}",
            loader_version, mc_version
        ),
    })
}

/// 安装Fabric API
#[tauri::command]
pub async fn install_fabric_api(
    mc_version: String,
    api_version: String,
    _window: tauri::Window,
) -> Result<FabricInstallResult, String> {
    let minecraft_path = get_minecraft_dir()?;

    fabric_installer::install_fabric_api(
        &mc_version,
        &api_version,
        &minecraft_path.to_string_lossy().to_string(),
    )
    .await
    .map_err(|e| format!("安装 Fabric API 失败: {}", e))?;

    Ok(FabricInstallResult {
        message: format!(
            "Fabric API {} 已成功安装到 Minecraft {}",
            api_version, mc_version
        ),
    })
}

/// 下载并安装指定版本的Fabric（带进度显示）
#[tauri::command]
pub async fn download_and_install_fabric(
    app: AppHandle,
    mc_version: String,
    loader_version: String,
    api_version: Option<String>,
    instance_name: Option<String>,
) -> Result<u64, String> {
    let task_id = FABRIC_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let minecraft_path = get_minecraft_dir()?;
    std::fs::create_dir_all(&minecraft_path).map_err(|e| format!("创建目录失败: {}", e))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);
    let cancel = Arc::new(AtomicBool::new(false));

    // 注册活跃任务
    {
        let mut tasks = fabric_active_tasks().lock().unwrap();
        tasks.insert(task_id, FabricActiveTaskInfo {
            cancel: cancel.clone(),
        });
    }

    // 接收进度并通过 Tauri 事件发送到前端
    let app_clone = app.clone();
    let task_id_clone = task_id;
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit(
                "fabric-download-progress",
                FabricDownloadProgressPayload {
                    task_id: task_id_clone,
                    percent,
                },
            );
        }
    });

    // 立即发送初始进度事件
    let _ = app.emit(
        "fabric-download-progress",
        FabricDownloadProgressPayload {
            task_id,
            percent: 0.0,
        },
    );

    let app_finish = app.clone();
    let version = mc_version.clone();
    let loader_ver = loader_version.clone();
    let api_ver = api_version.clone();
    let cancel_clone = cancel.clone();
    let minecraft_path_clone = minecraft_path.clone();
    let instance_name_cloned = instance_name.clone();

    tokio::spawn(async move {
        // ============= 并行下载：原版 Minecraft + Fabric 安装 =============

        // --- Task A: 原版 Minecraft 下载 (0-60%) ---
        let tx_for_original = tx.clone();
        let cancel_clone_a = cancel_clone.clone();
        let version_a = version.clone();
        let mc_path_a = minecraft_path_clone.clone();
        let original_handle = tokio::spawn(async move {
            let (tx1, mut rx1) = tokio::sync::mpsc::channel::<f64>(64);
            tokio::spawn(async move {
                while let Some(percent) = rx1.recv().await {
                    let _ = tx_for_original.send(percent * 0.6).await;
                }
            });
            crate::downloader::original_dwl::process_version(
                &version_a,
                &mc_path_a,
                tx1,
                cancel_clone_a.clone(),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        });

        // --- Task B: Fabric 安装 (60-100%) ---
        let tx_for_fabric = tx.clone();
        let version_b = version.clone();
        let loader_ver_b = loader_ver.clone();
        let api_ver_b = api_ver.clone();
        let mc_path_b = minecraft_path_clone.clone();
        let fabric_handle = tokio::spawn(async move {
            let _ = tx_for_fabric.send(60.0).await;

            let minecraft_path_str = mc_path_b.to_string_lossy().to_string();

            // 安装 Fabric Loader (60-80%)
            let loader_dir = fabric_installer::install_fabric_loader(
                &version_b,
                &loader_ver_b,
                &minecraft_path_str,
                true,
            )
            .await
            .map_err(|e| format!("Fabric Loader 安装失败: {}", e))?;

            let _ = tx_for_fabric.send(80.0).await;

            // 安装 Fabric API (80-100%)
            if let Some(api_ver) = api_ver_b {
                fabric_installer::install_fabric_api(&version_b, &api_ver, &minecraft_path_str)
                    .await
                    .map_err(|e| format!("Fabric API 安装失败: {}", e))?;
            }

            let _ = tx_for_fabric.send(100.0).await;
            Ok::<String, String>(loader_dir)
        });

        // --- 等待两个 task 完成 ---
        let (original_result, fabric_result) = tokio::join!(original_handle, fabric_handle);
        let original_result = original_result.unwrap_or_else(|e| Err(e.to_string()));
        let fabric_result = fabric_result.unwrap_or_else(|e| Err(e.to_string()));

        // 移除活跃任务
        {
            let mut tasks = fabric_active_tasks().lock().unwrap();
            tasks.remove(&task_id);
        }

        let was_cancelled = cancel_clone.load(Ordering::SeqCst);

        if was_cancelled {
            let _ = app_finish.emit(
                "fabric-download-finished",
                FabricDownloadFinishedPayload {
                    task_id,
                    success: false,
                    error: Some("已取消".to_string()),
                },
            );
            return;
        }

        // 优先报告原版下载错误
        if let Err(e) = original_result {
            let _ = app_finish.emit(
                "fabric-download-finished",
                FabricDownloadFinishedPayload {
                    task_id,
                    success: false,
                    error: Some(format!("原版下载失败: {}", e)),
                },
            );
            return;
        }

        match fabric_result {
            Ok(loader_version_name) => {
                println!("Fabric 安装成功: {}", loader_ver);

                if let Some(inst_name) = instance_name_cloned {
                    let clean_name = sanitize_instance_name(&inst_name);
                    println!("[Fabric] 创建实例目录: {}", clean_name);
                    let final_name = if clean_name.trim().is_empty() {
                        sanitize_instance_name(&loader_version_name)
                    } else {
                        clean_name
                    };
                    match merge_version_jsons_to_instance(
                        &final_name,
                        &version,
                        &loader_version_name,
                        "fabric",
                        &minecraft_path_clone,
                    ) {
                        Ok(_) => {
                            println!("[Fabric] 实例 JSON 合并完成: {}", final_name);
                            // Fabric API 以 mod 形式装在 versions/{mc}/mods，搬运到实例 mods
                            if api_ver.is_some() {
                                copy_version_mods_to_instance(
                                    &final_name,
                                    &version,
                                    &minecraft_path_clone,
                                );
                            }
                        }
                        Err(e) => println!("[Fabric] 警告: 合并实例 JSON 失败: {}", e),
                    }
                }

                let _ = app_finish.emit(
                    "fabric-download-finished",
                    FabricDownloadFinishedPayload {
                        task_id,
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                println!("Fabric 安装失败: {}", e);
                let _ = app_finish.emit(
                    "fabric-download-finished",
                    FabricDownloadFinishedPayload {
                        task_id,
                        success: false,
                        error: Some(e),
                    },
                );
            }
        }
    });

    // 立即返回 task_id，不阻塞前端
    Ok(task_id)
}

/// 取消Fabric下载任务
#[tauri::command]
pub async fn cancel_fabric_download(taskId: u64) -> Result<(), String> {
    let tasks = fabric_active_tasks().lock().map_err(|e| e.to_string())?;
    if let Some(info) = tasks.get(&taskId) {
        info.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}
