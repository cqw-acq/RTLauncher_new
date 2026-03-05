pub mod resource_checker;

use serde::Serialize;
use std::path::Path;

// ── 返回结构体 ───────────────────────────────────────────────

/// 单个实例的元数据
#[derive(Debug, Serialize)]
pub struct InstanceData {
    /// 实例名称（即 instance 目录下的文件夹名，当前约定同时也是 MC 版本号）
    pub name: String,
    /// Minecraft 版本号
    pub minecraft_version: String,
    /// 加载器类型（从 versions/<name>/<name>.json 的 mainClass 推断）
    pub loader: String,
    /// mods/ 目录中的文件数量
    pub mods_count: usize,
}

/// 单个材质包/光影包的信息
#[derive(Debug, Serialize)]
pub struct ResourcePackInfo {
    /// 文件夹名
    pub name: String,
    /// pack.png 的绝对路径（若不存在则为空字符串）
    pub icon_path: String,
    /// 基于 pack_format 的 MC 版本范围描述
    pub mc_version_range: String,
}

/// level.dat 解析结果
#[derive(Debug, Serialize)]
pub struct LevelDatInfo {
    pub seed: String,
    pub keep_inventory: bool,
    pub mob_griefing: bool,
    pub do_fire_tick: bool,
    pub allow_commands: bool,
}

// ── 辅助工具 ─────────────────────────────────────────────────

/// 根据 mainClass 推断加载器类型
fn detect_loader_from_main_class(main_class: &str) -> &'static str {
    let mc = main_class.to_lowercase();
    if mc.contains("fabricmc") || mc.contains("knot") {
        "Fabric"
    } else if mc.contains("quiltmc") || mc.contains("quilt") {
        "Quilt"
    } else if mc.contains("neoforged") {
        "NeoForge"
    } else if mc.contains("bootstraplauncher") || mc.contains("modlauncher") || mc.contains("minecraftforge") {
        "Forge"
    } else if mc.contains("liteloader") {
        "LiteLoader"
    } else {
        "Vanilla"
    }
}

/// 从版本文件夹名中快速推断加载器（备用方案）
fn detect_loader_from_name(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("neoforge") {
        "NeoForge"
    } else if lower.contains("fabric") {
        "Fabric"
    } else if lower.contains("quilt") {
        "Quilt"
    } else if lower.contains("forge") {
        "Forge"
    } else if lower.contains("liteloader") {
        "LiteLoader"
    } else {
        "Vanilla"
    }
}

/// 扫描单个实例目录，构建 InstanceData
fn build_instance_data(instance_dir: &Path, minecraft_path: &Path) -> Option<InstanceData> {
    let name = instance_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // 计算 mods 数量
    let mods_dir = instance_dir.join("mods");
    let mods_count = if mods_dir.is_dir() {
        std::fs::read_dir(&mods_dir)
            .map(|entries| entries.flatten().filter(|e| e.path().is_file()).count())
            .unwrap_or(0)
    } else {
        0
    };

    // 尝试从 versions/<name>/<name>.json 推断加载器与真实 MC 版本
    let version_json_path = minecraft_path.join("versions").join(&name).join(format!("{}.json", name));
    let (minecraft_version, loader) = if version_json_path.is_file() {
        match std::fs::read_to_string(&version_json_path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        // 从 inheritsFrom 字段获取真实 MC 版本（Fabric/Forge 等情况）
                        let mc_ver = json
                            .get("inheritsFrom")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&name)
                            .to_string();
                        // 从 mainClass 推断加载器
                        let loader = json
                            .get("mainClass")
                            .and_then(|v| v.as_str())
                            .map(detect_loader_from_main_class)
                            .unwrap_or_else(|| detect_loader_from_name(&name))
                            .to_string();
                        (mc_ver, loader)
                    }
                    Err(_) => (name.clone(), detect_loader_from_name(&name).to_string()),
                }
            }
            Err(_) => (name.clone(), detect_loader_from_name(&name).to_string()),
        }
    } else {
        (name.clone(), detect_loader_from_name(&name).to_string())
    };

    Some(InstanceData {
        name,
        minecraft_version,
        loader,
        mods_count,
    })
}

/// 目录条目信息
#[derive(Debug, Serialize)]
pub struct DirEntry {
    /// 文件或目录名（不含父路径）
    pub name: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件扩展名（小写，不含点；目录为空字符串）
    pub extension: String,
    /// 文件大小（字节），目录为 0
    pub size: u64,
}

