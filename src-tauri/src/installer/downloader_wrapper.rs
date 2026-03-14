use anyhow::Result;
use std::path::Path;
use tokio::runtime::Runtime;
use crate::downloader;

pub fn download_file_blocking(url: &str, path: &Path, max_threads: usize) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        downloader::download(url, path.to_string_lossy().as_ref(), max_threads).await
    })
}
