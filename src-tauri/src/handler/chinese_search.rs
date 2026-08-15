use reqwest;
use serde::{Deserialize, Serialize};
use sqlite::{Connection, State};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// moddata.db 的全局连接（单例，延迟初始化）
fn get_moddata_connection() -> &'static Mutex<Option<Connection>> {
    static MDDATA_CONN: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();
    MDDATA_CONN.get_or_init(|| Mutex::new(None))
}

/// 获取 moddata.db 的隐藏存放目录（各平台用户数据目录，默认均不直接暴露）：
///   Linux:   $XDG_DATA_HOME/rtlauncher 或 ~/.local/share/rtlauncher（.local 为隐藏目录）
///   macOS:   ~/Library/Application Support/rtlauncher（Library 在 Finder 中默认隐藏）
///   Windows: %APPDATA%\rtlauncher（AppData 目录默认隐藏）
fn get_moddata_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("rtlauncher"))
}

/// 获取 moddata.db 的目标路径（优先存放于隐藏的用户数据目录）
fn get_moddata_target_path() -> PathBuf {
    if let Some(dir) = get_moddata_data_dir() {
        return dir.join("moddata.db");
    }
    // 兜底：无用户数据目录时退回可执行文件同目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("moddata.db");
        }
    }
    PathBuf::from(".")
}

/// 获取 moddata.db 的文件路径
/// 查找顺序：
///   1. 隐藏的用户数据目录（新版本默认位置）
///   2. 程序当前目录下的 moddata.db（旧版本遗留）
///   3. 程序可执行文件同目录下的 moddata.db（旧版本遗留）
///   4. 上层目录的 moddata.db（开发环境）
fn resolve_moddata_path() -> Option<PathBuf> {
    // 1) 隐藏的用户数据目录
    if let Some(dir) = get_moddata_data_dir() {
        let p = dir.join("moddata.db");
        if p.exists() {
            return Some(p);
        }
    }

    // 2) 当前工作目录（旧版本遗留）
    let cwd = PathBuf::from(".");
    let p1 = cwd.join("moddata.db");
    if p1.exists() {
        return Some(p1);
    }

    // 3) 可执行文件目录（旧版本遗留）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("moddata.db");
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 4) 上层目录（开发环境，项目根）
    let p3 = cwd.join("..").join("moddata.db");
    if p3.exists() {
        return Some(p3);
    }

    // 5) src-tauri 上层
    let p4 = cwd.join("..").join("..").join("moddata.db");
    if p4.exists() {
        return Some(p4);
    }

    None
}

/// 远程数据库的候选下载地址（按顺序尝试，成功后即停止）
/// 新源可在此追加：GitHub 优先，GitHub 不可达或资源不存在时回退到 GitCode
const MODDATA_DOWNLOAD_URLS: &[&str] = &[
    // 首选：GitHub（需在 https://github.com/cqw-acq/RTLauncher_new 的
    //       "工具" 标签发布 moddata.db 资源后才生效）
    "https://github.com/cqw-acq/RTLauncher_new/releases/download/%E5%B7%A5%E5%85%B7/moddata.db",
    // 备选：GitCode（GitHub 无法访问时的镜像）
    "https://gitcode.com/bubulaladdi/RTLauncher/releases/download/%E5%B7%A5%E5%85%B7/moddata.db",
];

