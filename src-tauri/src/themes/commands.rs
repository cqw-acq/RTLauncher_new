use super::store::{ThemePackage, ThemeStore, ThemeStoreState};
use super::ThemeStoreError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

pub struct ThemeStoreManager(Mutex<ThemeStore>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStoreView {
    pub active_theme_id: String,
    pub last_healthy_theme_id: String,
    pub pending_theme_id: Option<String>,
    pub packages: Vec<ThemePackage>,
}

pub fn initialize_theme_store(app: &AppHandle) -> Result<(), ThemeStoreError> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| {
            ThemeStoreError::new(
                "THEME_STORE_IO",
                format!("Cannot resolve the application data directory: {error}"),
            )
        })?
        .join("themes");
    app.manage(ThemeStoreManager(Mutex::new(ThemeStore::open(root)?)));
    Ok(())
}

fn with_store<T>(
    manager: State<'_, ThemeStoreManager>,
    operation: impl FnOnce(&mut ThemeStore) -> Result<T, ThemeStoreError>,
) -> Result<T, ThemeStoreError> {
    let mut store = manager.0.lock().map_err(|_| {
        ThemeStoreError::new("THEME_STORE_LOCKED", "Theme store lock is not available.")
    })?;
    operation(&mut store)
}

#[tauri::command]
pub fn theme_list(
    manager: State<'_, ThemeStoreManager>,
) -> Result<ThemeStoreView, ThemeStoreError> {
    with_store(manager, |store| {
        let state: &ThemeStoreState = store.state();
        Ok(ThemeStoreView {
            active_theme_id: state.active_theme_id.clone(),
            last_healthy_theme_id: state.last_healthy_theme_id.clone(),
            pending_theme_id: state.pending_theme_id.clone(),
            packages: store.list()?,
        })
    })
}

#[tauri::command]
pub fn theme_install_archive(
    manager: State<'_, ThemeStoreManager>,
    archive_path: String,
) -> Result<ThemePackage, ThemeStoreError> {
    with_store(manager, |store| {
        store.install_archive(&PathBuf::from(archive_path))
    })
}

#[tauri::command]
pub fn theme_register_dev_directory(
    manager: State<'_, ThemeStoreManager>,
    directory: String,
) -> Result<ThemePackage, ThemeStoreError> {
    with_store(manager, |store| {
        store.register_dev_directory(&PathBuf::from(directory))
    })
}

#[tauri::command]
pub fn theme_remove(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
    version: Option<String>,
) -> Result<(), ThemeStoreError> {
    with_store(manager, |store| store.remove(&theme_id, version.as_deref()))
}

#[tauri::command]
pub fn theme_read_text(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
    path: String,
) -> Result<String, ThemeStoreError> {
    with_store(manager, |store| store.read_text(&theme_id, &path))
}

#[tauri::command]
pub fn theme_read_binary(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
    path: String,
) -> Result<String, ThemeStoreError> {
    with_store(manager, |store| {
        store
            .read_binary(&theme_id, &path)
            .map(|content| STANDARD.encode(content))
    })
}

#[tauri::command]
pub fn theme_set_active(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
) -> Result<(), ThemeStoreError> {
    with_store(manager, |store| store.set_active(&theme_id))
}

#[tauri::command]
pub fn theme_mark_healthy(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
) -> Result<(), ThemeStoreError> {
    with_store(manager, |store| store.mark_healthy(&theme_id))
}

#[tauri::command]
pub fn theme_is_trusted(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
    version: String,
) -> Result<bool, ThemeStoreError> {
    with_store(manager, |store| Ok(store.is_trusted(&theme_id, &version)))
}

#[tauri::command]
pub fn theme_set_trusted(
    manager: State<'_, ThemeStoreManager>,
    theme_id: String,
    version: String,
    trusted: bool,
) -> Result<(), ThemeStoreError> {
    with_store(manager, |store| {
        store.set_trusted(&theme_id, &version, trusted)
    })
}
