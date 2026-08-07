use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::config::{
    get_current_os, get_update_config, get_update_endpoint, matches_asset_name, save_update_config,
    should_check_update, UpdateStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitCodeAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default, rename = "type")]
    pub asset_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitCodeRelease {
    pub tag_name: String,
    pub name: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GitCodeAsset>,
}

#[derive(Clone)]
pub struct UpdateFetcher {
    cancel_flag: Arc<AtomicBool>,
    is_downloading: Arc<AtomicBool>,
}

impl UpdateFetcher {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            is_downloading: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub fn is_downloading(&self) -> bool {
        self.is_downloading.load(Ordering::SeqCst)
    }

    fn extract_version(input: &str) -> Option<String> {
        let re = regex::Regex::new(r"(\d+\.\d+\.\d+(?:\.\d+)?)").ok()?;
        re.captures(input).map(|c| c.get(1).unwrap().as_str().to_string())
    }

    pub async fn check_for_update(&self) -> Result<UpdateCheckResult, String> {
        if !should_check_update() {
            let cfg = get_update_config();
            return Ok(UpdateCheckResult {
                needs_check: false,
                update_available: cfg.target_version.is_some(),
                current_version: cfg.current_version,
                target_version: cfg.target_version,
                message: "距上次检查不足24小时，请稍后再试".to_string(),
            });
        }

        self.cancel_flag.store(false, Ordering::SeqCst);

        let mut cfg = get_update_config();
        cfg.status = UpdateStatus::Checking;
        let _ = save_update_config(cfg.clone());

        let endpoint = get_update_endpoint();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let response = client
            .get(&endpoint)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("请求更新信息失败: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            cfg.status = UpdateStatus::Error(format!("服务器返回错误: {}", status));
            let _ = save_update_config(cfg);
            return Err(format!("服务器返回错误: {}", status));
        }

        let releases: Vec<GitCodeRelease> = response
            .json()
            .await
            .map_err(|e| format!("解析更新信息失败: {}", e))?;

        if releases.is_empty() {
            cfg.status = UpdateStatus::Idle;
            let _ = save_update_config(cfg);
            return Ok(UpdateCheckResult {
                needs_check: true,
                update_available: false,
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                target_version: None,
                message: "当前没有可用的发布版本".to_string(),
            });
        }

        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let current_os = get_current_os();

        let release = releases
            .iter()
            .find(|r| !r.prerelease)
            .unwrap_or(&releases[0]);

        let release_version = Self::extract_version(&release.name)
            .or_else(|| Self::extract_version(&release.tag_name))
            .unwrap_or_else(|| release.name.clone());

        let matching_asset = release
            .assets
            .iter()
            .find(|a| matches_asset_name(&a.name));

        let has_update = release_version != current_version && matching_asset.is_some();

        if has_update {
            if let Some(asset) = matching_asset {
                cfg.target_version = Some(release_version);
                cfg.target_os = Some(current_os);
                cfg.download_url = Some(asset.browser_download_url.clone());
                cfg.file_size = asset.size;
                cfg.status = UpdateStatus::Available;
            }
        } else {
            cfg.target_version = None;
            cfg.target_os = None;
            cfg.download_url = None;
            cfg.file_size = None;
            cfg.status = UpdateStatus::Idle;
        }

        cfg.last_check_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );

        let _ = save_update_config(cfg.clone());

