//! 组合安装：原版（必选）+ 可选加载器（互斥）+ OptiFine/Fabric API 叠加，
//! 一次安装自动组包成单个完整实例（参考 PCL/HMCL 的多选安装流程）。

use crate::downloader::dwPatch::get_minecraft_dir;
use crate::downloader::mod_loader_installer_shared::pick_java_executable;
use crate::downloader::original_dwl::process_version;
use crate::downloader::shared_utils::{
    cleanup_loader_version_dir, cleanup_vanilla_version_dir, copy_version_mods_to_instance,
    merge_version_jsons_to_instance, sanitize_instance_name,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedSelection {
    /// "forge" | "neoforge" | "fabric" | "quilt" | "liteloader" | "optifine" | "fabric_api"
    pub loader_type: String,
    pub version: String,
}

#[derive(Clone, Serialize)]
struct CombinedProgressPayload {
    task_id: u64,
    percent: f64,
}

#[derive(Clone, Serialize)]
struct CombinedFinishedPayload {
    task_id: u64,
    success: bool,
    error: Option<String>,
}

static COMBINED_TASK_COUNTER: AtomicU64 = AtomicU64::new(9000000);

struct CombinedActiveTaskInfo {
    cancel: Arc<AtomicBool>,
}

fn combined_active_tasks() -> &'static Mutex<HashMap<u64, CombinedActiveTaskInfo>> {
    static INSTANCE: OnceLock<Mutex<HashMap<u64, CombinedActiveTaskInfo>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_mod_loader(loader_type: &str) -> bool {
    matches!(
        loader_type,
        "forge" | "neoforge" | "fabric" | "quilt" | "liteloader"
    )
}

/// 校验选择组合的兼容性（与前端一致）
fn validate_selections(selections: &[CombinedSelection]) -> Result<(), String> {
    let mod_loaders: Vec<&CombinedSelection> = selections
        .iter()
        .filter(|s| is_mod_loader(&s.loader_type))
        .collect();
    if mod_loaders.len() > 1 {
        return Err("最多只能选择一个模组加载器（Forge/NeoForge/Fabric/Quilt/LiteLoader 互不兼容）".to_string());
    }
    let has_fabric = mod_loaders
        .iter()
        .any(|s| s.loader_type == "fabric");
    if selections
        .iter()
        .any(|s| s.loader_type == "fabric_api")
        && !has_fabric
    {
        return Err("Fabric API 需要同时选择 Fabric 加载器".to_string());
    }
    if selections.iter().any(|s| s.loader_type == "optifine") {
        let conflict = mod_loaders
            .iter()
            .any(|s| matches!(s.loader_type.as_str(), "fabric" | "quilt" | "liteloader"));
        if conflict {
            return Err("OptiFine 不能与 Fabric/Quilt/LiteLoader 组合".to_string());
        }
    }
    Ok(())
}

fn loader_type_hint(loader_type: &str) -> &'static str {
    match loader_type {
        "forge" => "forge",
        "neoforge" => "neoforge",
        "fabric" => "fabric",
        "quilt" => "quilt",
        "liteloader" => "liteloader",
        "optifine" => "optifine",
        _ => "vanilla",
    }
}

