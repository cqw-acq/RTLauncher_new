use std::fs;
use std::path::Path;

fn main() {
    let config_path = Path::new("update_target.json");
    if config_path.exists() {
        match fs::read_to_string(config_path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        if let Some(target_name) = json.get("target_release_name").and_then(|v| v.as_str()) {
                            println!("cargo:rustc-env=UPDATE_TARGET_RELEASE_NAME={}", target_name);
                            println!("cargo:warning=Update target release name set to: {}", target_name);
                        } else {
                            println!("cargo:warning=update_target.json missing 'target_release_name' field, using default");
                            println!("cargo:rustc-env=UPDATE_TARGET_RELEASE_NAME=");
                        }
                    }
                    Err(e) => {
                        println!("cargo:warning=Failed to parse update_target.json: {}, using default", e);
                        println!("cargo:rustc-env=UPDATE_TARGET_RELEASE_NAME=");
                    }
                }
            }
            Err(e) => {
                println!("cargo:warning=Failed to read update_target.json: {}, using default", e);
                println!("cargo:rustc-env=UPDATE_TARGET_RELEASE_NAME=");
            }
        }
    } else {
        println!("cargo:warning=update_target.json not found, using default empty target");
        println!("cargo:rustc-env=UPDATE_TARGET_RELEASE_NAME=");
    }

    println!("cargo:rerun-if-changed=update_target.json");
    tauri_build::build()
}