        Ok(UpdateCheckResult {
            needs_check: true,
            update_available: has_update,
            current_version,
            target_version: cfg.target_version,
            message: if has_update {
                "发现新版本可用".to_string()
            } else {
                "当前已是最新版本".to_string()
            },
        })
    }

    pub async fn download_update(&self) -> Result<DownloadResult, String> {
        if self.is_downloading.load(Ordering::SeqCst) {
            return Err("已有更新正在下载中".to_string());
        }

        let cfg = get_update_config();
        let download_url = match &cfg.download_url {
            Some(url) => url.clone(),
            None => return Err("没有可下载的更新".to_string()),
        };

        self.is_downloading.store(true, Ordering::SeqCst);
        self.cancel_flag.store(false, Ordering::SeqCst);

        let mut cfg = cfg;
        cfg.status = UpdateStatus::Downloading;
        cfg.download_progress = Some(0.0);
        let _ = save_update_config(cfg.clone());

        let config_dir = super::config::config_dir();
        let download_dir = PathBuf::from(&config_dir).join("updates");
        let _ = fs::create_dir_all(&download_dir);

        let target_filename = download_url
            .rsplit('/')
            .next()
            .unwrap_or("update.bin")
            .to_string();
        let download_path = download_dir.join(&target_filename);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let response = client
            .get(&download_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "*/*")
            .header("Referer", "https://gitcode.com/")
            .send()
            .await
            .map_err(|e| format!("下载更新失败: {}", e))?;

        if !response.status().is_success() {
            self.is_downloading.store(false, Ordering::SeqCst);
            cfg.status = UpdateStatus::Error(format!("下载失败: HTTP {}", response.status()));
            let _ = save_update_config(cfg);
            return Err(format!("下载失败: HTTP {}", response.status()));
        }

        let total_size = response.content_length();

        let mut file = fs::File::create(&download_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        while let Some(chunk) = stream.next().await {
            if self.cancel_flag.load(Ordering::SeqCst) {
                self.is_downloading.store(false, Ordering::SeqCst);
                let _ = fs::remove_file(&download_path);
                let mut cfg = get_update_config();
                cfg.status = UpdateStatus::Idle;
                cfg.download_progress = None;
                let _ = save_update_config(cfg);
                return Err("下载已取消".to_string());
            }

            let chunk = chunk.map_err(|e| format!("下载数据错误: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;

            downloaded += chunk.len() as u64;

            if let Some(total) = total_size {
                let progress = (downloaded as f64 / total as f64) * 100.0;
                let mut cfg = get_update_config();
                cfg.download_progress = Some(progress);
                let _ = save_update_config(cfg);
            }
        }

        self.is_downloading.store(false, Ordering::SeqCst);

        let mut cfg = get_update_config();
        cfg.status = UpdateStatus::Downloaded;
        cfg.download_progress = Some(100.0);
        cfg.download_path = Some(download_path.to_string_lossy().to_string());
        let _ = save_update_config(cfg);

        Ok(DownloadResult {
            success: true,
            path: download_path.to_string_lossy().to_string(),
            size: downloaded,
        })
    }

    pub async fn install_update(&self) -> Result<InstallResult, String> {
        let cfg = get_update_config();
        let download_path_str = match &cfg.download_path {
            Some(path) => path.clone(),
            None => return Err("No downloaded update available".to_string()),
        };

        let download_path = PathBuf::from(&download_path_str);
        if !download_path.exists() {
            return Err("Update file does not exist".to_string());
        }

        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?;

        let exe_dir = current_exe
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .to_path_buf();

        let temp_dir = exe_dir.join("update_temp");
        let _ = fs::create_dir_all(&temp_dir);

        let extracted = self.extract_if_needed(&download_path, &temp_dir)?;

        let backup_dir = exe_dir.join("backup");
        let _ = fs::create_dir_all(&backup_dir);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let current_os = get_current_os();

        if current_os.starts_with("windows") {
            self.install_windows(&download_path, &temp_dir, &backup_dir, timestamp, extracted, &exe_dir)?;
        } else if current_os.starts_with("macos") {
            self.install_macos(&download_path_str, &temp_dir, &backup_dir, timestamp, extracted, &exe_dir)?;
        } else if current_os.starts_with("linux") {
            self.install_linux(&download_path, &temp_dir, &backup_dir, timestamp, extracted, &exe_dir)?;
        } else {
            return Err("Unsupported OS for update".to_string());
        }

        Ok(InstallResult {
            success: true,
            message: "Installer started".to_string(),
        })
    }

    fn install_windows(
        &self,
        download_path: &PathBuf,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        let target_exe = exe_dir.join("RTLauncher.exe");
        if target_exe.exists() {
            let backup_path = backup_dir.join(format!("RTLauncher_backup_{}.exe", timestamp));
            let _ = fs::copy(&target_exe, &backup_path);
        }

        let source_file = if extracted {
            self.find_executable_in_dir(temp_dir, "windows")?
        } else {
            download_path.clone()
        };

        let script = format!(
            "@echo off\n\
            chcp 65001 >nul\n\
            echo Installing update...\n\
            timeout /t 3 /nobreak >nul\n\
            copy /y \"{source}\" \"{target}\"\n\
            if %errorlevel% neq 0 (\n\
                echo Install failed\n\
                pause\n\
                exit /b 1\n\
            )\n\
            echo Install complete, restarting...\n\
            start \"\" \"{target}\"\n\
            timeout /t 2 /nobreak >nul\n\
            rmdir /s /q \"{temp}\"\n\
            del \"%~f0\"",
            source = source_file.to_string_lossy(),
            target = target_exe.to_string_lossy(),
            temp = temp_dir.to_string_lossy()
        );

        let script_path = exe_dir.join("update_installer.bat");
        fs::write(&script_path, &script)
            .map_err(|e| format!("Write install script failed: {}", e))?;

        std::process::Command::new("cmd")
            .args(["/C", script_path.to_string_lossy().as_ref()])
            .spawn()
            .map_err(|e| format!("Start installer failed: {}", e))?;

        std::process::exit(0);
    }

    fn install_macos(
        &self,
        download_path_str: &str,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        if extracted {
            let target_app = self.find_app_bundle_in_dir(temp_dir)?;
            let target_path = exe_dir.join("RTLauncher.app");

            if target_path.exists() {
                let backup_path = backup_dir.join(format!("RTLauncher_backup_{}.app", timestamp));
                let _ = self.copy_dir_recursive(&target_path, &backup_path);
            }

            let script = format!(
                "#!/bin/bash\n\
                set -e\n\
                echo \"Installing update...\"\n\
                sleep 3\n\
                rm -rf \"{target}\"\n\
                cp -R \"{source}\" \"{target}\"\n\
                echo \"Install complete, restarting...\"\n\
                open \"{target}\"\n\
                rm -rf \"{temp}\"\n\
                rm -- \"$0\"",
                source = target_app.to_string_lossy(),
                target = target_path.to_string_lossy(),
                temp = temp_dir.to_string_lossy()
            );

            let script_path = exe_dir.join("update_installer.sh");
            fs::write(&script_path, &script)
                .map_err(|e| format!("Write install script failed: {}", e))?;

            self.make_executable(&script_path)?;

            std::process::Command::new("open")
                .arg(&script_path)
                .spawn()
                .map_err(|e| format!("Start installer failed: {}", e))?;

            std::process::exit(0);
        } else {
            let target_binary = exe_dir.join("RTLauncher");
            if target_binary.exists() {
                let backup_path = backup_dir.join(format!("RTLauncher_backup_{}", timestamp));
                let _ = fs::copy(&target_binary, &backup_path);
            }

            let script = format!(
                "#!/bin/bash\n\
                set -e\n\
                echo \"Installing update...\"\n\
                sleep 3\n\
                cp \"{source}\" \"{target}\"\n\
                chmod +x \"{target}\"\n\
                echo \"Install complete, restarting...\"\n\
                \"{target}\" &\n\
                rm -rf \"{temp}\"\n\
                rm -- \"$0\"",
                source = download_path_str,
                target = target_binary.to_string_lossy(),
                temp = temp_dir.to_string_lossy()
            );

            let script_path = exe_dir.join("update_installer.sh");
            fs::write(&script_path, &script)
                .map_err(|e| format!("Write install script failed: {}", e))?;

            self.make_executable(&script_path)?;

            std::process::Command::new("open")
                .arg(&script_path)
                .spawn()
                .map_err(|e| format!("Start installer failed: {}", e))?;

            std::process::exit(0);
        }
    }

    fn install_linux(
        &self,
        download_path: &PathBuf,
        temp_dir: &PathBuf,
        backup_dir: &PathBuf,
        timestamp: u64,
        extracted: bool,
        exe_dir: &PathBuf,
    ) -> Result<(), String> {
        let target_binary = exe_dir.join("RTLauncher");
        if target_binary.exists() {
            let backup_path = backup_dir.join(format!("RTLauncher_backup_{}", timestamp));
            let _ = fs::copy(&target_binary, &backup_path);
        }

        let source_file = if extracted {
            self.find_executable_in_dir(temp_dir, "linux")?
        } else {
            download_path.clone()
        };

        let script = format!(
            "#!/bin/bash\n\
            set -e\n\
            echo \"Installing update...\"\n\
            sleep 3\n\
            cp \"{source}\" \"{target}\"\n\
            chmod +x \"{target}\"\n\
            echo \"Install complete, restarting...\"\n\
            nohup \"{target}\" &\n\
            rm -rf \"{temp}\"\n\
            rm -- \"$0\"",
            source = source_file.to_string_lossy(),
            target = target_binary.to_string_lossy(),
            temp = temp_dir.to_string_lossy()
        );

        let script_path = exe_dir.join("update_installer.sh");
        fs::write(&script_path, &script)
            .map_err(|e| format!("Write install script failed: {}", e))?;

        self.make_executable(&script_path)?;

        std::process::Command::new("sh")
            .arg(&script_path)
            .spawn()
            .map_err(|e| format!("Start installer failed: {}", e))?;

        std::process::exit(0);
    }

    #[allow(unused_variables)]
    fn make_executable(&self, path: &PathBuf) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(path, perms)
                .map_err(|e| format!("Set script permissions failed: {}", e))?;
        }
        Ok(())
    }

    fn extract_if_needed(&self, source: &PathBuf, dest: &PathBuf) -> Result<bool, String> {
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "zip" => {
                let file = fs::File::open(source).map_err(|e| e.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
                    let outpath = dest.join(file.name());

                    if file.is_dir() {
                        fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            let _ = fs::create_dir_all(p);
                        }
                        let mut outfile = fs::File::create(&outpath).map_err(|e| e.to_string())?;
                        std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn find_executable_in_dir(&self, dir: &PathBuf, os: &str) -> Result<PathBuf, String> {
        let expected_name = match os {
            "windows" => "RTLauncher.exe",
            "macos" => "RTLauncher",
            "linux" => "RTLauncher",
            _ => "RTLauncher",
        };

        let search_paths = vec![
            dir.join(expected_name),
            dir.join("RTLauncher"),
            dir.join("bin").join(expected_name),
            dir.join("bin").join("RTLauncher"),
        ];

        for path in &search_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        self.find_file_recursive(dir, expected_name)
    }

    fn find_file_recursive(&self, dir: &PathBuf, name: &str) -> Result<PathBuf, String> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(found) = self.find_file_recursive(&path, name) {
                        return Ok(found);
                    }
                } else if path.file_name().map(|f| f == name).unwrap_or(false) {
                    return Ok(path);
                }
            }
        }
        Err(format!("找不到可执行文件: {}", name))
    }

    fn find_app_bundle_in_dir(&self, dir: &PathBuf) -> Result<PathBuf, String> {
        let search_paths = vec![
            dir.join("RTLauncher.app"),
            dir.join("RTLauncher-macos").join("RTLauncher.app"),
            dir.join("RTLauncher-aarch64").join("RTLauncher.app"),
        ];

        for path in &search_paths {
            if path.exists() && path.is_dir() {
                return Ok(path.clone());
            }
        }

        self.find_app_bundle_recursive(dir)
    }

    fn find_app_bundle_recursive(&self, dir: &PathBuf) -> Result<PathBuf, String> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_dir() {
                    if path.extension().map(|e| e == "app").unwrap_or(false) {
                        return Ok(path);
                    }
                    if let Ok(found) = self.find_app_bundle_recursive(&path) {
                        return Ok(found);
                    }
                }
            }
        }
        Err("找不到 .app 包".to_string())
    }

    fn copy_dir_recursive(&self, src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
        if !dst.exists() {
            fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        }
        for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let target_path = dst.join(entry.file_name());
            if path.is_dir() {
                self.copy_dir_recursive(&path, &target_path)?;
            } else {
                fs::copy(&path, &target_path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub needs_check: bool,
    pub update_available: bool,
    pub current_version: String,
    pub target_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub success: bool,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
}