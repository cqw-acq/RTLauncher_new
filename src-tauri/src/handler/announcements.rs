use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// 同步 GitHub 仓库中指定目录下的公告文本文件到本地 `announcements/` 目录。
/// 参数均为字符串，branch 可为空表示使用 "main"。
#[tauri::command]
pub fn sync_announcements(owner: String, repo: String, path: String, branch: Option<String>) -> Result<usize, String> {
    let branch = branch.unwrap_or_else(|| "main".to_string());
    let api_url = format!(
        "https://api.github.com/repos/cqw-acq/RTLauncher_new/contents/?ref={branch}"
    );

    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&api_url)
        .header("User-Agent", "RTLauncher/announcements-sync")
        .send()
        .map_err(|e| format!("请求 GitHub API 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回错误状态: {}", resp.status()));
    }

    let items: serde_json::Value = resp.json().map_err(|e| format!("解析 GitHub 响应失败: {}", e))?;
    if !items.is_array() {
        return Err("GitHub 返回的内容目录格式不正确".to_string());
    }

    let mut saved = 0usize;
    let cwd = std::env::current_dir().map_err(|e| format!("无法获取当前目录: {}", e))?;
    let announcements_dir = cwd.join("announcements");
    if !announcements_dir.exists() {
        fs::create_dir_all(&announcements_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    for item in items.as_array().unwrap() {
        if item.get("type").and_then(|v| v.as_str()) != Some("file") {
            continue;
        }
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if !name.to_lowercase().ends_with(".txt") {
            continue;
        }
        let download_url = item.get("download_url").and_then(|v| v.as_str()).ok_or_else(|| "缺少 download_url".to_string())?;
        let file_resp = client
            .get(download_url)
            .header("User-Agent", "RTLauncher/announcements-sync")
            .send()
            .map_err(|e| format!("下载公告文件失败: {}", e))?;
        if !file_resp.status().is_success() {
            continue;
        }
        let content = file_resp.text().map_err(|e| format!("读取公告文件内容失败: {}", e))?;
        let target = announcements_dir.join(name);
        fs::write(&target, content).map_err(|e| format!("写入公告文件失败: {}", e))?;
        saved += 1;
    }

    Ok(saved)
}

#[derive(Serialize)]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub content: String,
}

/// 从本地 `announcements/` 目录读取所有 txt 文件并返回解析后的公告。
#[tauri::command]
pub fn get_announcements() -> Result<Vec<Announcement>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("无法获取当前目录: {}", e))?;
    let announcements_dir = cwd.join("announcements");
    if !announcements_dir.exists() {
        return Ok(vec![]);
    }
    let mut result: Vec<Announcement> = Vec::new();
    for entry in fs::read_dir(&announcements_dir).map_err(|e| format!("读取目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path: PathBuf = entry.path();
        if !path.is_file() { continue; }
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if ext.to_lowercase() != "txt" { continue; }
        } else { continue; }

        let content = fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))?;
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let mut lines = content.lines();
        let title = lines.next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| id.clone());
        let body = lines.collect::<Vec<&str>>().join("\n");
        result.push(Announcement { id, title, content: body });
    }
    Ok(result)
}