/// 从远程下载 moddata.db
fn download_moddata_db() -> Result<PathBuf, String> {
    let target_path = get_moddata_target_path();

    // 确保目标目录存在
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let client = reqwest::blocking::Client::builder()
        // GitHub 无法访问时连接会挂起，限制连接超时以便快速回退到下一源
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut last_error: Option<String> = None;
    for url in MODDATA_DOWNLOAD_URLS {
        println!("[moddata] 正在下载数据库文件: {}", url);
        match download_from_url(&client, url, &target_path) {
            Ok(()) => {
                println!("[moddata] 数据库文件已下载到: {}", target_path.display());
                return Ok(target_path);
            }
            Err(e) => {
                println!("[moddata] 从 {} 下载失败: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "所有下载源均不可用".to_string()))
}

fn download_from_url(
    client: &reqwest::blocking::Client,
    url: &str,
    target_path: &PathBuf,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP状态码: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("读取下载内容失败: {}", e))?;

    fs::write(target_path, bytes).map_err(|e| format!("写入数据库文件失败: {}", e))?;
    Ok(())
}

/// 打开 moddata 数据库连接（首次调用时建立，之后复用）
fn ensure_moddata_connection() -> Result<(), String> {
    let lock = get_moddata_connection();
    let mut guard = lock.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Ok(());
    }

    let path = match resolve_moddata_path() {
        Some(p) => p,
        None => {
            println!("[moddata] 未找到本地数据库，尝试从远程下载...");
            download_moddata_db()?
        }
    };

    println!("[moddata] 使用数据库: {}", path.display());
    let conn = sqlite::open(&path).map_err(|e| format!("打开数据库失败: {}", e))?;
    *guard = Some(conn);
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModSearchResult {
    pub slug: String,
    pub chinese_name: String,
    pub mcmod_id: Option<i64>,
}

/// 在 moddata.db 中按中文关键词模糊搜索
/// 
/// # 参数
/// * `keyword` - 中文关键词（也可接受英文 slug 模糊匹配）
/// 
/// # 返回
/// JSON 字符串，格式：[{"slug":"...", "chinese_name":"..."}, ...]
#[tauri::command]
pub fn search_moddata(keyword: String) -> Result<String, String> {
    let keyword_trimmed = keyword.trim();
    if keyword_trimmed.is_empty() {
        return Err("搜索关键词不能为空".to_string());
    }

    ensure_moddata_connection()?;

    let lock = get_moddata_connection();
    let guard = lock.lock().map_err(|e| e.to_string())?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| "数据库连接未初始化".to_string())?;

    // 同时按 chinese_name 和 slug 做 LIKE 模糊匹配
    // 允许用户输入中文或部分 slug
    let like_pattern = format!("%{}%", keyword_trimmed);

    let query = "SELECT slug, chinese_name, mcmod_id FROM mod_names \
                 WHERE chinese_name LIKE ?1 OR slug LIKE ?1 \
                 ORDER BY (CASE WHEN chinese_name LIKE ?2 THEN 0 ELSE 1 END), \
                          (CASE WHEN slug LIKE ?2 THEN 0 ELSE 1 END), \
                          id ASC \
                 LIMIT 200";

    let mut statement = conn
        .prepare(query)
        .map_err(|e| format!("准备查询失败: {}", e))?;
    let startswith_pattern = format!("{}%", keyword_trimmed);
    statement
        .bind((1, like_pattern.as_str()))
        .map_err(|e| format!("绑定参数失败: {}", e))?;
    statement
        .bind((2, startswith_pattern.as_str()))
        .map_err(|e| format!("绑定参数失败: {}", e))?;

    let mut results: Vec<ModSearchResult> = Vec::new();
    while let State::Row = statement.next().map_err(|e| format!("查询失败: {}", e))? {
        let slug = statement.read::<String, _>(0).unwrap_or_default();
        let chinese_name = statement.read::<String, _>(1).unwrap_or_default();
        let mcmod_id: Option<i64> = statement.read::<i64, _>(2).ok();
        results.push(ModSearchResult {
            slug,
            chinese_name,
            mcmod_id,
        });
    }

    serde_json::to_string(&results).map_err(|e| format!("序列化结果失败: {}", e))
}

/// 获取 moddata 数据库的基本信息（调试用）
#[tauri::command]
pub fn get_moddata_info() -> Result<String, String> {
    ensure_moddata_connection()?;

    let lock = get_moddata_connection();
    let guard = lock.lock().map_err(|e| e.to_string())?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| "数据库连接未初始化".to_string())?;

    let mut statement = conn
        .prepare("SELECT COUNT(*) FROM mod_names")
        .map_err(|e| format!("准备查询失败: {}", e))?;

    if let State::Row = statement.next().map_err(|e| format!("查询失败: {}", e))? {
        let count = statement.read::<i64, _>(0).unwrap_or(0);
        let path = resolve_moddata_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "未知".to_string());
        Ok(format!("{{\"count\":{},\"path\":\"{}\"}}", count, path))
    } else {
        Err("无法获取记录数".to_string())
    }
}