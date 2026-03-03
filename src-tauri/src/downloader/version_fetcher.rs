use serde::Deserialize;

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
#[tauri::command]
pub async fn classify_minecraft_versions() -> Result<[Vec<String>; 4], String> {
    // 获取版本清单数据（自动处理HTTP错误）
    let response = reqwest::get("https://piston-meta.mojang.com/mc/game/version_manifest.json")
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    // 解析JSON数据
    let manifest: VersionManifest = response.json().await.map_err(|e| e.to_string())?;

    // 初始化分类容器
    let mut releases = Vec::new();     // 正式版
    let mut snapshots = Vec::new();    // 快照版
    let mut april_fools = Vec::new();  // 愚人节版本
    let mut old_versions = Vec::new(); // 远古版

    // 分类处理
    for entry in manifest.versions {
        // 处理远古版（优先级最高）
        if matches!(entry.version_type.as_str(), "old_alpha" | "old_beta") {
            old_versions.push(entry.id);
            continue;
        }
        
        // 处理愚人节版本（次高优先级）
        if entry.time.contains("-04-01") {
            april_fools.push(entry.id);
            continue;
        }

        // 处理正式版和快照版
        match entry.version_type.as_str() {
            "release" => releases.push(entry.id),
            "snapshot" => snapshots.push(entry.id),
            _ => {} // 忽略其他类型
        }
    }

    Ok([releases, snapshots, april_fools, old_versions])
}

