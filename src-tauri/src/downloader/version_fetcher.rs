use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}
#[derive(Debug, Deserialize)]
struct VersionEntry {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    #[serde(rename = "releaseTime")]
    time: String,
}
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}
#[tauri::command]
pub async fn classify_minecraft_versions() -> Result<[Vec<VersionInfo>; 4], String> {
    let response = reqwest::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let manifest: VersionManifest = response.json().await.map_err(|e| e.to_string())?;
    let mut releases = Vec::new();     
    let mut snapshots = Vec::new();    
    let mut april_fools = Vec::new();  
    let mut old_versions = Vec::new(); 
    
    // 愚人节版本的特征标识符
    let april_fools_indicators = [
        "20w14infinite", "15w14a", "2.0", "1.RV-Pre1", "3D Shareware v1.34"
    ];
    
    for entry in manifest.versions {
        let info = VersionInfo {
            id: entry.id.clone(),
            release_time: entry.time.clone(),
        };
        
        // 首先检查是否为愚人节版本（通过版本ID或时间戳）
        let is_april_fools = april_fools_indicators.iter().any(|&indicator| {
            entry.id.to_lowercase().contains(&indicator.to_lowercase())
        }) || entry.time.contains("-04-01");
        
        if is_april_fools {
            april_fools.push(info);
            continue;
        }
        
        // 检查旧版本
        if matches!(entry.version_type.as_str(), "old_alpha" | "old_beta") {
            old_versions.push(info);
            continue;
        }
        
        // 按照版本类型分类
        match entry.version_type.as_str() {
            "release" => releases.push(info),
            "snapshot" => snapshots.push(info),
            _ => {
                // 对于未知类型，尝试通过版本ID格式推断
                if is_likely_snapshot(&entry.id) {
                    snapshots.push(info);
                } else {
                    releases.push(info);
                }
            }
        }
    }
    Ok([releases, snapshots, april_fools, old_versions])
}

/// 判断版本ID是否可能是快照版本
fn is_likely_snapshot(version_id: &str) -> bool {
    // 快照版本通常以年份+周数开头，如 24w12a, 25w42a
    let weekly_snapshot = Regex::new(r"^\d{2}w\d{2}[a-z]$").unwrap();
    // 包含 pre, rc, snapshot 等关键词
    let pre_release = Regex::new(r"(?i)(pre|rc|snapshot|beta|alpha)").unwrap();
    
    weekly_snapshot.is_match(version_id) || pre_release.is_match(version_id)
}