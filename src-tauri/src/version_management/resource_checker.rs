use std::fs;
use std::path::Path;
use std::collections::HashMap;
use fastnbt::{Value, from_bytes, to_bytes};
use serde::Deserialize;


pub type PackInfo = (String, String, String);

#[derive(Debug, Deserialize)]
pub struct LevelInfo {
    #[serde(default)]
    #[serde(rename = "RandomSeed")]
    pub random_seed: Option<i64>,
    #[serde(default)]
    #[serde(rename = "Seed")]
    pub seed: Option<i64>,
    #[serde(default)]
    #[serde(rename = "GameRules")]
    pub game_rules: Option<Value>,
    #[serde(default)]
    #[serde(rename = "WorldGenSettings")]
    pub world_gen_settings: Option<Value>,
    #[serde(default)]
    #[serde(rename = "allowCommands")]
    pub allow_commands: Option<Value>,
}

impl Default for LevelInfo {
    fn default() -> Self {
        Self {
            random_seed: None,
            seed: None,
            game_rules: Some(Value::Compound(HashMap::new())),
            world_gen_settings: None,
            allow_commands: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RootCompound {
    #[serde(rename = "Data")]
    data: LevelInfo,
}


/// 解析单个光影包/材质包信息
fn parse_resource_pack(folder_abs_path: &str) -> Option<PackInfo> {
    let path = Path::new(folder_abs_path);

    // 1. 获取材质包文件夹名称
    let folder_name = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown_Pack")
        .to_string();

    // 2. 查找 pack.png
    let pack_png_path = path.join("pack.png");
    let pack_png_str = if pack_png_path.exists() && pack_png_path.is_file() {
        pack_png_path.to_string_lossy().replace(r"\", "/")
    } else {
        String::new()
    };

    // 3. 解析 pack.mcmeta 获取版本号
    let version_string = get_mc_version(path);

    Some((folder_name, pack_png_str, version_string))
}

/// 遍历文件夹中的所有子文件夹，解析每个光影包/材质包信息
pub fn find_resource_packs(root_path: &str) -> Vec<PackInfo> {
    let root = Path::new(root_path).join("resourcepacks");
    let mut packs = Vec::new();

    // 读取根目录下的所有条目
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();

            // 只处理目录
            if path.is_dir() {
                // 尝试解析每个子文件夹
                if let Some(pack_info) = parse_resource_pack(&path.to_string_lossy().replace(r"\", "/")) {
                    packs.push(pack_info);
                }
            }
        }
    }

    packs
}

/// 辅助函数：从 pack.mcmeta 中解析版本号
pub fn get_mc_version(folder_path: &Path) -> String {
    let mcmeta_path = folder_path.join("pack.mcmeta");

    // 如果文件不存在，返回空字符串
    if !mcmeta_path.exists() {
        return String::new();
    }

    // 读取文件内容
    let content = match fs::read_to_string(&mcmeta_path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains(r#""pack_format":"#) {
            // 找到包含 "pack_format": 的行
            // 提取冒号后面的数字部分
            if let Some(colon_pos) = trimmed.find(':') {
                let after_colon = &trimmed[colon_pos + 1..];
                // 提取数字（跳过可能的空格）
                let num_str: String = after_colon
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();

                if let Ok(format_num) = num_str.parse::<u64>() {
                    return translate_pack_format_to_version(format_num);
                }
            }
        }
    }

    String::new()
}


fn translate_pack_format_to_version(format: u64) -> String {
    match format {
        1 => "1.6.1 ~ 1.8.9".to_string(),
        2 => "1.9 ~ 1.10.2".to_string(),
        3 => "1.11 ~ 1.12.2".to_string(),
        4 => "1.13 ~ 1.14.4".to_string(),
        5 => "1.15 ~ 1.16.1".to_string(),
        6 => "1.16.2 ~ 1.16.5".to_string(),
        7 => "1.17 ~ 1.17.1".to_string(),
        8 => "1.18 ~ 1.18.2".to_string(),
        9 => "1.19 ~ 1.19.2".to_string(),
        11 => "1.19.3".to_string(),
        12 => "1.19.4".to_string(),
        15 => "1.20 ~ 1.20.1".to_string(),
        18 => "1.20.2".to_string(),
        22 => "1.20.3~1.20.4".to_string(),
        32 => "1.20.5".to_string(),
        34 => "1.21".to_string(),
        46 => "1.21.4".to_string(),
        55 => "1.21.5".to_string(),
        63 => "1.21.6".to_string(),
        64 => "1.21.7~1.21.8".to_string(),
        // 如果遇到未知的版本号，返回数字本身或特定提示
        _ => format!("版本位置, 格式为 {}", format),
    }
}

/// 扫描根路径下的 instance 文件夹，返回所有文件夹的名称数组
pub fn scan_instances(instances_path: &Path) -> Vec<String> {
    let mut instance_names = Vec::new();

    // 检查 instances 文件夹是否存在
    if !instances_path.exists() || !instances_path.is_dir() {
        return instance_names;
    }

    // 读取 instances 文件夹下的所有条目
    if let Ok(entries) = fs::read_dir(instances_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 只处理目录
            if path.is_dir() {
                // 获取路径的最后一部分（文件夹名称）
                let folder_name = path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Unknown_Instance")
                    .to_string();
                instance_names.push(folder_name);
            }
        }
    }

    instance_names
}

/// 解析level.dat文件，返回种子和游戏规则信息
/// 参数：world_folder_abs_path - 世界文件夹的绝对路径
/// 返回：Option<Vec<String>> - 包含RandomSeed或seed的值，keepInventory的值，mobGriefing的值，doFireTick的值，allowCommands的值的数组
pub fn parse_level_dat(world_folder_abs_path: &str) -> Option<Vec<String>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let path = Path::new(world_folder_abs_path);
    let level_dat_path = path.join("level.dat");

    // 检查level.dat文件是否存在
    if !level_dat_path.exists() || !level_dat_path.is_file() {
        return None;
    }

    // 读取文件内容
    let bytes = match fs::read(&level_dat_path) {
        Ok(b) => b,
        Err(_) => return None,
    };

    // 使用GZIP解压数据
    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_err() {
        return None;
    }
    
    let root: RootCompound = match from_bytes(&decompressed) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("解析 NBT 数据失败: {:?}", e);
            return None;
        }
    };
    let level_info = root.data;
    // 获取种子值：优先从RandomSeed获取，如果没有则从WorldGenSettings中的seed获取，最后再从seed获取
    let seed_value = if let Some(random_seed) = level_info.random_seed {
        random_seed
    } else if let Some(Value::Compound(settings)) = level_info.world_gen_settings {
        settings.get("seed")
            .and_then(|v| match v {
                Value::Long(l) => Some(*l),
                Value::Int(i) => Some(*i as i64),
                _ => None,
            })
            .unwrap_or(0)
    } else if let Some(seed) = level_info.seed {
        seed
    } else {
        0
    };

    // 从level_info的game_rules中获取游戏规则映射
    let rules_map = match &level_info.game_rules {
        Some(Value::Compound(rules)) => rules,
        _ => return Some(vec![
            seed_value.to_string(),
            "false".to_string(),
            "false".to_string(),
            "false".to_string(),
            "false".to_string(),
        ]),
    };

    // 辅助函数：从游戏规则中获取布尔值
    let get_bool_value = |value: Option<&Value>| -> bool {
        value
            .and_then(|v| match v {
                Value::Byte(b) => Some(*b != 0),
                Value::String(s) => Some(s.to_lowercase() == "true"),
                _ => None,
            })
            .unwrap_or(false)
    };

    // 从level_info的game_rules中获取各项值
    let keep_inventory = get_bool_value(rules_map.get("keepInventory"));
    let mob_griefing = get_bool_value(rules_map.get("mobGriefing"));
    let do_fire_tick = get_bool_value(rules_map.get("doFireTick"));
    
    // 从level_info.allow_commands中获取allowCommands的值
    let allow_commands = get_bool_value(level_info.allow_commands.as_ref());

    // 返回格式：[RandomSeed/seed, keepInventory, mobGriefing, doFireTick, allowCommands]
    Some(vec![
        seed_value.to_string(),
        keep_inventory.to_string(),
        mob_griefing.to_string(),
        do_fire_tick.to_string(),
        allow_commands.to_string(),
    ])
}

