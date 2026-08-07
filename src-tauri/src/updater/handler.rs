use std::sync::Arc;
use std::sync::Mutex;

use tauri::State;

use super::config::{get_update_config, should_check_update, UpdateConfig};
use super::fetcher::{UpdateCheckResult, UpdateFetcher};

pub struct UpdaterState {
    fetcher: Arc<Mutex<UpdateFetcher>>,
}

#[tauri::command]
pub fn get_update_status() -> UpdateConfig {
    get_update_config()
}

#[tauri::command]
pub async fn check_for_updates(
    state: State<'_, UpdaterState>,
) -> Result<UpdateCheckResult, String> {
    let fetcher = {
        let fetcher = state.fetcher.lock().map_err(|e| e.to_string())?;
        fetcher.clone()
    };
    fetcher.check_for_update().await
}

#[tauri::command]
pub async fn download_update(
    state: State<'_, UpdaterState>,
) -> Result<super::fetcher::DownloadResult, String> {
    let fetcher = {
        let fetcher = state.fetcher.lock().map_err(|e| e.to_string())?;
        fetcher.clone()
    };
    fetcher.download_update().await
}

#[tauri::command]
pub async fn install_update(
    state: State<'_, UpdaterState>,
) -> Result<super::fetcher::InstallResult, String> {
    let fetcher = {
        let fetcher = state.fetcher.lock().map_err(|e| e.to_string())?;
        fetcher.clone()
    };
    fetcher.install_update().await
}

#[tauri::command]
pub fn cancel_update(state: State<'_, UpdaterState>) -> Result<(), String> {
    let fetcher = state.fetcher.lock().map_err(|e| e.to_string())?;
    fetcher.cancel();
    Ok(())
}

#[tauri::command]
pub fn can_check_update() -> bool {
    should_check_update()
}

pub fn create_updater_state() -> UpdaterState {
    UpdaterState {
        fetcher: Arc::new(Mutex::new(UpdateFetcher::new())),
    }
}