// ── Tauri 命令 ────────────────────────────────────────────────

/// 扫描 instances 目录，返回所有实例的结构化信息
///
/// - `instances_path`: instances 目录的绝对路径（如 `<minecraft_path>/instance`）
#[tauri::command]
pub async fn vm_scan_instances(instances_path: String) -> Result<Vec<InstanceData>, String> {
    let path = Path::new(&instances_path);

    // minecraft_path 是 instances_path 的父目录（用于查找 versions/）
    let minecraft_path = path
        .parent()
        .ok_or_else(|| format!("无法获取父目录: {}", instances_path))?;

    if !path.exists() || !path.is_dir() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("读取实例目录失败: {}", e))?;

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(data) = build_instance_data(&p, minecraft_path) {
                result.push(data);
            }
        }
    }

    Ok(result)
}

/// 扫描指定根目录下的 resourcepacks/ 文件夹，返回所有材质包信息
///
/// - `root_path`: 包含 `resourcepacks/` 子目录的根路径
#[tauri::command]
pub async fn vm_find_resource_packs(root_path: String) -> Result<Vec<ResourcePackInfo>, String> {
    let packs = resource_checker::find_resource_packs(&root_path);
    let result = packs
        .into_iter()
        .map(|(name, icon_path, mc_version_range)| ResourcePackInfo {
            name,
            icon_path,
            mc_version_range,
        })
        .collect();
    Ok(result)
}

/// 解析世界目录下的 level.dat，返回种子和游戏规则
///
/// - `world_folder_path`: 包含 `level.dat` 的世界文件夹绝对路径
#[tauri::command]
pub async fn vm_parse_level_dat(world_folder_path: String) -> Result<LevelDatInfo, String> {
    let raw = resource_checker::parse_level_dat(&world_folder_path)
        .ok_or_else(|| format!("无法解析 level.dat: {}", world_folder_path))?;

    // 原始格式：[seed, keepInventory, mobGriefing, doFireTick, allowCommands]
    let parse_bool = |s: &str| s.to_lowercase() == "true";

    Ok(LevelDatInfo {
        seed: raw.get(0).cloned().unwrap_or_default(),
        keep_inventory: raw.get(1).map(|s| parse_bool(s)).unwrap_or(false),
        mob_griefing: raw.get(2).map(|s| parse_bool(s)).unwrap_or(false),
        do_fire_tick: raw.get(3).map(|s| parse_bool(s)).unwrap_or(false),
        allow_commands: raw.get(4).map(|s| parse_bool(s)).unwrap_or(false),
    })
}

/// 修改世界目录中 level.dat 的游戏规则
///
/// - `world_folder_path`: 包含 `level.dat` 的世界文件夹绝对路径
/// - `param_name`: 规则名（keepInventory / mobGriefing / doFireTick / allowCommands）
/// - `new_value`: 新值（字符串，如 `"true"` / `"false"`）
#[tauri::command]
pub async fn vm_modify_game_rule(
    world_folder_path: String,
    param_name: String,
    new_value: String,
) -> Result<(), String> {
    resource_checker::modify_nbt_param_str(&world_folder_path, &param_name, &new_value)
}

/// 列出指定目录的直接子条目（一层，不递归）
///
/// - `dir_path`: 要列出的目录绝对路径
/// - `extensions_filter`: 允许通过的文件扩展名列表（小写，不含点），为空时返回所有文件和目录
#[tauri::command]
pub async fn vm_list_dir(
    dir_path: String,
    extensions_filter: Vec<String>,
) -> Result<Vec<DirEntry>, String> {
    let path = Path::new(&dir_path);
    if !path.exists() || !path.is_dir() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("读取目录失败: {}", e))?;

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') {
            continue; // 跳过隐藏文件
        }
        let is_dir = p.is_dir();
        let extension = if is_dir {
            String::new()
        } else {
            p.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };
        // 过滤扩展名
        if !extensions_filter.is_empty() && !is_dir {
            if !extensions_filter.iter().any(|f| f == &extension) {
                continue;
            }
        }
        let size = if is_dir {
            0
        } else {
            std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0)
        };
        result.push(DirEntry {
            name,
            is_dir,
            extension,
            size,
        });
    }

    // 目录在前，文件在后；同类按名称排序
    result.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(result)
}
