pub mod resource_checker;

use base64::Engine;
use serde::Serialize;
use std::path::Path;

/// Minecraft 实例中需要存在的标准子目录
///
/// 每次打开文件检索页面对应实例时都会检查这些目录是否存在，不存在则创建。
const STANDARD_INSTANCE_SUBDIRS: &[&str] = &[
    "mods",
    "resourcepacks",
    "shaderpacks",
    "saves",
    "datapacks",
    "config",
];

/// 确保指定实例目录下存在所有 Minecraft 标准子目录
///
/// 调用频率：在文件检索页面点开对应实例时检查一次即可。
pub fn ensure_instance_dirs(instance_dir: &Path) -> Result<(), String> {
    for sub in STANDARD_INSTANCE_SUBDIRS {
        let sub_path = instance_dir.join(sub);
        if !sub_path.exists() {
            std::fs::create_dir_all(&sub_path)
                .map_err(|e| format!("创建目录 {} 失败: {}", sub_path.display(), e))?;
        }
    }
    Ok(())
}

/// 前端调用命令：确保指定实例目录具备完整的标准子目录结构
#[tauri::command]
pub async fn vm_ensure_instance_dirs(instance_dir: String) -> Result<(), String> {
    let path = Path::new(&instance_dir);
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("创建实例目录 {} 失败: {}", path.display(), e))?;
    }
    ensure_instance_dirs(path)
}

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
    if mc.contains("optifine") {
        "OptiFine"
    } else if mc.contains("quiltmc") || mc.contains("quilt") {
        "Quilt"
    } else if mc.contains("fabricmc") || mc.contains("knot") {
        "Fabric"
    } else if mc.contains("neoforged") {
        "NeoForge"
    } else if mc.contains("bootstraplauncher")
        || mc.contains("modlauncher")
        || mc.contains("minecraftforge")
    {
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
    let starts_with_mc_version = regex::Regex::new(r"^\d+\.\d+(?:\.\d+)?[-_]")
        .map(|re| re.is_match(&lower))
        .unwrap_or(false);
    // 使用更精确的匹配模式，避免误报
    // 例如： "-fabric-", "_fabric_", "-fabric", "fabric-", "fabric_" 等模式
    if lower.contains("optifine") {
        "OptiFine"
    } else if lower.contains("neoforge") {
        "NeoForge"
    } else if (starts_with_mc_version && (lower.contains("-fabric-") || lower.contains("_fabric_"))) ||
              lower.ends_with("-fabric") || lower.ends_with("_fabric") ||
              lower.starts_with("fabric-loader-") || lower.starts_with("fabric_loader_") ||
              lower.contains("-fabricloader-") || lower.contains("_fabricloader_") {
        "Fabric"
    } else if lower.contains("-quilt-") || lower.contains("_quilt_") ||
              lower.ends_with("-quilt") || lower.ends_with("_quilt") ||
              lower.starts_with("quilt-") || lower.starts_with("quilt_") ||
              lower.contains("-quiltloader-") || lower.contains("_quiltloader_") {
        "Quilt"
    } else if lower.contains("-forge-") || lower.contains("_forge_") ||
              lower.ends_with("-forge") || lower.ends_with("_forge") ||
              lower.starts_with("forge-") || lower.starts_with("forge_") {
        "Forge"
    } else if lower.contains("liteloader") {
        "LiteLoader"
    } else {
        "Vanilla"
    }
}

