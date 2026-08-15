use std::env;
use std::path::{Path, PathBuf};

use super::OPENP2P_BIN;

fn legacy_bridge_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .parent()
        .map(|parent| parent.join("RTL").join("bridge"))
}

fn preferred_bridge_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(PathBuf::from)
            .map(|base| base.join("RTLauncher").join("bridge"))
            .unwrap_or_else(|| env::temp_dir().join("RTLauncher").join("bridge"));
    }

    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|home| {
                home.join("Library")
                    .join("Application Support")
                    .join("RTLauncher")
                    .join("bridge")
            })
            .unwrap_or_else(|| env::temp_dir().join("RTLauncher").join("bridge"));
    }

    #[cfg(target_os = "linux")]
    {
        crate::app_paths::linux_data_dir().join("bridge")
    }
}

pub(super) fn bridge_dir() -> Result<PathBuf, String> {
    let preferred = preferred_bridge_dir();
    if preferred.join(OPENP2P_BIN).is_file() {
        return Ok(preferred);
    }

    if let Some(legacy) = legacy_bridge_dir() {
        if legacy.join(OPENP2P_BIN).is_file() {
            return Ok(legacy);
        }
    }
    Ok(preferred)
}

pub(super) fn openp2p_path() -> Result<PathBuf, String> {
    Ok(bridge_dir()?.join(OPENP2P_BIN))
}

pub(super) fn openp2p_dir() -> Result<PathBuf, String> {
    let path = openp2p_path()?;
    Ok(path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".")))
}

pub(super) fn executable_path(path: &Path) -> Result<String, String> {
    if path.is_absolute() {
        return Ok(path.display().to_string());
    }

    Ok(env::current_dir()
        .map_err(|error| format!("无法获取当前目录: {}", error))?
        .join(path)
        .display()
        .to_string())
}
