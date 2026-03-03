use crate::downloader::original_dwl::process_version;

#[tauri::command]
pub async fn download_patcher(mc_version: String) -> Result<(), String> {
    let minecraft_path = std::env::current_dir().map_err(|e| e.to_string())?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);

    // 接收并打印百分比
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            println!("下载进度: {:.1}%", percent);
        }
    });

    process_version(&mc_version, &minecraft_path, tx).await.map_err(|e| e.to_string())?;
    Ok(())
}