/// 从版本文件夹名称中提取原始 Minecraft 版本号
///
/// 支持的常见格式：
/// - "1.21.1" -> "1.21.1"
/// - "1.21.1-neoforge-4.0.1.20" -> "1.21.1"
/// - "1.21.1-forge-52.0.0" -> "1.21.1"
/// - "fabric-loader-0.15.0-1.21.1" -> "1.21.1"
/// - "quilt-loader-0.25.0-1.21.1" -> "1.21.1"
/// - "1.21.1-OptiFine_HD_U_I7_pre1" -> "1.21.1"
/// - "26.3-snapshot-5" -> "26.3"
/// - "25w42a" -> "25w42a"
fn extract_minecraft_version(name: &str) -> String {
    // 模式 1：快照版本格式，如 25w42a, 24w12a
    let snapshot_re = regex::Regex::new(r"^\d{2}w\d{2}[a-z]$").unwrap();
    if snapshot_re.is_match(name) {
        return name.to_string();
    }
    
    // 模式 2：以数字开头，后面跟 . 和数字，即 "x.y.z" 或 "x.y" 格式
    // 匹配 "1.21.1"、"1.21" 等开头的部分
    let standard_re = regex::Regex::new(r"^(\d+\.\d+(?:\.\d+)?)").unwrap();
    if let Some(caps) = standard_re.captures(name) {
        return caps.get(1).unwrap().as_str().to_string();
    }
    
    // 模式 3：版本号在字符串中间。一个名称可能同时包含加载器版本和
    // Minecraft 版本（如 fabric-loader-0.15.0-1.21.1），优先选 1.x，
    // 否则选日历版本中最后一个主版本 >= 20 的候选。
    let middle_re = regex::Regex::new(r"\d+\.\d+(?:\.\d+)?").unwrap();
    let candidates: Vec<&str> = middle_re.find_iter(name).map(|m| m.as_str()).collect();
    if let Some(version) = candidates.iter().find(|v| v.starts_with("1.")) {
        return (*version).to_string();
    }
    if let Some(version) = candidates.iter().rev().find(|v| {
        v.split('.').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0) >= 20
    }) {
        return (*version).to_string();
    }
    
    // 模式 4：处理类似 "26.3-snapshot-5" 的格式
    let snapshot_ver_re = regex::Regex::new(r"^(\d+\.\d+)-snapshot").unwrap();
    if let Some(caps) = snapshot_ver_re.captures(name) {
        return caps.get(1).unwrap().as_str().to_string();
    }
    
    // 模式 5：处理单个数字版本，如 "26" (用于新的快照格式)
    let single_ver_re = regex::Regex::new(r"^(\d+)(?:[-_.]|$)").unwrap();
    if let Some(caps) = single_ver_re.captures(name) {
        let ver = caps.get(1).unwrap().as_str();
        // 只有当数字大于等于 20 时才认为是版本号（避免误判其他数字）
        if let Ok(num) = ver.parse::<u32>() {
            if num >= 20 {
                return ver.to_string();
            }
        }
    }
    
    // fallback：原样返回
    name.to_string()
}

