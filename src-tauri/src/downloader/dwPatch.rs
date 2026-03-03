mod original_dwl;
use original_dwl::process_version;
use std::error::Error;

#[tauri::command]
async fn download_patcher(mc_version:String) -> Result<(), Box<dyn Error + Send + Sync>> {  // 修改返回类型
    let minecraft_path = std::env::current_dir()?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<f64>(64);

    // 接收并打印百分比
    tokio::spawn(async move {
        while let Some(percent) = rx.recv().await {
            println!("下载进度: {:.1}%", percent);
        }
    });

    process_version(mc_version, &minecraft_path, tx).await?;
    Ok(())
}
