pub mod version_fetcher;
pub mod decompression;
pub mod original_dwl;
pub mod dwPatch;

pub async fn download(url: &str, dest: &str, _max_threads: usize) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;
    let bytes = resp.bytes().await?;
    let path = std::path::Path::new(dest);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, bytes).await?;
    Ok(())
}
