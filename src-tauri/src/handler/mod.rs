pub mod cache_paths;
pub mod chinese_search;
pub mod config;
pub mod diagnostics;
// Tauri command argument names intentionally match the existing frontend API.
#[allow(dead_code, non_snake_case)]
pub mod fabric_handler;
#[allow(dead_code, non_snake_case)]
pub mod forge_handler;
#[allow(dead_code)]
pub mod java_downloader;
pub mod java_scanner;
#[allow(dead_code, non_snake_case)]
pub mod launcher;
#[allow(dead_code, non_snake_case)]
pub mod liteloader_handler;
#[allow(dead_code, non_snake_case)]
pub mod mod_links;
pub mod mod_parser;
#[allow(dead_code)]
pub mod modpack_builder;
#[allow(non_snake_case)]
pub mod modpack_installer_handler;
#[allow(dead_code, non_snake_case)]
pub mod neoforge_handler;
#[allow(dead_code, non_snake_case)]
pub mod optifine_handler;
#[allow(dead_code, non_snake_case)]
pub mod quilt_handler;
#[allow(dead_code, non_snake_case)]
pub mod system;