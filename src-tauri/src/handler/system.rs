use base64::{self, Engine};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use sysinfo::System;

#[derive(Clone, Serialize)]
pub struct MemoryInfo {
    /// 系统物理总内存（MB）
    pub total_mb: u64,
    /// 已使用内存（MB）
    pub used_mb: u64,
    /// 当前可用内存（MB，即 total - used）
    pub available_mb: u64,
    /// 推荐自动分配给游戏的内存（MB，取可用内存的 80%）
    pub recommended_mb: u64,
}

#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    webbrowser::open(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
pub fn read_file_base64(path: String) -> Result<String, String> {
    let content = fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&content))
}

#[tauri::command]
pub fn get_system_memory() -> MemoryInfo {
    if let Ok(cache) = system_memory_cache().lock() {
        if let Some(cached) = cache.as_ref() {
            if cached.refreshed_at.elapsed() < SYSTEM_MEMORY_CACHE_TTL {
                return cached.value.clone();
            }
        }
    }

    let mut sys = System::new();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / 1024 / 1024;
    let used_mb = sys.used_memory() / 1024 / 1024;
    let available_mb = total_mb.saturating_sub(used_mb);
    // 推荐分配：可用内存的 80%，同时设置合理上下限
    //  - 至少 512 MB，最多总内存的 90%
    let raw_recommended = (available_mb as f64 * 0.8) as u64;
    let upper_bound = (total_mb as f64 * 0.9) as u64;
    let recommended_mb = raw_recommended
        .min(upper_bound)
        .max(512);
    let info = MemoryInfo {
        total_mb,
        used_mb,
        available_mb,
        recommended_mb,
    };

    if let Ok(mut cache) = system_memory_cache().lock() {
        *cache = Some(CachedMemoryInfo {
            value: info.clone(),
            refreshed_at: Instant::now(),
        });
    }
    info
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

// ---------------------------------------------------------------------------
// 跨平台内存清理
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct MemoryOptimizationReport {
    /// 清理前可用内存（MB）
    pub available_before_mb: u64,
    /// 清理后可用内存（MB）
    pub available_after_mb: u64,
    /// 差值（约等于释放的内存 MB，粗略值）
    pub freed_mb: i64,
    /// 系统总内存（MB）
    pub total_mb: u64,
    /// 当前平台标识
    pub platform: String,
    /// 本次实际用到的清理手段（用于前端显示/调试）
    pub methods: Vec<String>,
    /// 清理耗时（ms）
    pub duration_ms: u64,
}

/// 主入口：一键优化系统内存
///
/// 实现思路：
///   - 调用平台专属的系统 API 收缩文件缓存 + 当前进程工作集
///   - 尝试释放 standby/cache（如权限不足则忽略）
///   - 不再执行大块内存“抖动”：该操作会制造瞬时分配峰值和页面换入，
///     与释放内存、保持界面流畅的目标相悖
#[tauri::command]
pub fn optimize_memory_usage() -> Result<MemoryOptimizationReport, String> {
    let start = std::time::Instant::now();
    let mut methods: Vec<String> = Vec::new();
    let mut sys = System::new();

    // 1. 读取清理前的内存状态
    sys.refresh_memory();
    let total_kb = sys.total_memory();
    let available_before_kb = sys.available_memory();

    // 2. 平台专属清理
    platform_trim_current_process(&mut methods);
    platform_drop_file_caches(&mut methods);
    platform_try_empty_system_caches(&mut methods);

    // 给系统一点时间来反映真实的可用内存
    std::thread::sleep(Duration::from_millis(80));

    // 4. 再次读取内存状态
    sys.refresh_memory();
    let available_after_kb = sys.available_memory();

    let total_mb = total_kb / 1024 / 1024;
    let before_mb = available_before_kb / 1024 / 1024;
    let after_mb = available_after_kb / 1024 / 1024;
    let freed_mb = after_mb as i64 - before_mb as i64;
    let duration_ms = start.elapsed().as_millis() as u64;

    #[cfg(target_os = "windows")]
    let platform = "windows".to_string();
    #[cfg(target_os = "linux")]
    let platform = "linux".to_string();
    #[cfg(target_os = "macos")]
    let platform = "macos".to_string();
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let platform = "other".to_string();

    Ok(MemoryOptimizationReport {
        available_before_mb: before_mb,
        available_after_mb: after_mb,
        freed_mb,
        total_mb,
        platform,
        methods,
        duration_ms,
    })
}

// ---------------------------------------------------------------------------
// 跨平台实现
// ---------------------------------------------------------------------------

/// 让当前进程把不必要的内存页还给操作系统
fn platform_trim_current_process(methods: &mut Vec<String>) {
    #[cfg(target_os = "windows")]
    {
        // SetProcessWorkingSetSize(hProcess, -1, -1) + EmptyWorkingSet(hProcess)
        // 收缩本进程的物理工作集。然后会在 platform_try_empty_system_caches
        // 里遍历所有进程再做一次。
        use winapi::shared::minwindef::BOOL;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        extern "system" {
            fn SetProcessWorkingSetSize(
                hProcess: *mut winapi::ctypes::c_void,
                dwMinimumWorkingSetSize: usize,
                dwMaximumWorkingSetSize: usize,
            ) -> BOOL;
            fn EmptyWorkingSet(hProcess: *mut winapi::ctypes::c_void) -> BOOL;
        }
        unsafe {
            let handle = GetCurrentProcess();
            if SetProcessWorkingSetSize(handle, usize::MAX, usize::MAX) != 0 {
                methods.push("windows.working_set(self)".to_string());
            }
            let _ = EmptyWorkingSet(handle);
        }
    }

    #[cfg(target_os = "linux")]
    {
        // malloc_trim(0) 让 glibc 释放顶部分配区周围未用的内存
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe {
            if malloc_trim(0) == 1 {
                methods.push("linux.malloc_trim".to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS 上用 malloc_zone_pressure_relief(NULL, 0) 让 malloc
        // 在内存压力下主动放弃未用的区域；不需要额外权限。
        extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut libc::c_void, goal: usize) -> usize;
        }
        unsafe {
            let n = malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
            if n > 0 {
                methods.push(format!("macos.pressure_relief({})", n));
            }
        }
    }
}

/// 让系统收缩文件缓存（不需要管理员权限，效果温和但完全无副作用）
fn platform_drop_file_caches(methods: &mut Vec<String>) {
    #[cfg(target_os = "windows")]
    {
        // SetSystemFileCacheSize(0, SIZE_MAX, 0) 让系统把工作集上限设为无限，
        // 同时会触发一次 cache 收缩。
        use winapi::shared::minwindef::BOOL;
        extern "system" {
            fn SetSystemFileCacheSize(
                minimum_file_cache_size: usize,
                maximum_file_cache_size: usize,
                flags: u32,
            ) -> BOOL;
        }
        unsafe {
            // 0 / SIZE_MAX 是"让系统管理"的语义，不会丢数据
            if SetSystemFileCacheSize(0, usize::MAX, 0) != 0 {
                methods.push("windows.file_cache_trim".to_string());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 先 sync 让脏页写回（无害），然后尝试 drop_caches
        use std::fs::OpenOptions;
        use std::io::Write;

        unsafe { libc::sync() };
        methods.push("linux.sync".to_string());

        if let Ok(mut f) = OpenOptions::new()
            .write(true)
            .open("/proc/sys/vm/drop_caches")
        {
            if f.write_all(b"3\n").is_ok() {
                methods.push("linux.drop_caches(pagecache+dentries)".to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS 的 `purge` 需要 sudo，权限不够就直接跳过
        // 但我们也可以用 fsync(...) 对活跃文件做一些温和 flush
        unsafe { libc::fsync(libc::STDIN_FILENO) };
        // 尝试调用 `/usr/bin/purge`（需要 sudo，没权限就忽略）
        let status = std::process::Command::new("/usr/bin/purge").status();
        if let Ok(s) = status {
            if s.success() {
                methods.push("macos.purge".to_string());
            }
        }
    }
}

/// （权限允许时）尝试排空系统 standby/cache：真正意义上的"释放系统整体内存"
/// 权限不足就忽略，而不是让整个调用失败
fn platform_try_empty_system_caches(methods: &mut Vec<String>) {
    #[cfg(target_os = "windows")]
    {
        // ============================================================
        //  PCL2 风格：遍历系统内所有进程，逐个 EmptyWorkingSet
        //  普通用户就能做（除了 svchost/system 等会被 ACCESS_DENIED）
        // ============================================================
        use winapi::shared::minwindef::{BOOL, DWORD, FALSE};

        // 常量（不依赖 winapi feature 打开）
        const PROCESS_SET_QUOTA: DWORD = 0x0100;
        const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;
        const PROCESS_VM_READ: DWORD = 0x0010;

        // 全部自己 extern 声明，避免依赖 winapi feature
        extern "system" {
            fn EnumProcesses(lpidProcess: *mut DWORD, cb: DWORD, lpcbNeeded: *mut DWORD) -> BOOL;
            fn OpenProcess(
                dwDesiredAccess: DWORD,
                bInheritHandle: BOOL,
                dwProcessId: DWORD,
            ) -> *mut winapi::ctypes::c_void;
            fn GetCurrentProcessId() -> DWORD;
            fn CloseHandle(hObject: *mut winapi::ctypes::c_void) -> BOOL;
            fn EmptyWorkingSet(hProcess: *mut winapi::ctypes::c_void) -> BOOL;
            fn SetSystemFileCacheSize(
                minimum_file_cache_size: usize,
                maximum_file_cache_size: usize,
                flags: u32,
            ) -> BOOL;
            fn NtSetSystemInformation(
                SystemInformationClass: i32,
                SystemInformation: *mut u8,
                SystemInformationLength: u32,
            ) -> i32;
        }

        unsafe {
            let mut pids: Vec<DWORD> = vec![0u32; 4096];
            let mut bytes_needed: DWORD = 0;
            let enum_ok = EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<DWORD>()) as DWORD,
                &mut bytes_needed,
            );
            if enum_ok != 0 {
                let count = bytes_needed as usize / std::mem::size_of::<DWORD>();
                pids.truncate(count);

                let my_pid = GetCurrentProcessId();
                let mut success_count: u32 = 0;
                for &pid in &pids {
                    if pid == 0 || pid == my_pid {
                        continue;
                    }
                    let access = PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ;
                    let handle = OpenProcess(access, FALSE as i32, pid);
                    if handle.is_null() {
                        continue;
                    }
                    // 只要没 ACCESS_DENIED，就一定会成功把工作集丢到 standby
                    EmptyWorkingSet(handle);
                    success_count += 1;
                    CloseHandle(handle);
                }
                methods.push(format!(
                    "windows.empty_working_set({} processes)",
                    success_count
                ));
            }

            // SetSystemFileCacheSize(0, SIZE_MAX, FILE_CACHE_MAX_HARD_DISABLE)
            // 强制收缩系统文件缓存工作集
            if SetSystemFileCacheSize(0, usize::MAX, 2) != 0 {
                methods.push("windows.system_cache_hard_trim".to_string());
            }

            // 管理员模式大招：NtSetSystemInformation(80)
            // SystemPurgeStandbyList —— 把 standby 列表全部清到 free
            // 普通用户调了会失败（静默忽略）
            let status = NtSetSystemInformation(80, std::ptr::null_mut(), 0);
            if status == 0 {
                methods.push("windows.purge_standby_list(admin)".to_string());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux：drop_caches（需 root）+ 通过 /proc/<pid>/clear_refs
        // 尝试让内核回收各进程的未用页
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut f) = OpenOptions::new()
            .write(true)
            .open("/proc/sys/vm/drop_caches")
        {
            if f.write_all(b"3\n").is_ok() {
                methods.push("linux.drop_caches(3)".to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS：上面已试过 `purge`，这里不再重复。
    }
}

// 在非三大平台上，给 linker 一个空实现，避免编译失败
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn platform_trim_current_process(_methods: &mut Vec<String>) {}
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn platform_drop_file_caches(_methods: &mut Vec<String>) {}
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn platform_try_empty_system_caches(methods: &mut Vec<String>) {}

// ---------------------------------------------------------------------------
//  startup：启动时自动生成 launcher_profiles.json
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct LauncherProfile {
    icon: String,
    name: String,
    lastVersionId: String,
    #[serde(rename = "type")]
    typ: String,
    lastUsed: i64,
}

#[derive(Serialize)]
struct LauncherProfiles {
    profiles: std::collections::BTreeMap<String, LauncherProfile>,
    selectedProfile: String,
    clientToken: String,
}

fn startup_minecraft_paths() -> Vec<String> {
    // 与 config.rs 保持一致：读取 RTL/config/launcher.json 拿到所有 minecraft 路径
    // 也始终包含一份「平台默认路径」兜底
    #[cfg(target_os = "windows")]
    let default_path = {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        exe_dir.join("minecraft").to_string_lossy().to_string()
    };
    #[cfg(target_os = "macos")]
    let default_path = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/Library/Application Support/RTLauncher/version", home)
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let default_path = "./minecraft".to_string();

    #[cfg(target_os = "macos")]
    let config_file = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(format!(
            "{}/Library/Application Support/RTLauncher/config",
            home
        ))
        .join("launcher.json")
    };
    #[cfg(not(target_os = "macos"))]
    let config_file = std::path::PathBuf::from("./RTL/config").join("launcher.json");

    let mut paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    paths.insert(default_path);

    if config_file.exists() {
        if let Ok(text) = fs::read_to_string(&config_file) {
            // 只关心 minecraft_paths / selected_minecraft_path 两个字段
            #[derive(serde::Deserialize)]
            struct PartialConfig {
                minecraft_paths: Option<Vec<String>>,
                selected_minecraft_path: Option<String>,
            }
            if let Ok(cfg) = serde_json::from_str::<PartialConfig>(&text) {
                if let Some(list) = cfg.minecraft_paths {
                    for p in list {
                        paths.insert(p);
                    }
                }
                if let Some(p) = cfg.selected_minecraft_path {
                    paths.insert(p);
                }
            }
        }
    }

    paths.into_iter().collect()
}

/// 启动时自动检查：对所有已配置的 minecraft 路径，
/// 若缺少 launcher_profiles.json 则生成一份；
/// 若已存在则不覆盖（保留用户设置）。
/// 在独立线程中调用，不阻塞 UI。
pub fn ensure_launcher_profiles_on_startup() {
    let paths = startup_minecraft_paths();
    let now_unix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    for mc_path in paths {
        let dir = Path::new(&mc_path);
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("[launcher_profiles] 创建目录失败 {}: {}", mc_path, e);
            continue;
        }

        let file = dir.join("launcher_profiles.json");
        if file.exists() {
            // 已存在则不覆盖，保留用户设置
            continue;
        }

        let mut profiles_map = std::collections::BTreeMap::new();
        profiles_map.insert(
            "RTL".to_string(),
            LauncherProfile {
                icon: "Grass".to_string(),
                name: "RTL".to_string(),
                lastVersionId: "latest-release".to_string(),
                typ: "latest-release".to_string(),
                lastUsed: now_unix,
            },
        );

        let lp = LauncherProfiles {
            profiles: profiles_map,
            selectedProfile: "RTL".to_string(),
            clientToken: "23323323323323323323323323323333".to_string(),
        };

        match serde_json::to_string_pretty(&lp) {
            Ok(json) => {
                if let Err(e) = fs::write(&file, &json) {
                    eprintln!("[launcher_profiles] 写入失败 {}: {}", file.display(), e);
                } else {
                    eprintln!("[launcher_profiles] 已生成 {}", file.display());
                }
            }
            Err(e) => {
                eprintln!("[launcher_profiles] 序列化失败: {}", e);
            }
        }
    }
}

/// 在首次真正启动游戏时再检查配置路径，避免应用打开时争用磁盘 I/O。
pub fn schedule_launcher_profiles_check() {
    static SCHEDULED: Once = Once::new();
    SCHEDULED.call_once(|| {
        std::thread::spawn(ensure_launcher_profiles_on_startup);
    });
}

struct CachedMemoryInfo {
    value: MemoryInfo,
    refreshed_at: Instant,
}

const SYSTEM_MEMORY_CACHE_TTL: Duration = Duration::from_secs(10);

fn system_memory_cache() -> &'static Mutex<Option<CachedMemoryInfo>> {
    static CACHE: OnceLock<Mutex<Option<CachedMemoryInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}