/// 判断目录名本身是否是可识别的 Minecraft 版本标识。
///
/// assetIndex.id 是资源索引代号（例如 32），不是 Minecraft 版本；只有目录名
/// 确实像正式版或周快照时，才把它作为分组版本使用。
fn version_from_instance_name(name: &str) -> Option<String> {
    let release = regex::Regex::new(r"^\d+\.\d+(?:\.\d+)?(?:-.+)?$").ok()?;
    let weekly_snapshot = regex::Regex::new(r"^\d{2}w\d{2}[a-z]$").ok()?;
    // 新的快照格式，如 26, 26.3, 26.3-snapshot-5
    let new_snapshot = regex::Regex::new(r"^\d+(\.\d+)?(?:-snapshot)?$").ok()?;

    if release.is_match(name) {
        Some(extract_minecraft_version(name))
    } else if weekly_snapshot.is_match(name) {
        Some(name.to_string())
    } else if new_snapshot.is_match(name) {
        // 检查是否为合理的版本号（主版本号 >= 20）
        let extracted = extract_minecraft_version(name);
        if let Ok(num) = extracted.parse::<f32>() {
            if num >= 20.0 {
                Some(extracted)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 合并型整合包实例没有 inheritsFrom，目录名也不一定带加载器名称。
/// 优先从加载器库坐标中读取真实 Minecraft 版本。
fn detect_minecraft_version_from_libraries(json: &serde_json::Value) -> Option<String> {
    let libraries = json.get("libraries")?.as_array()?;
    for library in libraries {
        let name = library.get("name")?.as_str()?;
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() < 3 {
            continue;
        }

        let group = parts[0];
        let artifact = parts[1];
        let version = parts[2];
        let carries_mc_version = (group == "net.minecraftforge"
            && (artifact == "forge" || artifact == "fmlloader"))
            || (group == "net.fabricmc" && artifact == "intermediary");
        if carries_mc_version {
            let detected = extract_minecraft_version(version);
            // 合并型整合包里 fabric 的 intermediary 会被改写成占位符 "0.0.0"，
            // 它不含真实版本信息，直接跳过，留给后续的 patches 检测。
            if detected != "0.0.0" && (detected != version || version.contains('.')) {
                return Some(detected);
            }
        }
    }
    None
}

/// 从合并型整合包 version.json 的 `patches` 字段读取真实 Minecraft 版本。
///
/// 合并型整合包的 JSON 会把补丁列表写入 `patches`，例如：
/// ```json
/// { "patches": [ { "id": "game", "version": "26.1.2", "priority": 0 } ] }
/// ```
/// 其中 `id == "game"` 的补丁携带的是真实的 Minecraft 版本号。
fn detect_minecraft_version_from_patches(json: &serde_json::Value) -> Option<String> {
    json.get("patches")?
        .as_array()?
        .iter()
        .find_map(|patch| {
            if patch.get("id").and_then(|v| v.as_str()) != Some("game") {
                return None;
            }
            let ver = patch.get("version").and_then(|v| v.as_str())?;
            let detected = extract_minecraft_version(ver);
            if version_from_instance_name(&detected).is_some() {
                Some(detected)
            } else {
                None
            }
        })
}

/// 扫描单个实例目录，构建 InstanceData
fn build_instance_data(instance_dir: &Path, minecraft_path: &Path) -> Option<InstanceData> {
    let name = instance_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // 跳过 HMCL 特有的目录
    if name == ".hmcl" || name == "hmcl" {
        return None;
    }

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
    let version_json_path = minecraft_path
        .join("versions")
        .join(&name)
        .join(format!("{}.json", name));
    let (minecraft_version, loader) = if version_json_path.is_file() {
        match std::fs::read_to_string(&version_json_path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        // assetIndex.id 是资源索引代号，不是 Minecraft 版本。
                        // 合并型实例优先读补丁列表里的真实版本，其次读加载器库坐标，普通实例再读版本目录名。
                        let raw_ver = json
                            .get("inheritsFrom")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| detect_minecraft_version_from_patches(&json))
                            .or_else(|| detect_minecraft_version_from_libraries(&json))
                            .or_else(|| version_from_instance_name(&name))
                            .or_else(|| {
                                // 从 downloads.client.url 中匹配版本号
                                json.get("downloads")
                                    .and_then(|v| v.get("client"))
                                    .and_then(|v| v.get("url"))
                                    .and_then(|v| v.as_str())
                                    .and_then(|url| {
                                        let re = regex::Regex::new(
                                            r"/(\d+\.\d+(\.\d+)?(-pre\d+)?(-rc\d+)?(-snapshot)?)/",
                                        )
                                        .ok()?;
                                        re.captures(url)
                                            .and_then(|c| c.get(1))
                                            .map(|m| m.as_str().to_string())
                                    })
                            })
                            .unwrap_or(name.clone());
                        // 进一步提取纯版本号（防止 inheritsFrom 也包含加载器信息）
                        let mut mc_ver = extract_minecraft_version(&raw_ver);
                        // 如果提取到的版本看起来不合理（例如 "0.0.0" 或等于原始目录名），
                        // 那么尝试从目录名中再次提取一个更可信的版本号。
                        let is_plausible = version_from_instance_name(&mc_ver).is_some();
                        if !is_plausible || mc_ver == "0.0.0" || mc_ver == name {
                            let alt = extract_minecraft_version(&name);
                            if version_from_instance_name(&alt).is_some() {
                                mc_ver = alt;
                            }
                        }
                        // 从 mainClass 推断加载器，但与文件夹名交叉验证
                        // （NeoForge 的 mainClass 可能与 Forge 相同，需要用文件夹名区分）
                        let from_main = json
                            .get("mainClass")
                            .and_then(|v| v.as_str())
                            .map(detect_loader_from_main_class)
                            .unwrap_or("Vanilla");
                        let from_name = detect_loader_from_name(&name);
                        let loader = if from_main == "Forge" && from_name == "NeoForge" {
                            // mainClass 误判为 Forge，但文件夹名明确是 NeoForge → 以文件夹名为准
                            "NeoForge"
                        } else if from_main == "Forge" && from_name == "OptiFine" {
                            // OptiFine 继承 Forge 的 mainClass，但文件夹名明确是 OptiFine → 以文件夹名为准
                            "OptiFine"
                        } else if from_main == "Vanilla" {
                            // mainClass 无法判断 → 用文件夹名
                            from_name
                        } else {
                            from_main
                        }
                        .to_string();
                        (mc_ver, loader)
                    }
                    Err(_) => (
                        extract_minecraft_version(&name),
                        detect_loader_from_name(&name).to_string(),
                    ),
                }
            }
            Err(_) => (
                extract_minecraft_version(&name),
                detect_loader_from_name(&name).to_string(),
            ),
        }
    } else {
        (
            extract_minecraft_version(&name),
            detect_loader_from_name(&name).to_string(),
        )
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
    /// 文件完整路径
    pub path: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件扩展名（小写，不含点；目录为空字符串）
    pub extension: String,
    /// 文件大小（字节），目录为 0
    pub size: u64,
    /// 修改时间（RFC3339 格式）
    pub modified: String,
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

    let entries = std::fs::read_dir(path).map_err(|e| format!("读取实例目录失败: {}", e))?;

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            // 跳过 HMCL 特有的目录，避免误判
            if let Some(dir_name) = p.file_name().and_then(|n| n.to_str()) {
                // 明确跳过 HMCL 相关目录
                if dir_name == ".hmcl" || dir_name == "hmcl" {
                    continue;
                }
            }
            
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

    let entries = std::fs::read_dir(path).map_err(|e| format!("读取目录失败: {}", e))?;

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
        
        let full_path = p.to_string_lossy().to_string();
        
        let modified = match std::fs::metadata(&p).and_then(|m| m.modified()) {
            Ok(t) => {
                if let Ok(duration) = t.duration_since(std::time::UNIX_EPOCH) {
                    format!("{}", duration.as_secs())
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(),
        };
        
        result.push(DirEntry {
            name,
            path: full_path,
            is_dir,
            extension,
            size,
            modified,
        });
    }

    // 目录在前，文件在后；同类按名称排序
    result.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(result)
}

/// 删除实例目录中的指定文件
///
/// - `dir_path`: 文件所在的目录绝对路径
/// - `file_name`: 要删除的文件名
#[tauri::command]
pub async fn vm_delete_file(dir_path: String, file_name: String) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let file_path = dir.join(&file_name);
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", file_path.display()));
    }
    if file_path.is_dir() {
        std::fs::remove_dir_all(&file_path).map_err(|e| format!("删除目录失败: {}", e))?;
    } else {
        std::fs::remove_file(&file_path).map_err(|e| format!("删除文件失败: {}", e))?;
    }
    Ok(())
}

