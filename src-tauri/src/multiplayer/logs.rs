use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LOG_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OPENP2P_TXT_OFFSET: Mutex<u64> = Mutex::new(0);
static OPENP2P_LOG_OFFSET: Mutex<u64> = Mutex::new(0);

pub(super) fn append(text: &[u8]) {
    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        buffer.extend_from_slice(text);
    }
}

pub(super) fn append_text(text: &str) {
    append(text.as_bytes());
}

fn openp2p_log_file(working_dir: &Path) -> PathBuf {
    working_dir.join("log").join("openp2p.txt")
}

fn openp2p_legacy_log_file(working_dir: &Path) -> PathBuf {
    working_dir.join("log").join("openp2p.log")
}

pub(super) fn clear_files(working_dir: &Path) {
    let log_dir = working_dir.join("log");
    if let Err(error) = fs::create_dir_all(&log_dir) {
        append_text(&format!("[RTLauncher] ⚠ 创建日志目录失败: {}\n", error));
        return;
    }

    for log_file in [
        openp2p_log_file(working_dir),
        openp2p_legacy_log_file(working_dir),
    ] {
        let _ = fs::File::create(log_file);
    }

    if let Ok(mut offset) = OPENP2P_TXT_OFFSET.lock() {
        *offset = 0;
    }
    if let Ok(mut offset) = OPENP2P_LOG_OFFSET.lock() {
        *offset = 0;
    }
}

pub(super) fn clear_buffer() {
    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        buffer.clear();
    }
}

pub(super) fn poll(working_dir: Option<&Path>) -> String {
    let mut content = if let Ok(mut buffer) = LOG_BUFFER.lock() {
        std::mem::take(&mut *buffer)
    } else {
        Vec::new()
    };

    if let Some(working_dir) = working_dir {
        for chunk in [
            read_increment(&openp2p_log_file(working_dir), &OPENP2P_TXT_OFFSET),
            read_increment(&openp2p_legacy_log_file(working_dir), &OPENP2P_LOG_OFFSET),
        ] {
            if !chunk.is_empty() {
                if !content.is_empty() && !content.ends_with(b"\n") {
                    content.push(b'\n');
                }
                content.extend_from_slice(&chunk);
            }
        }
    }

    String::from_utf8_lossy(&content).to_string()
}

pub(super) fn read_increment(path: &Path, offset: &Mutex<u64>) -> Vec<u8> {
    let file_size = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(_) => return Vec::new(),
    };
    let mut offset_guard = match offset.lock() {
        Ok(guard) => guard,
        Err(_) => return Vec::new(),
    };

    if file_size < *offset_guard {
        *offset_guard = 0;
    }
    if file_size == *offset_guard {
        return Vec::new();
    }

    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    if file.seek(std::io::SeekFrom::Start(*offset_guard)).is_err() {
        return Vec::new();
    }

    let mut buffer = Vec::new();
    if file
        .take(file_size - *offset_guard)
        .read_to_end(&mut buffer)
        .is_err()
    {
        return Vec::new();
    }

    *offset_guard = file_size;
    buffer
}