/// 修改 NBT 文件中的参数值
/// 参数：
///   - world_folder_abs_path: 世界文件夹的绝对路径
///   - param_name: 要修改的参数名
///   - new_value: 要修改成的值（支持 String, i64, bool 类型）
/// 返回：Result<(), String> 表示操作是否成功
pub fn modify_nbt_param(
    world_folder_abs_path: &str,
    param_name: &str,
    new_value: NBTValue,
) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Read, Write};

    let path = Path::new(world_folder_abs_path);
    let level_dat_path = path.join("level.dat");

    // 读取并解压文件
    let bytes = fs::read(&level_dat_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)
        .map_err(|_| "解压文件失败".to_string())?;

    // 解析 NBT 数据
    let mut nbt_value: Value = from_bytes(&decompressed)
        .map_err(|e| format!("解析 NBT 数据失败: {:?}", e))?;

    // 递归查找并修改参数
    modify_value_recursive(&mut nbt_value, param_name, &new_value)?;

    // 序列化、压缩并写入文件
    let modified_bytes = to_bytes(&nbt_value)
        .map_err(|e| format!("序列化失败: {:?}", e))?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&modified_bytes)
        .map_err(|_| "压缩失败".to_string())?;

    let compressed = encoder.finish()
        .map_err(|e| format!("完成压缩失败: {}", e))?;

    fs::write(&level_dat_path, compressed)
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}