/// 重命名实例目录中的文件
///
/// - `dir_path`: 文件所在的目录绝对路径
/// - `old_name`: 原文件名
/// - `new_name`: 新文件名
#[tauri::command]
pub async fn vm_rename_file(
    dir_path: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let old_path = dir.join(&old_name);
    let new_path = dir.join(&new_name);
    if !old_path.exists() {
        return Err(format!("源文件不存在: {}", old_path.display()));
    }
    if new_path.exists() {
        return Err(format!("目标文件已存在: {}", new_path.display()));
    }
    std::fs::rename(&old_path, &new_path).map_err(|e| format!("重命名失败: {}", e))?;
    Ok(())
}

/// 将 Base64 编码的内容写入指定目录的文件
///
/// - `dir_path`: 目标目录绝对路径
/// - `file_name`: 要创建的文件名
/// - `content_base64`: Base64 编码的文件内容
#[tauri::command]
pub async fn vm_write_file_base64(
    dir_path: String,
    file_name: String,
    content_base64: String,
) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    
    // 确保目录存在
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
    let file_path = dir.join(&file_name);
    
    // 解码 Base64
    let content = base64::engine::general_purpose::STANDARD
        .decode(&content_base64)
        .map_err(|e| format!("Base64 解码失败: {}", e))?;
    
    // 写入文件
    std::fs::write(&file_path, content).map_err(|e| format!("写入文件失败: {}", e))?;
    
    Ok(())
}