#[tauri::command]
pub async fn install_combined_package(
    app: AppHandle,
    mc_version: String,
    selections: Vec<CombinedSelection>,
    instance_name: Option<String>,
) -> Result<u64, String> {
    validate_selections(&selections)?;
    if selections.is_empty() {
        println!("[Combined] 未选择加载器，仅安装原版 {}", mc_version);
    }

    let task_id = COMBINED_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let minecraft_path = get_minecraft_dir()?;
    std::fs::create_dir_all(&minecraft_path).map_err(|e| format!("创建目录失败: {}", e))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut tasks = combined_active_tasks().lock().unwrap();
        tasks.insert(
            task_id,
            CombinedActiveTaskInfo {
                cancel: cancel.clone(),
            },
        );
    }

    let app_clone = app.clone();
    let task_id_clone = task_id;
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            let _ = app_clone.emit(
                "combined-download-progress",
                CombinedProgressPayload {
                    task_id: task_id_clone,
                    percent,
                },
            );
        }
    });
    let _ = app.emit(
        "combined-download-progress",
        CombinedProgressPayload {
            task_id,
            percent: 0.0,
        },
    );

    let app_finish = app.clone();
    let version = mc_version.clone();
    let selections_cloned = selections.clone();
    let cancel_clone = cancel.clone();
    let minecraft_path_clone = minecraft_path.clone();
    let instance_name_cloned = instance_name.clone();

    tokio::spawn(async move {
        let result = run_combined_install(
            &app_finish,
            task_id,
            &version,
            &selections_cloned,
            instance_name_cloned.as_deref(),
            &minecraft_path_clone,
            tx.clone(),
            cancel_clone.clone(),
        )
        .await;

        {
            let mut tasks = combined_active_tasks().lock().unwrap();
            tasks.remove(&task_id);
        }

        if cancel_clone.load(Ordering::SeqCst) {
            let _ = app_finish.emit(
                "combined-download-finished",
                CombinedFinishedPayload {
                    task_id,
                    success: false,
                    error: Some("已取消".to_string()),
                },
            );
            return;
        }

        match result {
            Ok(()) => {
                let _ = tx.send(100.0).await;
                let _ = app_finish.emit(
                    "combined-download-finished",
                    CombinedFinishedPayload {
                        task_id,
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                let _ = app_finish.emit(
                    "combined-download-finished",
                    CombinedFinishedPayload {
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

async fn run_combined_install(
    _app: &AppHandle,
    task_id: u64,
    mc_version: &str,
    selections: &[CombinedSelection],
    instance_name: Option<&str>,
    minecraft_path: &std::path::Path,
    tx: tokio::sync::mpsc::Sender<f64>,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let send_progress = |pct: f64| {
        let _ = tx.try_send(pct);
    };

    // ===== 1. 原版（必选）：已存在则跳过 =====
    let vanilla_json = minecraft_path
        .join("versions")
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    let vanilla_jar = minecraft_path
        .join("versions")
        .join(mc_version)
        .join(format!("{}.jar", mc_version));
    send_progress(1.0);
    if vanilla_json.exists() && vanilla_jar.exists() {
        println!("[Combined] 原版 {} 已存在，跳过下载", mc_version);
    } else {
        let (tx1, mut rx1) = tokio::sync::mpsc::channel::<f64>(64);
        let tx_main = tx.clone();
        tokio::spawn(async move {
            while let Some(percent) = rx1.recv().await {
                let _ = tx_main.send(1.0 + percent * 0.39).await;
            }
        });
        process_version(mc_version, minecraft_path, tx1, cancel.clone())
            .await
            .map_err(|e| format!("原版下载失败: {}", e))?;
    }
    send_progress(42.0);

    // ===== 2. 模组加载器（五选一）=====
    let mod_loader = selections
        .iter()
        .find(|s| is_mod_loader(&s.loader_type));
    let mut loader_version_name: Option<String> = None;
    if let Some(sel) = mod_loader {
        if cancel.load(Ordering::SeqCst) {
            return Err("已取消".to_string());
        }
        let mc_path_str = minecraft_path.to_string_lossy().to_string();
        let java = pick_java_executable(mc_version);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel::<f64>(64);
        let tx_main = tx.clone();
        tokio::spawn(async move {
            while let Some(percent) = rx2.recv().await {
                let _ = tx_main.send(42.0 + percent * 0.33).await;
            }
        });
        let installed = match sel.loader_type.as_str() {
            "forge" => {
                crate::downloader::forge_installer::install_forge(
                    mc_version,
                    &sel.version,
                    &mc_path_str,
                    &java,
                    Some(tx2),
                    None,
                )
                .await
            }
            "neoforge" => {
                crate::downloader::neoforge_installer::install_neoforge(
                    mc_version,
                    &sel.version,
                    &mc_path_str,
                    &java,
                    Some(tx2),
                    None,
                )
                .await
            }
            "fabric" => {
                crate::downloader::fabric_installer::install_fabric_loader(
                    mc_version,
                    &sel.version,
                    &mc_path_str,
                    true,
                )
                .await
            }
            "quilt" => {
                crate::downloader::quilt_installer::install_quilt_loader(
                    mc_version,
                    &sel.version,
                    &mc_path_str,
                    Some(tx2),
                )
                .await
            }
            "liteloader" => {
                crate::downloader::liteloader_installer::install_liteloader(
                    mc_version,
                    &sel.version,
                    &mc_path_str,
                    &java,
                    Some(tx2),
                )
                .await
            }
            other => Err(anyhow::anyhow!("不支持的加载器类型: {}", other)),
        }
        .map_err(|e| format!("{} 安装失败: {}", sel.loader_type, e))?;
        println!("[Combined] 加载器安装完成: {} -> {}", sel.loader_type, installed);
        loader_version_name = Some(installed);
    }

    // ===== 3. Fabric API（mod 形式，落在 versions/{mc}/mods）=====
    if let Some(sel) = selections.iter().find(|s| s.loader_type == "fabric_api") {
        if cancel.load(Ordering::SeqCst) {
            return Err("已取消".to_string());
        }
        let mc_path_str = minecraft_path.to_string_lossy().to_string();
        crate::downloader::fabric_installer::install_fabric_api(
            mc_version,
            &sel.version,
            &mc_path_str,
        )
        .await
        .map_err(|e| format!("Fabric API 安装失败: {}", e))?;
        send_progress(77.0);
    }

    // ===== 4. OptiFine（自动检测叠加到已装 Forge/NeoForge）=====
    let mut optifine_version_name: Option<String> = None;
    if let Some(sel) = selections.iter().find(|s| s.loader_type == "optifine") {
        if cancel.load(Ordering::SeqCst) {
            return Err("已取消".to_string());
        }
        send_progress(78.0);
        let url = sel.version.clone();
        let fallback = url.clone();
        let installed = crate::downloader::optifine_installer::install_optifine_from_bmcl_with_fallback(
            mc_version,
            &url,
            &fallback,
            minecraft_path,
        )
        .await
        .map_err(|e| format!("OptiFine 安装失败: {}", e))?;
        println!("[Combined] OptiFine 安装完成: {}", installed);
        optifine_version_name = Some(installed);
        send_progress(88.0);
    }

    // ===== 5. 组包：合并为一个完整实例 =====
    let loader_target = optifine_version_name.as_ref().or(loader_version_name.as_ref());
    let final_name = if let Some(inst) = instance_name {
        let clean = sanitize_instance_name(inst);
        if clean.trim().is_empty() {
            loader_target
                .map(|s| sanitize_instance_name(s))
                .unwrap_or_else(|| sanitize_instance_name(mc_version))
        } else {
            clean
        }
    } else {
        loader_target
            .map(|s| sanitize_instance_name(s))
            .unwrap_or_else(|| sanitize_instance_name(mc_version))
    };

    if let Some(loader_ver) = loader_target {
        let hint = if optifine_version_name.is_some() {
            "optifine"
        } else {
            mod_loader
                .map(|s| loader_type_hint(&s.loader_type))
                .unwrap_or("vanilla")
        };
        merge_version_jsons_to_instance(
            &final_name,
            mc_version,
            loader_ver,
            hint,
            minecraft_path,
        )
        .map_err(|e| format!("合并实例 JSON 失败: {}", e))?;
        println!("[Combined] 实例组包完成: {}", final_name);
    } else {
        // 只装原版：创建原版实例（自包含复制），随后清理原版目录，只保留单个实例
        crate::downloader::shared_utils::create_vanilla_instance(
            &final_name,
            mc_version,
            minecraft_path,
        )
        .map_err(|e| format!("创建原版实例失败: {}", e))?;
        if final_name != mc_version {
            cleanup_vanilla_version_dir(mc_version, minecraft_path);
        }
    }
    send_progress(93.0);

    // ===== 6. 搬运 mod 文件（Fabric API / 叠加的 OptiFine）=====
    if selections.iter().any(|s| s.loader_type == "fabric_api") {
        copy_version_mods_to_instance(&final_name, mc_version, minecraft_path);
        // fabric-api jar 是我们下载到原版 mods 目录的临时产物，复制到实例后删掉，
        // 让原版目录保持干净，便于组包后整体清理
        if final_name != mc_version {
            let mc_mods = minecraft_path.join("versions").join(mc_version).join("mods");
            let inst_mods = minecraft_path.join("versions").join(&final_name).join("mods");
            if let Ok(entries) = fs::read_dir(&mc_mods) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    if !src.is_file() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("fabric-api-") && name.ends_with(".jar") {
                        let dst = inst_mods.join(&name);
                        if dst.exists() {
                            let _ = fs::remove_file(&src);
                        }
                    }
                }
            }
        }
    }
    if optifine_version_name.is_some() {
        if let Some(loader_ver) = loader_version_name.as_ref() {
            copy_version_mods_to_instance(&final_name, loader_ver, minecraft_path);
        }
    }

    // ===== 7. 清理中间 loader/OptiFine/原版版本目录 =====
    if let Some(of_ver) = optifine_version_name.as_ref() {
        if of_ver != &final_name {
            cleanup_loader_version_dir(of_ver, minecraft_path);
        }
    }
    if let Some(loader_ver) = loader_version_name.as_ref() {
        if loader_ver != &final_name {
            cleanup_loader_version_dir(loader_ver, minecraft_path);
        }
    }
    // 与 HMCL 一致：原版已完整并入合并实例（无 inheritsFrom、库全量合并、jar 已复制），
    // 删除原版目录，版本列表只保留合并后的单个实例
    if final_name != mc_version {
        cleanup_vanilla_version_dir(mc_version, minecraft_path);
    }

    send_progress(98.0);
    println!("[Combined] 安装完成: {} (任务 {})", final_name, task_id);
    Ok(())
}

#[tauri::command]
pub async fn cancel_combined_download(taskId: u64) -> Result<(), String> {
    let tasks = combined_active_tasks().lock().map_err(|e| e.to_string())?;
    if let Some(info) = tasks.get(&taskId) {
        info.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}