/// 修改 NBT 文件中的参数值（字符串版本）
/// 参数：
///   - world_folder_abs_path: 世界文件夹的绝对路径
///   - param_name: 要修改的参数名，仅支持：keepInventory、mobGriefing、doFireTick、allowCommands
///   - new_value: 要修改成的值（字符串类型，会自动转换为适当的NBT类型）
/// 返回：Result<(), String> 表示操作是否成功
pub fn modify_nbt_param_str(
    world_folder_abs_path: &str,
    param_name: &str,
    new_value: &str,
) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Read, Write};

    // 验证参数名是否合法
    let param_name_lower = param_name.to_lowercase();
    if !matches!(
        param_name_lower.as_str(),
        "keepinventory" | "mobgriefing" | "dofiretick" | "allowcommands"
    ) {
        return Err(format!("不支持的参数名: {}，仅支持: keepInventory, mobGriefing, doFireTick, allowCommands", param_name));
    }

    let path = Path::new(world_folder_abs_path);
    let level_dat_path = path.join("level.dat");

    // 读取并解压文件
    let bytes = fs::read(&level_dat_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)
        .map_err(|_| "解压文件失败".to_string())?;

    // 解析 NBT 数据
    let mut nbt_value: Value = from_bytes(&decompressed)
        .map_err(|e| format!("解析 NBT 数据失败: {:?}", e))?;

    // 转换字符串值为适当的 NBT 类型
    // 注意：游戏规则(GameRules)中的布尔值存储为 String("true"/"false")
    //       allowCommands 存储为 Byte(0/1)（真逆天吧）
    let is_game_rule = matches!(param_name_lower.as_str(), "keepinventory" | "mobgriefing" | "dofiretick");
    
    let nbt_value_to_set = if let Ok(b) = new_value.parse::<bool>() {
        if is_game_rule {
            // 游戏规则的布尔值存储为 NBT String
            NBTValue::String(if b { "true".to_string() } else { "false".to_string() })
        } else {
            // allowCommands 等字段的布尔值存储为 NBT Byte
            NBTValue::Byte(if b { 1 } else { 0 })
        }
    } else if let Ok(i) = new_value.parse::<i64>() {
        if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
            NBTValue::Byte(i as i8)
        } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
            NBTValue::Short(i as i16)
        } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
            NBTValue::Int(i as i32)
        } else {
            NBTValue::Long(i)
        }
    } else if let Ok(f) = new_value.parse::<f64>() {
        NBTValue::Double(f)
    } else {
        NBTValue::String(new_value.to_string())
    };

    // 根据参数名修改对应的值
    if let Value::Compound(root_map) = &mut nbt_value {
        // 获取 Data 节点
        if let Some(Value::Compound(data_map)) = root_map.get_mut("Data") {
            // 处理游戏规则（keepInventory、mobGriefing、doFireTick）
            if matches!(param_name_lower.as_str(), "keepinventory" | "mobgriefing" | "dofiretick") {
                // 若 GameRules 不存在则自动创建
                if !data_map.contains_key("GameRules") {
                    data_map.insert("GameRules".to_string(), Value::Compound(HashMap::new()));
                }
                if let Some(Value::Compound(game_rules)) = data_map.get_mut("GameRules") {
                    // 优先查找已有的同名键（大小写可能不同），否则直接用标准名写入
                    let existing_key = game_rules.keys()
                        .find(|k| k.to_lowercase() == param_name_lower)
                        .cloned();
                    let key = existing_key.unwrap_or_else(|| param_name.to_string());
                    game_rules.insert(key, nbt_value_to_set.to_value());
                } else {
                    return Err("GameRules 节点类型不是 Compound".to_string());
                }
            }
            // 处理 allowCommands
            else if param_name_lower == "allowcommands" {
                // 查找已有键（大小写不敏感），若不存在则直接创建
                let existing_key = data_map.keys()
                    .find(|k| k.to_lowercase() == param_name_lower)
                    .cloned();
                let key = existing_key.unwrap_or_else(|| "allowCommands".to_string());
                data_map.insert(key, nbt_value_to_set.to_value());
            }
        } else {
            return Err("Data 节点不存在".to_string());
        }
    } else {
        return Err("NBT 数据格式错误".to_string());
    }

    // 序列化、压缩并写入文件
    let modified_bytes = to_bytes(&nbt_value)
        .map_err(|e| format!("序列化失败: {:?}", e))?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&modified_bytes)
        .map_err(|_| "压缩失败".to_string())?;

    let compressed = encoder.finish()
        .map_err(|e| format!("完成压缩失败: {}", e))?;

    fs::write(&level_dat_path, compressed)
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}

