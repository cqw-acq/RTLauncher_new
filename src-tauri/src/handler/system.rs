use serde::Serialize;
use sysinfo::System;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct MemoryInfo {
    /// 系统物理总内存（MB）
    pub total_mb: u64,
    /// 已使用内存（MB）
    pub used_mb: u64,
}

/// 获取系统内存信息
#[tauri::command]
pub fn get_system_memory() -> MemoryInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / 1024 / 1024;
    let used_mb = sys.used_memory() / 1024 / 1024;
    MemoryInfo { total_mb, used_mb }
}

/// 写入文件
#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    let file_path = Path::new(&path);

    // 确保父目录存在
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 写入文件
    fs::write(file_path, content).map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}