/// 从 cache 中删除文件（彻底删除，不再移入实例）
///
/// - `kind`: 资源类型（mod, resourcepack, world, shaderpack, datapack, schematic, screenshot）
/// - `mc_version`: Minecraft 版本号
/// - `mod_loader`: 仅 kind = mod 时需要
/// - `file_name`: 要删除的文件名
#[tauri::command]
pub async fn vm_delete_cached_file(
    kind: String,
    mc_version: String,
    mod_loader: Option<String>,
    file_name: String,
) -> Result<(), String> {
    use crate::handler::cache_paths::{
        get_cache_dir_for_version, get_mod_cache_dir, parse_mod_loader, parse_resource_kind,
        CacheResourceKind,
    };

    let resource_kind = parse_resource_kind(&kind)?;

    let cache_dir = if resource_kind == CacheResourceKind::Mod {
        let loader_str = mod_loader.as_ref().map(|s| s.as_str()).unwrap_or("forge");
        let loader = parse_mod_loader(loader_str)?;
        get_mod_cache_dir(&mc_version, loader)?
    } else {
        get_cache_dir_for_version(resource_kind, &mc_version)?
    };

    let file_path = cache_dir.join(&file_name);
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", file_path.display()));
    }
    if file_path.is_dir() {
        std::fs::remove_dir_all(&file_path).map_err(|e| format!("删除目录失败: {}", e))?;
    } else {
        std::fs::remove_file(&file_path).map_err(|e| format!("删除文件失败: {}", e))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{
        detect_loader_from_main_class, detect_loader_from_name,
        detect_minecraft_version_from_libraries, detect_minecraft_version_from_patches,
        extract_minecraft_version, version_from_instance_name,
    };
    use serde_json::json;

    #[test]
    fn extracts_version_from_named_modpack_instance() {
        assert_eq!(extract_minecraft_version("1-1.20.1"), "1.20.1");
    }

    #[test]
    fn extracts_version_from_standard_format() {
        assert_eq!(extract_minecraft_version("1.21.1"), "1.21.1");
        assert_eq!(extract_minecraft_version("1.20.4"), "1.20.4");
        assert_eq!(extract_minecraft_version("1.21"), "1.21");
    }

    #[test]
    fn extracts_version_from_loader_format() {
        assert_eq!(extract_minecraft_version("1.21.1-neoforge-4.0.1.20"), "1.21.1");
        assert_eq!(extract_minecraft_version("1.21.1-forge-52.0.0"), "1.21.1");
        assert_eq!(extract_minecraft_version("fabric-loader-0.15.0-1.21.1"), "1.21.1");
        assert_eq!(extract_minecraft_version("quilt-loader-0.25.0-1.21.1"), "1.21.1");
        assert_eq!(extract_minecraft_version("1.21.1-OptiFine_HD_U_I7_pre1"), "1.21.1");
    }

    #[test]
    fn extracts_version_from_snapshot_format() {
        assert_eq!(extract_minecraft_version("25w42a"), "25w42a");
        assert_eq!(extract_minecraft_version("24w12a"), "24w12a");
        assert_eq!(extract_minecraft_version("26.3-snapshot-5"), "26.3");
        assert_eq!(extract_minecraft_version("26"), "26");
    }

    #[test]
    fn extracts_version_from_complex_format() {
        assert_eq!(extract_minecraft_version("1.20.1-fabric-0.15.11"), "1.20.1");
        assert_eq!(extract_minecraft_version("1.19.4-forge-45.2.0"), "1.19.4");
    }

    #[test]
    fn handles_edge_cases() {
        // 低版本号不应被误判为版本号
        assert_eq!(extract_minecraft_version("5-something"), "5-something");
        // 高版本号应该被正确识别
        assert_eq!(extract_minecraft_version("20"), "20");
        assert_eq!(extract_minecraft_version("21"), "21");
    }

    #[test]
    fn detects_forge_mc_version_from_merged_libraries() {
        let version = json!({
            "libraries": [
                { "name": "net.minecraftforge:fmlloader:1.20.1-47.4.9" }
            ]
        });

        assert_eq!(
            detect_minecraft_version_from_libraries(&version).as_deref(),
            Some("1.20.1")
        );
    }

    #[test]
    fn skips_zeroed_fabric_intermediary_placeholder() {
        // 合并型整合包中 fabric 的 intermediary 会被置为 "0.0.0"，
        // 它不含真实版本信息，不应被当作 Minecraft 版本。
        let version = json!({
            "libraries": [
                { "name": "net.fabricmc:intermediary:0.0.0" }
            ]
        });

        assert_eq!(detect_minecraft_version_from_libraries(&version), None);
    }

    #[test]
    fn detects_mc_version_from_merged_modpack_patches() {
        // PVZ_Survive 这类合并型整合包：没有 inheritsFrom，
        // 但 patches 里 id == "game" 的补丁携带真实版本号。
        let version = json!({
            "patches": [
                { "id": "game", "version": "26.1.2", "priority": 0 },
                { "id": "fabric", "version": "0.19.3", "priority": 1 }
            ]
        });

        assert_eq!(
            detect_minecraft_version_from_patches(&version).as_deref(),
            Some("26.1.2")
        );
    }

    #[test]
    fn igores_patches_without_game_entry() {
        let version = json!({
            "patches": [
                { "id": "fabric", "version": "0.19.3", "priority": 1 }
            ]
        });

        assert_eq!(detect_minecraft_version_from_patches(&version), None);
    }

    #[test]
    fn keeps_release_and_snapshot_versions_instead_of_asset_index_ids() {
        assert_eq!(version_from_instance_name("26.2").as_deref(), Some("26.2"));
        assert_eq!(
            version_from_instance_name("26.3-snapshot-5").as_deref(),
            Some("26.3")
        );
        assert_eq!(
            version_from_instance_name("25w42a").as_deref(),
            Some("25w42a")
        );
        assert_eq!(version_from_instance_name("custom-profile"), None);
    }

    #[test]
    fn detects_new_snapshot_format() {
        assert_eq!(version_from_instance_name("26").as_deref(), Some("26"));
        assert_eq!(version_from_instance_name("26.3").as_deref(), Some("26.3"));
        assert_eq!(version_from_instance_name("27").as_deref(), Some("27"));
        // 低版本号不应被识别
        assert_eq!(version_from_instance_name("5"), None);
    }

    #[test]
    fn detects_quilt_before_generic_knot_main_class() {
        assert_eq!(
            detect_loader_from_main_class("org.quiltmc.loader.impl.launch.knot.KnotClient"),
            "Quilt"
        );
        assert_eq!(
            detect_loader_from_main_class("net.fabricmc.loader.impl.launch.knot.KnotClient"),
            "Fabric"
        );
    }

    #[test]
    fn detects_optifine_loader() {
        assert_eq!(
            detect_loader_from_name("26.1.2-OptiFine-HD_U_K1_pre2"),
            "OptiFine"
        );
        assert_eq!(
            detect_loader_from_name("1.21.1-OptiFine-HD_U_I7"),
            "OptiFine"
        );
    }

    #[test]
    fn avoids_fabric_false_positive() {
        // 测试包含 "fabric" 但不是加载器的名称不会被误判
        assert_eq!(
            detect_loader_from_name("SHser-Basic-Package Edit by Meversation"),
            "Vanilla"
        );
        assert_eq!(
            detect_loader_from_name("some-fabric-name-not-loader"),
            "Vanilla"
        );
        // 但实际的 Fabric 加载器格式应该被正确识别
        assert_eq!(
            detect_loader_from_name("fabric-loader-0.15.0-1.21.1"),
            "Fabric"
        );
        assert_eq!(
            detect_loader_from_name("1.21.1-fabric-0.15.11"),
            "Fabric"
        );
    }
}