/// 递归查找并修改 NBT 值
fn modify_value_recursive(
    value: &mut Value,
    param_name: &str,
    new_value: &NBTValue,
) -> Result<(), String> {
    match value {
        Value::Compound(map) => {
            // 检查当前 Compound 是否包含目标参数
            if map.contains_key(param_name) {
                map.insert(param_name.to_string(), new_value.to_value());
                return Ok(());
            }
            // 递归查找子 Compound
            for (_, v) in map.iter_mut() {
                if let Err(e) = modify_value_recursive(v, param_name, new_value) {
                    if e != "参数未找到" {
                        return Err(e);
                    }
                } else {
                    return Ok(());
                }
            }
            Err("参数未找到".to_string())
        }
        Value::List(list) => {
            // 递归查找 List 中的元素
            for v in list.iter_mut() {
                if let Err(e) = modify_value_recursive(v, param_name, new_value) {
                    if e != "参数未找到" {
                        return Err(e);
                    }
                } else {
                    return Ok(());
                }
            }
            Err("参数未找到".to_string())
        }
        _ => Err("参数未找到".to_string()),
    }
}

/// NBT 值的枚举类型
#[derive(Debug, Clone)]
pub enum NBTValue {
    String(String),
    Int(i32),
    Long(i64),
    Short(i16),
    Byte(i8),
    Float(f32),
    Double(f64),
    Bool(bool),
}

impl NBTValue {
    fn to_value(&self) -> Value {
        match self {
            NBTValue::String(s) => Value::String(s.clone()),
            NBTValue::Int(i) => Value::Int(*i),
            NBTValue::Long(l) => Value::Long(*l),
            NBTValue::Short(s) => Value::Short(*s),
            NBTValue::Byte(b) => Value::Byte(*b),
            NBTValue::Float(f) => Value::Float(*f),
            NBTValue::Double(d) => Value::Double(*d),
            NBTValue::Bool(b) => Value::Byte(if *b { 1 } else { 0 }),
        }
    }
}
