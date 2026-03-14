pub mod common_libraries;
pub mod downloader_wrapper;
pub mod fabric_installer;
pub mod forge_installer;
pub mod quilt_installer;
pub mod neoforge_installer;
pub mod optifine_installer;
pub mod liteloader_installer;

// ─── Fabric ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_fabric_versions(mc_version: &str, use_mirror: bool) -> Result<Vec<String>, String> {
    fabric_installer::get_fabric_loader_versions(mc_version, use_mirror)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_fabric_api_versions(mc_version: &str) -> Result<Vec<String>, String> {
    fabric_installer::get_fabric_api_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_fabric(mc_version: &str, loader_version: &str, mc_folder_path: &str) -> Result<(), String> {
    fabric_installer::install_fabric_loader(mc_version, loader_version, mc_folder_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_fabric_api(mc_version: &str, fabric_api_version: &str, mc_folder_path: &str) -> Result<(), String> {
    fabric_installer::install_fabric_api(mc_version, fabric_api_version, mc_folder_path)
        .map_err(|e| e.to_string())
}

// ─── Forge ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_forge_versions(mc_version: &str) -> Result<Vec<String>, String> {
    forge_installer::get_forge_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_forge(mc_version: &str, forge_version: &str, download_dir: &str) -> Result<(), String> {
    forge_installer::install_forge(mc_version, forge_version, download_dir)
        .map_err(|e| e.to_string())
}

// ─── Quilt ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_quilt_versions(mc_version: &str) -> Result<Vec<String>, String> {
    quilt_installer::get_quilt_loader_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_quilt_api_versions(mc_version: &str) -> Result<Vec<String>, String> {
    quilt_installer::get_quilt_api_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_quilt(mc_version: &str, quilt_version: &str, mc_folder_path: &str) -> Result<(), String> {
    quilt_installer::install_quilt_loader(mc_version, quilt_version, mc_folder_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_quilt_api(mc_version: &str, quilt_api_version: &str, mc_folder_path: &str) -> Result<(), String> {
    quilt_installer::install_quilt_api(mc_version, quilt_api_version, mc_folder_path)
        .map_err(|e| e.to_string())
}

// ─── NeoForge ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_neoforge_versions(mc_version: &str) -> Result<Vec<String>, String> {
    neoforge_installer::get_neoforge_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_neoforge(mc_version: &str, neoforge_version: &str, download_dir: &str) -> Result<(), String> {
    neoforge_installer::install_neoforge(mc_version, neoforge_version, download_dir)
        .map_err(|e| e.to_string())
}

// ─── OptiFine ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_optifine_versions(mc_version: &str) -> Result<Vec<String>, String> {
    optifine_installer::list_optifine_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_optifine_alone(optifine_version: &str, minecraft_dir: &str, opt_wrapper_path: &str, mc_version: &str) -> Result<(), String> {
    optifine_installer::install_optifine_alone(optifine_version, minecraft_dir, opt_wrapper_path, mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_optifine_with_forge(optifine_version: &str, minecraft_dir: &str) -> Result<(), String> {
    optifine_installer::install_optifine_with_forge(optifine_version, minecraft_dir)
        .map_err(|e| e.to_string())
}

// ─── LiteLoader ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_liteloader_versions(mc_version: &str) -> Result<Vec<String>, String> {
    liteloader_installer::get_liteloader_versions(mc_version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_liteloader(mc_version: &str, lite_version: &str, minecraft_dir: &str) -> Result<(), String> {
    liteloader_installer::install_liteloader(mc_version, lite_version, std::path::Path::new(minecraft_dir))
        .map_err(|e| e.to_string())
}
