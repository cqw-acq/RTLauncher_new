use serde::Serialize;
use sysinfo::System;

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
