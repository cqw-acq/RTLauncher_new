use anyhow::{anyhow, Context, Result};
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Semaphore};

pub const MAX_CONCURRENT_FILES: usize = 24;
pub const THROTTLE_MS_AFTER_FILE: u64 = 0;
pub const MAX_TOTAL_CONNECTIONS: usize = 32;
const SMART_INITIAL_CONCURRENCY: usize = 5;
const SMART_MIN_CONCURRENCY: usize = 2;
const SMART_MAX_CONCURRENCY: usize = 12;
const SMART_ADJUST_BATCH: usize = 8;
const CHUNKED_THRESHOLD: u64 = 256 * 1024;
const SMALL_CHUNK: u64 = 256 * 1024;
const MEDIUM_CHUNK: u64 = 1 * 1024 * 1024;
const LARGE_CHUNK: u64 = 4 * 1024 * 1024;
const MIN_WORKERS_PER_FILE: usize = 1;
const MAX_WORKERS_PER_FILE: usize = 1;
const MAX_TOTAL_WORKERS_PER_FILE: usize = 1;
const LAST_MILE_THRESHOLD: f64 = 0.5;
const FINAL_SPRINT_THRESHOLD: f64 = 0.8;
const STALL_BYTES_PER_SEC: u64 = 30 * 1024;
const STALL_DETECTION_INTERVAL: Duration = Duration::from_secs(2);
/// 单个文件在所有下载源之间累计的最大尝试次数（外层按 URL 轮换）
const MAX_DOWNLOAD_ATTEMPTS: u32 = 8;
/// 同一个 URL 最多被完整尝试几次；有多个镜像时优先切换镜像，而不是反复打同一个源
const MAX_ATTEMPTS_PER_URL: u32 = 3;
/// 单个分块最多请求几次（含首次）；用完后该下载源判定失败，交给外层切换镜像或报错
const MAX_ATTEMPTS_PER_CHUNK: u32 = 5;
/// 失败重试的指数退避：第 n 次失败后等待 base * 2^(n-1)，且不超过上限
#[cfg(not(test))]
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(500);
#[cfg(not(test))]
const RETRY_BACKOFF_MAX: Duration = Duration::from_secs(8);
// 单元测试里缩短退避，避免测试本身变慢；逻辑与正式值一致
#[cfg(test)]
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(10);
#[cfg(test)]
const RETRY_BACKOFF_MAX: Duration = Duration::from_millis(80);
const CHUNK_TIMEOUT_BASE: Duration = Duration::from_secs(15);
const OVERALL_TIMEOUT_SECONDS: u64 = 600;

fn workers_for_size(_size: u64) -> usize {
    // 每个文件内部只开 1 个 worker，串行下载
    // 并发由外层 smart_batch_download 管理（文件级并发）
    1
}

fn chunk_size_for(size: u64) -> u64 {
    if size < 2 * 1024 * 1024 {
        SMALL_CHUNK
    } else if size < 20 * 1024 * 1024 {
        MEDIUM_CHUNK
    } else {
        LARGE_CHUNK
    }
}

/// 带上限的指数退避：第 `failures` 次失败后应等待的时间。
/// failures = 0 表示还没失败过，不需要等待。
fn retry_backoff(failures: u32) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let shift = (failures - 1).min(16);
    RETRY_BACKOFF_BASE
        .saturating_mul(1u32 << shift)
        .min(RETRY_BACKOFF_MAX)
}

/// 解析 `Retry-After` 响应头（仅支持秒数形式）
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// 分块请求收到了非成功的 HTTP 状态码（如 429 / 5xx）
#[derive(Debug)]
struct ChunkHttpStatusError {
    status: reqwest::StatusCode,
    retry_after: Option<Duration>,
    range: Range,
}

impl std::fmt::Display for ChunkHttpStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HTTP {} (bytes {}-{})",
            self.status, self.range.start, self.range.end
        )
    }
}

impl std::error::Error for ChunkHttpStatusError {}

/// 某个下载源已经在内部按上限重试过（例如分块重试次数用完），
/// 外层不应再对同一个 URL 整轮重试，只应切换到其他镜像或直接报错。
#[derive(Debug)]
struct SourceRetriesExhausted(String);

impl std::fmt::Display for SourceRetriesExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SourceRetriesExhausted {}

/// 外层按下载源的重试计划：
/// - 在多个 URL（镜像）之间轮换，换到一个还没试过的源时不等待；
/// - 同一个 URL 最多尝试 `max_per_url` 次，再次尝试前按指数退避等待；
/// - 被标记为“已耗尽”的 URL 不再尝试；
/// - 所有 URL 累计最多尝试 `max_total` 次。
struct UrlRetryPlan {
    attempts: Vec<u32>,
    exhausted: Vec<bool>,
    cursor: usize,
    total: u32,
    max_per_url: u32,
    max_total: u32,
}

impl UrlRetryPlan {
    fn new(url_count: usize, max_per_url: u32, max_total: u32) -> Self {
        Self {
            attempts: vec![0; url_count],
            exhausted: vec![false; url_count],
            cursor: 0,
            total: 0,
            max_per_url,
            max_total,
        }
    }

    /// 返回下一次要尝试的 URL 下标，以及尝试前需要等待的退避时间；没有可尝试的源时返回 None。
    fn next(&mut self) -> Option<(usize, Duration)> {
        let n = self.attempts.len();
        if n == 0 || self.total >= self.max_total {
            return None;
        }
        for step in 0..n {
            let i = (self.cursor + step) % n;
            if self.exhausted[i] || self.attempts[i] >= self.max_per_url {
                continue;
            }
            let delay = retry_backoff(self.attempts[i]);
            self.attempts[i] += 1;
            self.total += 1;
            self.cursor = (i + 1) % n;
            return Some((i, delay));
        }
        None
    }

    /// 该 URL 已在内部重试到上限，之后不再尝试
    fn mark_exhausted(&mut self, index: usize) {
        if let Some(flag) = self.exhausted.get_mut(index) {
            *flag = true;
        }
    }
}

/// 可被取消的等待；返回 false 表示等待期间被取消
async fn sleep_unless_cancelled(delay: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + delay;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let now = Instant::now();
        if now >= deadline {
            return true;
        }
        tokio::time::sleep((deadline - now).min(Duration::from_millis(100))).await;
    }
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub file_name: String,
    pub target_dir: PathBuf,
    pub urls: Vec<String>,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadFailure {
    pub file_name: String,
    pub error: String,
    pub urls_tried: Vec<String>,
}

#[derive(Debug)]
pub struct DownloadResult {
    pub success_count: usize,
    pub failures: Vec<DownloadFailure>,
}

#[derive(Debug)]
pub enum SingleDownloadResult {
    Success {
        path: PathBuf,
        used_url: String,
        sha1: Option<String>,
        size: u64,
    },
    Failed {
        error: String,
        urls_tried: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
struct Range {
    start: u64,
    end: u64,
}
impl Range {
    fn size(&self) -> u64 {
        self.end.saturating_sub(self.start) + 1
    }
}

struct ChunkTask {
    range: Range,
    started_at: Option<Instant>,
    last_progress_at: Option<Instant>,
    downloaded_in_chunk: Arc<AtomicU64>,
    /// 该分块已经失败过的次数；重试时沿用并递增，不会被清零
    attempts: u32,
    is_rescue: bool,
    /// 处于重试队列时，最早可以重新分配的时间（指数退避）
    retry_at: Option<Instant>,
}

impl Clone for ChunkTask {
    fn clone(&self) -> Self {
        Self {
            range: self.range,
            started_at: self.started_at,
            last_progress_at: self.last_progress_at,
            downloaded_in_chunk: Arc::new(AtomicU64::new(
                self.downloaded_in_chunk.load(Ordering::Relaxed),
            )),
            attempts: self.attempts,
            is_rescue: self.is_rescue,
            retry_at: self.retry_at,
        }
    }
}

/// 分块失败后调度器给出的处理结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkRetryDecision {
    /// 已放入重试队列，`delay` 之后才会被重新分配
    Retry { attempts: u32, delay: Duration },
    /// 已达到 `MAX_ATTEMPTS_PER_CHUNK`，该下载源应当判定失败
    Exhausted { attempts: u32 },
}

struct SharedDownloadState {
    total_size: u64,
    total_downloaded: Arc<AtomicU64>,
    last_mile_enabled: AtomicBool,
    failed: AtomicBool,
    /// 失败原因是分块重试次数用完（而不是取消 / 超时等）
    retries_exhausted: AtomicBool,
    error_msg: StdMutex<Option<String>>,
    cancel: Arc<AtomicBool>,
}

struct DynamicScheduler {
    alloc_cursor: u64,
    total_size: u64,
    base_chunk: u64,
    in_flight: VecDeque<ChunkTask>,
    /// 失败后等待重试的分块（保留 attempts，按 retry_at 退避）
    retry_queue: VecDeque<ChunkTask>,
    completed_offsets: VecDeque<(u64, u64)>,
    total_downloaded: Arc<AtomicU64>,
    last_mile_enabled: bool,
    throughput_samples: VecDeque<(Instant, u64)>,
    last_measured_bytes: u64,
    current_throughput: u64,
}

impl DynamicScheduler {
    fn new(total_size: u64, total_downloaded: Arc<AtomicU64>, base_chunk: u64) -> Self {
        Self {
            alloc_cursor: 0,
            total_size,
            base_chunk,
            in_flight: VecDeque::new(),
            retry_queue: VecDeque::new(),
            completed_offsets: VecDeque::new(),
            total_downloaded,
            last_mile_enabled: false,
            throughput_samples: VecDeque::new(),
            last_measured_bytes: 0,
            current_throughput: 0,
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let current = self.total_downloaded.load(Ordering::Relaxed);
        let delta = current.saturating_sub(self.last_measured_bytes);
        self.throughput_samples.push_back((now, delta));
        self.last_measured_bytes = current;
        while let Some(&(t, _)) = self.throughput_samples.front() {
            if now.duration_since(t) > Duration::from_secs(8) {
                self.throughput_samples.pop_front();
            } else {
                break;
            }
        }
        if self.throughput_samples.len() >= 2 {
            let window_bytes: u64 = self.throughput_samples.iter().map(|(_, b)| *b).sum();
            let first_t = self
                .throughput_samples
                .front()
                .map(|(t, _)| *t)
                .unwrap_or(now);
            let last_t = self
                .throughput_samples
                .back()
                .map(|(t, _)| *t)
                .unwrap_or(now);
            let elapsed = last_t.duration_since(first_t).as_secs_f64().max(0.5);
            self.current_throughput = (window_bytes as f64 / elapsed) as u64;
        }
        let progress_ratio = if self.total_size > 0 {
            current as f64 / self.total_size as f64
        } else {
            0.0
        };
        if progress_ratio > LAST_MILE_THRESHOLD && !self.last_mile_enabled {
            self.last_mile_enabled = true;
        }
    }

    fn alloc_next(&mut self) -> Option<ChunkTask> {
        // 失败的分块优先重试：沿用原来的 attempts（不清零），并且要等退避时间到了才重新分配
        if !self.retry_queue.is_empty() {
            let now = Instant::now();
            let ready = self
                .retry_queue
                .iter()
                .position(|c| c.retry_at.map_or(true, |t| t <= now));
            return match ready.and_then(|pos| self.retry_queue.remove(pos)) {
                Some(mut task) => {
                    task.started_at = Some(now);
                    task.last_progress_at = Some(now);
                    task.downloaded_in_chunk = Arc::new(AtomicU64::new(0));
                    task.retry_at = None;
                    self.in_flight.push_back(task.clone());
                    Some(task)
                }
                // 还有分块在退避中：暂停分配新分块，避免源站出错时换个分块继续高频请求
                None => None,
            };
        }
        if self.alloc_cursor < self.total_size {
            let remaining = self.total_size - self.alloc_cursor;
            let mut target_chunk = self.base_chunk;
            if self.last_mile_enabled {
                target_chunk = target_chunk.min(192 * 1024);
                if remaining < 5 * 1024 * 1024 {
                    target_chunk = target_chunk.min(128 * 1024);
                }
                if remaining < 1 * 1024 * 1024 {
                    target_chunk = target_chunk.min(64 * 1024);
                }
            }
            target_chunk = target_chunk.max(32 * 1024);
            let end = (self.alloc_cursor + target_chunk - 1).min(self.total_size - 1);
            let range = Range {
                start: self.alloc_cursor,
                end,
            };
            self.alloc_cursor = end + 1;
            let task = ChunkTask {
                range,
                started_at: Some(Instant::now()),
                last_progress_at: Some(Instant::now()),
                downloaded_in_chunk: Arc::new(AtomicU64::new(0)),
                attempts: 0,
                is_rescue: false,
                retry_at: None,
            };
            self.in_flight.push_back(task.clone());
            return Some(task);
        }
        self.try_rescue_stalled_chunk()
    }

    fn try_rescue_stalled_chunk(&mut self) -> Option<ChunkTask> {
        let now = Instant::now();
        let mut best_idx: Option<usize> = None;
        let mut best_slow: Duration = Duration::from_secs(0);
        for (i, c) in self.in_flight.iter().enumerate() {
            let elapsed = c.started_at.map(|t| t.elapsed()).unwrap_or_default();
            let download = c.downloaded_in_chunk.load(Ordering::Relaxed);
            let remain = c.range.size().saturating_sub(download);
            if remain < 16 * 1024 {
                continue;
            }
            let remain_fraction = if download == 0 {
                if elapsed > STALL_DETECTION_INTERVAL {
                    elapsed
                } else {
                    continue;
                }
            } else {
                let avg_bps = (download as f64 / elapsed.as_secs_f64().max(0.1)) as u64;
                let threshold = if self.last_mile_enabled {
                    STALL_BYTES_PER_SEC * 3 / 2
                } else {
                    STALL_BYTES_PER_SEC
                };
                if elapsed > STALL_DETECTION_INTERVAL && avg_bps < threshold {
                    elapsed
                } else {
                    continue;
                }
            };
            if remain_fraction > best_slow {
                best_slow = remain_fraction;
                best_idx = Some(i);
            }
        }
        if let Some(idx) = best_idx {
            let c = &self.in_flight[idx];
            let orig_range = c.range;
            let orig_attempts = c.attempts;
            let current_downloaded = c.downloaded_in_chunk.load(Ordering::Relaxed);
            let new_start = orig_range
                .start
                .saturating_add(current_downloaded.max(orig_range.size() / 3));
            if new_start >= orig_range.end {
                return None;
            }
            let new_chunk = ChunkTask {
                range: Range {
                    start: new_start,
                    end: orig_range.end,
                },
                started_at: Some(Instant::now()),
                last_progress_at: Some(Instant::now()),
                downloaded_in_chunk: Arc::new(AtomicU64::new(0)),
                // 拆出来的救援分块继承原分块的失败次数，避免借拆分把重试计数清零
                attempts: orig_attempts,
                is_rescue: true,
                retry_at: None,
            };
            self.in_flight[idx].range = Range {
                start: orig_range.start,
                end: new_start.saturating_sub(1),
            };
            let out = new_chunk.clone();
            self.in_flight.push_back(new_chunk);
            let _ = now;
            return Some(out);
        }
        None
    }

    fn mark_done(&mut self, start: u64, end: u64) {
        self.in_flight
            .retain(|c| !(c.range.start == start && c.range.end == end));
        if start == self.alloc_cursor {
            self.alloc_cursor = end + 1;
        }
        self.completed_offsets.push_back((start, end));
        if self.completed_offsets.len() > 64 {
            self.completed_offsets.pop_front();
        }
    }

    /// 记录一次分块失败。
    ///
    /// 失败次数沿用该分块已有的 attempts 再 +1（旧实现会回退 alloc_cursor，
    /// 导致同一范围被当作 attempts = 0 的新分块重新分配，重试上限永远触发不了）。
    /// 未达上限：放入重试队列，按指数退避（参考 Retry-After，但不超过上限）后再分配；
    /// 达到上限：返回 Exhausted，由调用方让整个下载源失败。
    fn mark_chunk_failed(
        &mut self,
        chunk: &ChunkTask,
        retry_after: Option<Duration>,
    ) -> ChunkRetryDecision {
        let range = chunk.range;
        let tracked_attempts = self
            .in_flight
            .iter()
            .position(|c| c.range.start == range.start && c.range.end == range.end)
            .and_then(|pos| self.in_flight.remove(pos))
            .map(|c| c.attempts)
            .unwrap_or(0);
        let attempts = tracked_attempts.max(chunk.attempts).saturating_add(1);
        if attempts >= MAX_ATTEMPTS_PER_CHUNK {
            return ChunkRetryDecision::Exhausted { attempts };
        }
        let delay = retry_backoff(attempts)
            .max(retry_after.unwrap_or(Duration::ZERO))
            .min(RETRY_BACKOFF_MAX);
        self.retry_queue.push_back(ChunkTask {
            range,
            started_at: None,
            last_progress_at: None,
            downloaded_in_chunk: Arc::new(AtomicU64::new(0)),
            attempts,
            is_rescue: false,
            retry_at: Some(Instant::now() + delay),
        });
        ChunkRetryDecision::Retry { attempts, delay }
    }

    /// 距离最早一个待重试分块可以重新分配还要等多久；没有待重试分块时返回 None
    fn next_retry_wait(&self) -> Option<Duration> {
        let now = Instant::now();
        self.retry_queue
            .iter()
            .map(|c| {
                c.retry_at
                    .map_or(Duration::ZERO, |t| t.saturating_duration_since(now))
            })
            .min()
    }

    fn is_done(&self) -> bool {
        self.alloc_cursor >= self.total_size
            && self.in_flight.is_empty()
            && self.retry_queue.is_empty()
    }

    fn remaining_ratio(&self) -> f64 {
        if self.total_size == 0 {
            return 0.0;
        }
        let done = self.total_downloaded.load(Ordering::Relaxed);
        1.0 - (done as f64 / self.total_size as f64)
    }

    fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }
}

struct TempFileGuard {
    temp_path: PathBuf,
    target_path: PathBuf,
    cancel: Arc<AtomicBool>,
    armed: bool,
}

impl TempFileGuard {
    fn new(temp_path: PathBuf, target_path: PathBuf, cancel: Arc<AtomicBool>) -> Self {
        Self {
            temp_path,
            target_path,
            cancel,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    fn ensure_cleanup(&self) {
        if self.temp_path.exists() {
            let _ = fs::remove_file(&self.temp_path);
        }
        let part_path = self.target_path.with_extension(format!(
            "{}.part",
            self.target_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("download")
        ));
        if part_path.exists() {
            let _ = fs::remove_file(&part_path);
        }
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.armed {
            self.cancel.store(true, Ordering::SeqCst);
            self.ensure_cleanup();
        }
    }
}

fn generate_temp_path(target: &Path) -> PathBuf {
    let base_name = target
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".downloading_{}", base_name))
}

fn verify_sha1(path: &Path, expected: &str) -> bool {
    match fs::read(path) {
        Ok(data) => {
            let mut hasher = Sha1::new();
            hasher.update(&data);
            let result = hasher.finalize();
            hex::encode(result).to_lowercase() == expected.to_lowercase()
        }
        Err(_) => false,
    }
}

pub async fn download_file(
    task: &DownloadTask,
    progress_tx: Option<mpsc::Sender<(u64, u64)>>,
    cancel: Option<Arc<AtomicBool>>,
) -> SingleDownloadResult {
    let total_start = Instant::now();
    let overall_timeout = Duration::from_secs(OVERALL_TIMEOUT_SECONDS);
    let file_name_for_log = if task.file_name.is_empty() {
        task.urls.first().cloned().unwrap_or_default()
    } else {
        task.file_name.clone()
    };
    
    println!(
        "[Download] ═══ 开始下载: {} | URL数: {} | 整体超时: {}s ═══",
        file_name_for_log,
        task.urls.len(),
        overall_timeout.as_secs()
    );
    
    let task_clone = task.clone();
    let cancel_clone = cancel.clone();
    
    let result = tokio::time::timeout(overall_timeout, async move {
        download_file_internal(&task_clone, progress_tx, cancel_clone).await
    })
    .await;
    
    match result {
        Ok(r) => {
            println!(
                "[Download] ═══ 结束: {} | 总耗时: {:.1}s ═══",
                file_name_for_log,
                total_start.elapsed().as_secs_f64()
            );
            r
        }
        Err(_) => {
            println!(
                "[Download] ═══ 超时: {} | 已等待超过 {}s, 清理残留文件 ═══",
                file_name_for_log,
                overall_timeout.as_secs()
            );
            
            let target = task.target_dir.join(if task.file_name.is_empty() {
                "unknown.bin"
            } else {
                &task.file_name
            });
            let temp_path = generate_temp_path(&target);
            let _ = fs::remove_file(&temp_path);
            let part_path = target.with_extension(format!(
                "{}.part",
                target
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("download")
            ));
            let _ = fs::remove_file(&part_path);
            
            SingleDownloadResult::Failed {
                error: format!("下载超时 ({}s, 速度太慢)", overall_timeout.as_secs()),
                urls_tried: task.urls.clone(),
            }
        }
    }
}

async fn download_file_internal(
    task: &DownloadTask,
    progress_tx: Option<mpsc::Sender<(u64, u64)>>,
    cancel: Option<Arc<AtomicBool>>,
) -> SingleDownloadResult {
    let mut resolved_file_name = task.file_name.clone();
    let mut resolved_sha1 = task.sha1.clone();
    let mut url_pool = task.urls.clone();
    
    if resolved_file_name.is_empty() {
        let first_url = task.urls.first().cloned().unwrap_or_default();
        println!("[Download] 文件名为空, 尝试识别: first_url={}", first_url);
        
        if is_cf_api_url(&first_url) {
            if let Some((pid, fid)) = extract_cf_ids(&first_url) {
                println!(
                    "[Download] 识别为 CurseForge: pid={}, fid={}, 开始解析真实文件名...",
                    pid, fid
                );

                if let Some((cf_file_name, cf_sha1, cdn_url)) =
                    resolve_cf_download_info(pid, fid).await
                {
                    resolved_file_name = cf_file_name;
                    if resolved_sha1.is_none() {
                        resolved_sha1 = cf_sha1;
                    }
                    if !cdn_url.is_empty() {
                        url_pool.insert(0, cdn_url.clone());
                    }
                    println!(
                        "[Download] CurseForge 解析完成: 文件名={}, CDN={}",
                        resolved_file_name, cdn_url
                    );
                } else {
                    // 兜底: 使用 format!("mod_{}.jar", fid)
                    let first4 = fid / 1000;
                    let last3 = fid % 1000;
                    let fallback_cdn = format!(
                        "https://edge.forgecdn.net/files/{}/{:03}/mod.jar",
                        first4, last3
                    );
                    url_pool.insert(0, fallback_cdn);
                    resolved_file_name = format!("mod_{}.jar", fid);
                    println!(
                        "[Download] CurseForge 解析失败, 使用兜底文件名: {}",
                        resolved_file_name
                    );
                }
            }
        }
        
        if resolved_file_name.is_empty() {
            if let Ok(url_obj) = reqwest::Url::parse(&first_url) {
                if let Some(last_segment) = url_obj.path_segments().and_then(|s| s.last()) {
                    if !last_segment.is_empty()
                        && last_segment != "file"
                        && last_segment != "download"
                        && !last_segment.starts_with("file.")
                    {
                        resolved_file_name = last_segment.to_string();
                    }
                }
            }
        }

        if resolved_file_name.is_empty() {
            resolved_file_name = format!(
                "file_{}.bin",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            );
        }
    }
    
    let target = task.target_dir.join(&resolved_file_name);
    let cancel_arc = cancel.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    
    println!(
        "[Download] 开始: {} | 目标: {} | URL数: {}",
        resolved_file_name,
        target.display(),
        url_pool.len()
    );
    for (i, u) in url_pool.iter().enumerate() {
        println!("[Download]   [{}] {}", i, u);
    }
    
    if let Err(e) = fs::create_dir_all(&task.target_dir) {
        println!("[Download] ✗ 创建目录失败: {}", e);
        return SingleDownloadResult::Failed {
            error: format!("创建目录失败: {}", e),
            urls_tried: Vec::new(),
        };
    }
    
    if target.exists() {
        if let Some(sha) = &resolved_sha1 {
            if verify_sha1(&target, sha) {
                let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                println!(
                    "[Download] ✓ 已存在 (SHA1匹配): {} ({} bytes)",
                    resolved_file_name, size
                );
                if let Some(tx) = progress_tx.as_ref() {
                    let _ = tx.send((size, size)).await;
                }
                return SingleDownloadResult::Success {
                    path: target,
                    used_url: "(已存在)".to_string(),
                    sha1: Some(sha.clone()),
                    size,
                };
            }
            println!(
                "[Download] ! 文件存在但SHA1不匹配, 重新下载: {}",
                resolved_file_name
            );
            let _ = fs::remove_file(&target);
        } else {
            let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
            println!(
                "[Download] ✓ 已存在 (无SHA1校验): {} ({} bytes)",
                resolved_file_name, size
            );
            if let Some(tx) = progress_tx.as_ref() {
                let _ = tx.send((size, size)).await;
            }
            return SingleDownloadResult::Success {
                path: target,
                used_url: "(已存在)".to_string(),
                sha1: resolved_sha1.clone(),
                size,
            };
        }
    }
    
    // 走到这里说明必须真正下载了，先推送一次 0% 让前端显示进度条
    if let Some(tx) = progress_tx.as_ref() {
        let _ = tx.try_send((0, 0));
    }
    
    let temp_path = generate_temp_path(&target);
    let _guard = TempFileGuard::new(temp_path.clone(), target.clone(), cancel_arc.clone());
    
    let mut last_err: Option<String> = None;
    let mut urls_tried: Vec<String> = Vec::new();
    // 在多个下载源之间轮换；同一 URL 有次数上限且重试前指数退避，
    // 内部已重试到上限的 URL（分块重试用完）直接跳过，只切换镜像
    let mut plan = UrlRetryPlan::new(url_pool.len(), MAX_ATTEMPTS_PER_URL, MAX_DOWNLOAD_ATTEMPTS);
    let mut attempt: u32 = 0;

    while let Some((url_index, backoff)) = plan.next() {
        attempt += 1;
        let url = url_pool[url_index].clone();

        if !backoff.is_zero() {
            println!(
                "[Download] 等待 {}ms 后再次尝试同一下载源: {}",
                backoff.as_millis(),
                url
            );
        }
        if !sleep_unless_cancelled(backoff, &cancel_arc).await {
            println!("[Download] ✗ 已取消: {}", resolved_file_name);
            last_err = Some("已取消".to_string());
            break;
        }

        urls_tried.push(url.clone());

        println!(
            "[Download] [{}] 尝试URL[{}]: {} | 文件: {}",
            attempt,
            url_index,
            url,
            resolved_file_name
        );

        match try_download_to_temp(
            &url,
            &temp_path,
            &target,
            &resolved_sha1,
            progress_tx.clone(),
            cancel_arc.clone(),
        )
        .await
        {
            Ok(()) => {
                if let Err(e) = finalize_download(&temp_path, &target) {
                    last_err = Some(format!("文件最终化失败: {}", e));
                    println!("[Download] ✗ 文件最终化失败: {}", e);
                    let _ = fs::remove_file(&temp_path);
                    let part_path = target.with_extension(format!(
                        "{}.part",
                        target
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("download")
                    ));
                    let _ = fs::remove_file(&part_path);
                    continue;
                }
                
                let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                println!(
                    "[Download] ✓ 成功: {} ({} bytes) | URL: {}",
                    resolved_file_name, size, url
                );
                return SingleDownloadResult::Success {
                    path: target,
                    used_url: url,
                    sha1: resolved_sha1.clone(),
                    size,
                };
            }
            Err(e) => {
                last_err = Some(e.to_string());
                println!("[Download] ✗ 失败: {} | 错误: {}", resolved_file_name, e);
                if e.downcast_ref::<SourceRetriesExhausted>().is_some() {
                    plan.mark_exhausted(url_index);
                    println!(
                        "[Download] ! 该下载源已重试到上限, 不再重复尝试: {}",
                        url
                    );
                }
                let _ = fs::remove_file(&temp_path);
                let part_path = target.with_extension(format!(
                    "{}.part",
                    target
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("download")
                ));
                let _ = fs::remove_file(&part_path);
            }
        }
    }
    
    println!(
        "[Download] ✗ 全部URL失败: {} | 最后错误: {}",
        resolved_file_name,
        last_err.as_ref().unwrap_or(&String::new())
    );
    let _ = fs::remove_file(&temp_path);
    let part_path = target.with_extension(format!(
        "{}.part",
        target
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("download")
    ));
    let _ = fs::remove_file(&part_path);
    
    SingleDownloadResult::Failed {
        error: last_err.unwrap_or_else(|| "未知错误".to_string()),
        urls_tried,
    }
}

async fn try_download_to_temp(
    url: &str,
    temp_path: &Path,
    _target: &Path,
    sha1: &Option<String>,
    progress_tx: Option<mpsc::Sender<(u64, u64)>>,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let mut url_to_use = url.to_string();
    
    // CurseForge: 尝试多个 CDN 备用地址
    let mut curseforge_urls: Vec<String> = Vec::new();
    if url.contains("curseforge.com")
        || url.contains("edge.forgecdn.net")
        || url.contains("files-cf.curseforge.com")
    {
        if let Some((pid, fid)) = extract_cf_ids(url) {
            let first4 = fid / 1000;
            let last3 = fid % 1000;
            curseforge_urls.push(format!(
                "https://edge.forgecdn.net/files/{}/{:03}/",
                first4, last3
            ));
            curseforge_urls.push(format!(
                "https://files-cf.curseforge.com/file/curseforge-files/{}/{:03}/",
                first4, last3
            ));
            curseforge_urls.push(format!(
                "https://www.curseforge.com/minecraft/mc-mods/{}/download/{}/file",
                pid, fid
            ));
            curseforge_urls.push(format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                pid, fid
            ));
        }
    }
    
    // 如果是 CurseForge API URL，解析为 CDN 列表
    if url.contains("api.curseforge.com/v1/mods/") {
        if let Some((pid, fid)) = extract_cf_ids(url) {
            if let Some((_, _, cdn)) = resolve_cf_download_info(pid, fid).await {
                url_to_use = cdn.clone();
                let first4 = fid / 1000;
                let last3 = fid % 1000;
                curseforge_urls.push(cdn);
                curseforge_urls.push(format!(
                    "https://edge.forgecdn.net/files/{}/{:03}/",
                    first4, last3
                ));
                curseforge_urls.push(format!(
                    "https://files-cf.curseforge.com/file/curseforge-files/{}/{:03}/",
                    first4, last3
                ));
            }
        }
    }
    
    // 处理 Modrinth CDN 重定向
    if url_to_use.contains("cdn.modrinth.com")
        || url_to_use.contains("github.com")
        || url_to_use.contains("//cdn")
    {
        if let Ok(resolved) = crate::http_client::resolve_redirect_url(&url_to_use).await {
            if resolved != url_to_use {
                println!("[Download]   解析重定向: {} -> {}", url_to_use, resolved);
                url_to_use = resolved;
            }
        }
    }
    
    // Modrinth CDN
    if url_to_use.contains("cdn.modrinth.com") {
        println!("[Download]   使用浏览器风格下载 (Modrinth CDN)");
        return browser_style_download(
            &url_to_use,
            temp_path,
            sha1,
            progress_tx,
            cancel,
            "modrinth",
        )
        .await;
    }
    
    // CurseForge 多 CDN 容错：逐个尝试不同的 CDN 地址
    if !curseforge_urls.is_empty() {
        let mut last_err: Option<String> = None;
        for (i, cdn_url) in curseforge_urls.iter().enumerate() {
            println!("[Download]   尝试 CurseForge CDN [{}]: {}", i + 1, cdn_url);
            match browser_style_download(
                cdn_url,
                temp_path,
                sha1,
                progress_tx.clone(),
                cancel.clone(),
                "curseforge",
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e.to_string());
                    println!("[Download]   ✗ CurseForge CDN [{}] 失败: {}", i + 1, e);
                    let _ = std::fs::remove_file(&temp_path);
                }
            }
        }
        return Err(anyhow!(
            "所有 CurseForge CDN 均失败: {}",
            last_err.unwrap_or_else(|| "未知错误".to_string())
        ));
    }
    
    if url_to_use.contains("curseforge.com") || url_to_use.contains("edge.forgecdn.net") {
        println!("[Download]   使用浏览器风格下载 (CurseForge)");
        return browser_style_download(
            &url_to_use,
            temp_path,
            sha1,
            progress_tx,
            cancel,
            "curseforge",
        )
        .await;
    }

    let is_maven_repo = url_to_use.contains("maven.neoforged.net/releases/")
        || url_to_use.contains("files.minecraftforge.net/maven/")
        || url_to_use.contains("bmclapi2.bangbang93.com/maven/")
        || url_to_use.contains("repo1.maven.org/maven2/")
        || url_to_use.contains("maven.fabricmc.net/");
    if is_maven_repo {
        println!("[Download]   使用单线程下载 (Maven 仓库)");
        let client = crate::http_client::shared_client().await;
        return single_threaded_download(&client, &url_to_use, temp_path, progress_tx, cancel)
            .await;
    }

    let client = crate::http_client::shared_client().await;
    let first_range_end = (MEDIUM_CHUNK - 1).min(u32::MAX as u64);
    
    println!(
        "[Download]   探测 Range请求: bytes=0-{} | URL: {}",
        first_range_end, url_to_use
    );
    let probe_resp = client
        .get(&url_to_use)
        .header(
            reqwest::header::RANGE,
            format!("bytes=0-{}", first_range_end),
        )
        .timeout(Duration::from_secs(10))
        .send()
        .await;
    
    let (total_size, mut first_chunk_data) = match probe_resp {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let final_url = resp.url().to_string();
            println!(
                "[Download]   探测响应: HTTP {} | 最终URL: {}",
                status, final_url
            );
            
            if status == 206 {
                let size = resp
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split('/').nth(1))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(resp.content_length().unwrap_or(0));
                let body = resp.bytes().await.unwrap_or_default();
                println!("[Download]   ✓ 支持Range (HTTP 206) | 文件大小: {} bytes | first_chunk: {} bytes", size, body.len());
                
                if size > 0 && body.is_empty() {
                    println!("[Download]   ! Range请求返回空body，回退到单线程下载");
                    return single_threaded_download(
                        &client,
                        &url_to_use,
                        temp_path,
                        progress_tx,
                        cancel,
                    )
                    .await;
                }
                (size, Some(body.to_vec()))
            } else if status == 200 {
                let size = resp.content_length().unwrap_or(0);
                println!(
                    "[Download]   ! 不支持Range (HTTP 200) | 文件大小: {} bytes | 改用单线程下载",
                    size
                );
                drop(resp);
                return single_threaded_download(
                    &client,
                    &url_to_use,
                    temp_path,
                    progress_tx,
                    cancel,
                )
                .await;
            } else if (300..400).contains(&status) {
                println!(
                    "[Download]   ! 重定向响应 (HTTP {})，改用单线程下载",
                    status
                );
                drop(resp);
                return single_threaded_download(
                    &client,
                    &url_to_use,
                    temp_path,
                    progress_tx,
                    cancel,
                )
                .await;
            } else {
                return Err(anyhow!("HTTP {}", status));
            }
        }
        Err(e) => {
            println!("[Download]   ✗ 探测请求失败: {} | 改用单线程下载", e);
            return single_threaded_download(&client, &url_to_use, temp_path, progress_tx, cancel)
                .await;
        }
    };
    
    if total_size == 0 {
        return Err(anyhow!("文件大小为 0，无法下载"));
    }
    
    if total_size <= first_range_end + 1 {
        if let Some(data) = first_chunk_data {
            if !data.is_empty() {
                println!("[Download]   小文件 ({} bytes), 直接写入", total_size);
                fs::write(temp_path, &data).with_context(|| "写入文件失败")?;
                
                if let Some(sha) = sha1 {
                    if !verify_sha1(temp_path, sha) {
                        let _ = fs::remove_file(temp_path);
                        return Err(anyhow!("SHA1 不匹配"));
                    }
                }
                return Ok(());
            }
        }
        println!("[Download]   走单线程下载");
        return single_threaded_download(&client, &url_to_use, temp_path, progress_tx, cancel)
            .await;
    }
    
    if total_size < CHUNKED_THRESHOLD {
        println!(
            "[Download]   文件小于 CHUNKED_THRESHOLD ({} bytes), 走单线程下载",
            total_size
        );
        return single_threaded_download(&client, &url_to_use, temp_path, progress_tx, cancel)
            .await;
    }
    
    if let Err(e) = preallocate_file(temp_path, total_size) {
        return Err(e.context("预分配文件失败"));
    }
    
    let bw = workers_for_size(total_size);
    println!(
        "[Download]   分片模式下载 | 总大小: {} bytes | base_workers: {}",
        total_size, bw
    );
    
    let (writer_tx, mut writer_rx): (
        mpsc::UnboundedSender<(u64, Vec<u8>)>,
        mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
    ) = mpsc::unbounded_channel();
    
    let first_chunk_len = first_chunk_data.as_ref().map(|d| d.len()).unwrap_or(0);
    if let Some(data) = first_chunk_data.take() {
        if !data.is_empty() {
            let _ = writer_tx.send((0, data));
        }
    }
    
    let temp_path_buf = temp_path.to_path_buf();
    let write_handle = tokio::spawn(async move {
        use tokio::io::AsyncSeekExt;
        let mut f = match tokio::fs::File::create(&temp_path_buf).await {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[Download]   ✗ 创建文件失败: {}", e);
                return;
            }
        };
        
        let mut buf: std::collections::BTreeMap<u64, Vec<u8>> = std::collections::BTreeMap::new();
        let mut expected: u64 = 0;
        let mut buffered_bytes: u64 = 0;
        let flush_threshold: u64 = 8 * 1024 * 1024;
        
        while let Some((offset, data)) = writer_rx.recv().await {
            let data_len = data.len() as u64;
            
            if offset == expected {
                let _ = f.seek(std::io::SeekFrom::Start(offset)).await;
                let _ = f.write_all(&data).await;
                expected = offset + data_len;
                buffered_bytes += data_len;
                
                while let Some(next_data) = buf.remove(&expected) {
                    let _ = f.seek(std::io::SeekFrom::Start(expected)).await;
                    let nd_len = next_data.len() as u64;
                    let _ = f.write_all(&next_data).await;
                    expected += nd_len;
                    buffered_bytes += nd_len;
                }
                
                if buffered_bytes >= flush_threshold {
                    let _ = f.flush().await;
                    buffered_bytes = 0;
                }
            } else {
                buffered_bytes += data_len;
                buf.insert(offset, data);
                
                if buffered_bytes >= flush_threshold {
                    let _ = f.flush().await;
                    buffered_bytes = 0;
                }
            }
        }
        
        let _ = f.flush().await;
        let _ = f.sync_all().await;
    });
    
    let total_downloaded = Arc::new(AtomicU64::new(0));
    if first_chunk_len > 0 {
        total_downloaded.fetch_add(first_chunk_len as u64, Ordering::Relaxed);
    }
    
    let state = Arc::new(SharedDownloadState {
        total_size,
        total_downloaded: total_downloaded.clone(),
        last_mile_enabled: AtomicBool::new(false),
        failed: AtomicBool::new(false),
        retries_exhausted: AtomicBool::new(false),
        error_msg: StdMutex::new(None),
        cancel: cancel.clone(),
    });
    
    let scheduler = Arc::new(tokio::sync::Mutex::new(DynamicScheduler::new(
        total_size,
        total_downloaded.clone(),
        chunk_size_for(total_size),
    )));
    
    if first_chunk_len > 0 {
        let mut s = scheduler.lock().await;
        s.mark_done(0, first_chunk_len as u64 - 1);
    }
    
    let base_workers = workers_for_size(total_size);
    
    let progress_monitor = {
        let state = state.clone();
        let scheduler = scheduler.clone();
        let tx = progress_tx.clone();
        
        tokio::spawn(async move {
            let mut last_report = Instant::now();
            loop {
                tokio::time::sleep(Duration::from_millis(200)).await;
                
                let done = state.total_downloaded.load(Ordering::Relaxed);
                if let Some(tx) = &tx {
                    let _ = tx.try_send((done, state.total_size));
                }
                
                if state.cancel.load(Ordering::SeqCst) || state.failed.load(Ordering::SeqCst) {
                    break;
                }
                
                let done_sched = {
                    let mut s = scheduler.lock().await;
                    s.tick();
                    s.is_done()
                };
                
                if done_sched
                    && Instant::now().duration_since(last_report) > Duration::from_millis(400)
                {
                    break;
                }
                
                last_report = Instant::now();
                
                if done >= state.total_size {
                    break;
                }
            }
        })
    };
    
    let url_arc = Arc::new(url_to_use.clone());
    let client_arc = client.clone();
    let writer_sender = writer_tx.clone();
    let mut worker_handles = Vec::new();
    
    for worker_id in 0..base_workers {
        let scheduler = scheduler.clone();
        let state = state.clone();
        let url = url_arc.clone();
        let client = client_arc.clone();
        let ws = writer_sender.clone();
        
        let handle = tokio::spawn(async move {
            worker_loop(worker_id, scheduler, state, url, client, ws).await;
        });
        worker_handles.push(handle);
    }
    
    let state_clone = state.clone();
    let scheduler_clone = scheduler.clone();
    let url_arc2 = url_arc.clone();
    let client_arc2 = client_arc.clone();
    let ws2 = writer_sender.clone();
    
    let accelerator_handle = tokio::spawn(async move {
        let mut last_add = Instant::now();
        let mut current_worker_count = base_workers as u64;
        let mut last_throughput_samples: VecDeque<u64> = VecDeque::new();
        let mut last_done: u64 = total_downloaded.load(Ordering::Relaxed);
        let mut last_time = Instant::now();
        
        loop {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            
            if state_clone.cancel.load(Ordering::SeqCst)
                || state_clone.failed.load(Ordering::SeqCst)
            {
                break;
            }
            
            let done = state_clone.total_downloaded.load(Ordering::SeqCst);
            if done >= state_clone.total_size {
                break;
            }
            
            let now = Instant::now();
            let dt = now.duration_since(last_time).as_secs_f64().max(0.1);
            let dd = done.saturating_sub(last_done);
            let current_bps = (dd as f64 / dt) as u64;
            
            last_throughput_samples.push_back(current_bps);
            if last_throughput_samples.len() > 4 {
                last_throughput_samples.pop_front();
            }
            
            let avg_bps = if !last_throughput_samples.is_empty() {
                last_throughput_samples.iter().sum::<u64>() / last_throughput_samples.len() as u64
            } else {
                0
            };
            
            last_done = done;
            last_time = now;
            
            let (in_flight, alloc_done, _remaining) = {
                let s = scheduler_clone.lock().await;
                (
                    s.in_flight_count(),
                    s.alloc_cursor >= s.total_size,
                    s.total_size
                        .saturating_sub(s.total_downloaded.load(Ordering::Relaxed)),
                )
            };
            
            let ratio = done as f64 / state_clone.total_size as f64;
            let mut should_spawn = false;
            let mut num_new: u64 = 0;
            
            // 早期阶段也主动增加 worker
            if ratio < 0.2 && in_flight > 0 {
                if current_worker_count < (MAX_TOTAL_WORKERS_PER_FILE as u64) / 2 {
                    num_new = 4;
                    should_spawn = true;
                }
            } else if ratio >= FINAL_SPRINT_THRESHOLD && in_flight > 0 {
                num_new = (in_flight as u64).min(24).max(8);
                should_spawn = true;
            } else if ratio >= LAST_MILE_THRESHOLD {
                if avg_bps < STALL_BYTES_PER_SEC * 6 && in_flight > 0 {
                    num_new = (in_flight as u64).min(16).max(6);
                    should_spawn = true;
                }
            } else if alloc_done && in_flight > 0 {
                if avg_bps < STALL_BYTES_PER_SEC * 6 {
                    num_new = (in_flight as u64).min(12).max(5);
                    should_spawn = true;
                }
            } else if avg_bps < STALL_BYTES_PER_SEC * 2 && in_flight > 0 {
                num_new = 5;
                should_spawn = true;
            }
            
            if should_spawn && last_add.elapsed() > Duration::from_secs(2) {
                let max_total = MAX_TOTAL_WORKERS_PER_FILE as u64;
                let can_add = max_total.saturating_sub(current_worker_count);
                let actual_add = num_new.min(can_add);
                
                if actual_add > 0 {
                    last_add = Instant::now();
                    current_worker_count += actual_add;
                    
                    for n in 0..actual_add {
                        let scheduler = scheduler_clone.clone();
                        let state = state_clone.clone();
                        let url = url_arc2.clone();
                        let client = client_arc2.clone();
                        let ws = ws2.clone();
                        let wid = 10000 + ((n as usize) * 7919 + current_worker_count as usize);
                        
                        tokio::spawn(async move {
                            worker_loop(wid, scheduler, state, url, client, ws).await;
                        });
                    }
                }
            }
        }
    });
    
    for h in worker_handles {
        let _ = h.await;
    }
    
    let _ = accelerator_handle.await;
    let _ = progress_monitor.await;
    
    drop(writer_sender);
    drop(writer_tx);
    let _ = write_handle.await;
    
    if state.failed.load(Ordering::SeqCst) {
        let msg = state
            .error_msg
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().cloned())
            .unwrap_or_else(|| "下载失败".to_string());
        if state.retries_exhausted.load(Ordering::SeqCst) {
            // 分块已在内部按上限重试过，告诉外层不要再对同一 URL 整轮重试
            return Err(anyhow::Error::new(SourceRetriesExhausted(msg)));
        }
        return Err(anyhow!(msg));
    }
    
    if state.cancel.load(Ordering::SeqCst) {
        return Err(anyhow!("已取消"));
    }
    
    let actual = state.total_downloaded.load(Ordering::Relaxed);
    if actual < state.total_size {
        return Err(anyhow!("下载未完成: {}/{} 字节", actual, state.total_size));
    }
    
    if let Some(sha) = sha1 {
        println!("[Download]   校验SHA1: {}", sha);
        if !verify_sha1(temp_path, sha) {
            return Err(anyhow!("SHA1 不匹配"));
        }
        println!("[Download]   ✓ SHA1匹配");
    }
    
    Ok(())
}

async fn worker_loop(
    _worker_id: usize,
    scheduler: Arc<tokio::sync::Mutex<DynamicScheduler>>,
    state: Arc<SharedDownloadState>,
    url: Arc<String>,
    client: Arc<reqwest::Client>,
    writer: mpsc::UnboundedSender<(u64, Vec<u8>)>,
) {
    let start = Instant::now();
    let hard_timeout = Duration::from_secs(120);
    let mut tick_counter: u64 = 0;
    
    loop {
        if state.cancel.load(Ordering::SeqCst) || state.failed.load(Ordering::SeqCst) {
            return;
        }
        
        if start.elapsed() > hard_timeout {
            state.failed.store(true, Ordering::SeqCst);
            let mut em = state.error_msg.lock().unwrap();
            *em = Some(format!("worker 整体超时 ({}s)", hard_timeout.as_secs()));
            return;
        }
        
        let maybe_chunk = {
            let mut s = scheduler.lock().await;
            s.alloc_next()
        };

        let chunk = match maybe_chunk {
            Some(c) => c,
            None => {
                let (done, retry_wait) = {
                    let s = scheduler.lock().await;
                    (s.is_done(), s.next_retry_wait())
                };
                if done {
                    return;
                }
                // 有分块在退避中时睡到可以重试为止；每轮最多 200ms，期间仍能及时响应取消 / 失败
                let wait = retry_wait
                    .unwrap_or(Duration::from_millis(200))
                    .clamp(Duration::from_millis(1), Duration::from_millis(200));
                tokio::time::sleep(wait).await;
                continue;
            }
        };

        let range = chunk.range;
        let chunk_start = Instant::now();
        let attempt = chunk.attempts;
        tick_counter += 1;
        
        if tick_counter % 5 == 0 {
            let elapsed = start.elapsed().as_secs();
            let done_bytes = state.total_downloaded.load(Ordering::Relaxed);
            let pct = done_bytes as f64 / state.total_size as f64 * 100.0;
            println!(
                "[Download]   worker 活跃: 已运行 {}s, 进度 {:.1}% ({}/{} bytes)",
                elapsed, pct, done_bytes, state.total_size
            );
        }
        
        match download_chunk(&client, &url, &writer, range, &state, &chunk).await {
            Ok(()) => {
                let mut s = scheduler.lock().await;
                s.mark_done(range.start, range.end);
                let elapsed = chunk_start.elapsed().as_secs_f64();
                println!(
                    "[Download]     ✓ chunk {}-{} 完成 (尝试 {}次, {:.1}s, {} bytes)",
                    range.start,
                    range.end,
                    attempt + 1,
                    elapsed,
                    range.size()
                );
            }
            Err(e) => {
                // 该分块已计入 total_downloaded 的字节并没有写盘，扣回去，避免重试后进度虚高
                let counted = chunk.downloaded_in_chunk.swap(0, Ordering::Relaxed);
                if counted > 0 {
                    let _ = state.total_downloaded.fetch_update(
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                        |v| Some(v.saturating_sub(counted)),
                    );
                }
                if state.cancel.load(Ordering::SeqCst) || state.failed.load(Ordering::SeqCst) {
                    // 已取消或其他 worker 已判定失败，不再记入重试
                    return;
                }
                let elapsed = chunk_start.elapsed().as_secs_f64();
                let retry_after = e
                    .downcast_ref::<ChunkHttpStatusError>()
                    .and_then(|h| h.retry_after);
                let decision = {
                    let mut s = scheduler.lock().await;
                    s.mark_chunk_failed(&chunk, retry_after)
                };
                match decision {
                    ChunkRetryDecision::Retry { attempts, delay } => {
                        println!(
                            "[Download]     ✗ chunk {}-{} 失败 (第 {}/{} 次, {:.1}s, 错误: {:#}), {}ms 后重试",
                            range.start,
                            range.end,
                            attempts,
                            MAX_ATTEMPTS_PER_CHUNK,
                            elapsed,
                            e,
                            delay.as_millis()
                        );
                    }
                    ChunkRetryDecision::Exhausted { attempts } => {
                        println!(
                            "[Download]     ✗ chunk {}-{} 失败 (第 {}/{} 次, {:.1}s, 错误: {:#}), 已达重试上限, 放弃该下载源",
                            range.start,
                            range.end,
                            attempts,
                            MAX_ATTEMPTS_PER_CHUNK,
                            elapsed,
                            e
                        );
                        if let Ok(mut em) = state.error_msg.lock() {
                            *em = Some(format!(
                                "分块 bytes {}-{} 连续失败 {} 次, 已达重试上限: {:#}",
                                range.start, range.end, attempts, e
                            ));
                        }
                        state.retries_exhausted.store(true, Ordering::SeqCst);
                        state.failed.store(true, Ordering::SeqCst);
                        return;
                    }
                }
            }
        }
    }
}

async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    writer: &mpsc::UnboundedSender<(u64, Vec<u8>)>,
    range: Range,
    state: &SharedDownloadState,
    chunk: &ChunkTask,
) -> Result<()> {
    let range_header = format!("bytes={}-{}", range.start, range.end);
    let size_mb = (range.size() as f64 / (1024.0 * 1024.0)).max(0.1);
    let req_timeout = Duration::from_secs_f64((30.0 + size_mb * 10.0).min(120.0));
    let req_start = Instant::now();
    
    let resp = client
        .get(url)
        .header(reqwest::header::RANGE, &range_header)
        .timeout(req_timeout)
        .send()
        .await
        .with_context(|| format!("请求 chunk {}-{} 失败", range.start, range.end))?;
    
    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return Err(anyhow::Error::new(ChunkHttpStatusError {
            status: resp.status(),
            retry_after: parse_retry_after(resp.headers()),
            range,
        }));
    }
    
    use futures::stream::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut offset = range.start;
    let downloaded_meter = chunk.downloaded_in_chunk.clone();
    downloaded_meter.store(0, Ordering::Relaxed);
    
    let mut buffer: Vec<u8> = Vec::with_capacity(range.size() as usize);
    let mut last_progress_time = Instant::now();
    let mut last_bytes_time = Instant::now();
    let mut last_offset: u64 = range.start;
    let read_timeout = Duration::from_secs(60);
    let mut error_count: u32 = 0;
    const MAX_ERRORS: u32 = 15;
    
    loop {
        if state.cancel.load(Ordering::SeqCst) {
            return Err(anyhow!("已取消"));
        }
        if state.failed.load(Ordering::SeqCst) {
            return Err(anyhow!("已失败"));
        }
        if req_start.elapsed() > Duration::from_secs(600) {
            return Err(anyhow!("chunk 整体超时"));
        }
        
        match tokio::time::timeout(read_timeout, stream.next()).await {
            Err(_) => {
                error_count += 1;
                if error_count > MAX_ERRORS {
                    return Err(anyhow!("chunk 读取超时 (连续 {} 次无数据)", MAX_ERRORS));
                }
                let sleep_ms = 200u64 + (error_count as u64) * 100;
                println!(
                    "[Download]     ! chunk {}-{} 读取超时 ({}/{}), 等待 {}ms...",
                    range.start, range.end, error_count, MAX_ERRORS, sleep_ms
                );
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                continue;
            }
            Ok(None) => break,
            Ok(Some(chunk_data)) => match chunk_data {
                Ok(data) => {
                    error_count = 0;
                    if data.is_empty() {
                        continue;
                    }
                    let data_len = data.len() as u64;
                    buffer.extend_from_slice(&data);
                    offset += data_len;
                    downloaded_meter.fetch_add(data_len, Ordering::Relaxed);
                    state
                        .total_downloaded
                        .fetch_add(data_len, Ordering::Relaxed);
                    if offset > last_offset {
                        last_offset = offset;
                        last_bytes_time = Instant::now();
                    } else if last_bytes_time.elapsed() > read_timeout * 2 {
                        return Err(anyhow!("chunk 下载停滞"));
                    }
                }
                Err(e) => {
                    error_count += 1;
                    if error_count > MAX_ERRORS {
                        return Err(anyhow!("chunk 读取错误 (连续 {} 次): {}", MAX_ERRORS, e));
                    }
                    let sleep_ms = 200u64 + (error_count as u64) * 100;
                    println!(
                        "[Download]     ! chunk {}-{} 错误 ({}/{}): {}, 等待 {}ms...",
                        range.start, range.end, error_count, MAX_ERRORS, e, sleep_ms
                    );
                    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                    continue;
                }
            },
        }
        
        if last_progress_time.elapsed() > Duration::from_millis(5000) {
            let total_done = state.total_downloaded.load(Ordering::Relaxed);
            let percent = total_done as f64 / state.total_size as f64 * 100.0;
            let elapsed = req_start.elapsed().as_secs_f64();
            let chunk_bytes = offset.saturating_sub(range.start);
            let chunk_pct = chunk_bytes as f64 / range.size() as f64 * 100.0;
            let speed_kb = chunk_bytes as f64 / 1024.0 / elapsed.max(0.1);
            println!("[Download]     ↓ 进度: {:.1}% ({}/{} bytes) | chunk {}-{}: {:.1}% (速度 {:.0} KB/s)",
                percent, total_done, state.total_size,
                range.start, range.end, chunk_pct, speed_kb);
            last_progress_time = Instant::now();
        }
        
        if state.last_mile_enabled.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    }
    
    if offset.saturating_sub(range.start) != range.size() {
        return Err(anyhow!(
            "chunk 不完整 (bytes {}-{}): 期望 {}, 实际 {}",
            range.start,
            range.end,
            range.size(),
            offset.saturating_sub(range.start)
        ));
    }
    
    let _ = writer.send((range.start, buffer));
    Ok(())
}

async fn single_threaded_download(
    client: &reqwest::Client,
    url: &str,
    temp_path: &Path,
    progress_tx: Option<mpsc::Sender<(u64, u64)>>,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    println!("[Download]   单线程下载: {}", url);
    let start = Instant::now();
    let hard_timeout = Duration::from_secs(600);
    
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("请求失败: {}", url))?;
    
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {}: {}", resp.status(), url));
    }
    
    let total_size = resp.content_length().unwrap_or(0);
    println!("[Download]   文件大小: {} bytes", total_size);
    
    // 立刻推送一次初始进度，让前端立刻显示进度条而不是等满 3 秒
    if let Some(ref tx) = progress_tx {
        let _ = tx.try_send((0, total_size));
    }
    
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .with_context(|| format!("创建文件失败: {}", temp_path.display()))?;
    
    use futures::stream::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut received: u64 = 0;
    let mut reporter_tick = Instant::now();
    let mut last_bytes_tick = Instant::now();
    let mut last_received: u64 = 0;
    let stall_timeout = Duration::from_secs(120);
    let mut chunk_error_count: u32 = 0;
    let mut chunk_timeout_count: u32 = 0;
    const MAX_CHUNK_ERRORS: u32 = 25;
    const MAX_CHUNK_TIMEOUTS: u32 = 40;
    const READ_TIMEOUT_SECS: u64 = 90;
    
    loop {
        let chunk_with_timeout =
            tokio::time::timeout(Duration::from_secs(READ_TIMEOUT_SECS), stream.next()).await;
        
        match chunk_with_timeout {
            Err(_) => {
                chunk_timeout_count += 1;
                if chunk_timeout_count > MAX_CHUNK_TIMEOUTS {
                    return Err(anyhow!("读取超时 (连续 {} 次无数据)", MAX_CHUNK_TIMEOUTS));
                }
                let sleep_ms = 300u64 + (chunk_timeout_count as u64) * 100;
                println!(
                    "[Download]   ! chunk {}s无响应 ({}/{}), 等待 {}ms 后继续...",
                    READ_TIMEOUT_SECS, chunk_timeout_count, MAX_CHUNK_TIMEOUTS, sleep_ms
                );
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                continue;
            }
            Ok(None) => break,
            Ok(Some(chunk_result)) => {
                if cancel.load(Ordering::SeqCst) {
                    return Err(anyhow!("已取消"));
                }
                
                if start.elapsed() > hard_timeout {
                    return Err(anyhow!("单线程下载整体超时"));
                }
                
                let data = match chunk_result {
                    Ok(d) => d,
                    Err(e) => {
                        chunk_error_count += 1;
                        if chunk_error_count > MAX_CHUNK_ERRORS {
                            return Err(anyhow!(
                                "读取字节流失败 (连续 {} 次错误): {}",
                                MAX_CHUNK_ERRORS,
                                e
                            ));
                        }
                        let sleep_ms = 300u64 + (chunk_error_count as u64) * 150;
                        println!(
                            "[Download]   ! chunk错误 ({}/{}): {}, 等待 {}ms 后继续...",
                            chunk_error_count, MAX_CHUNK_ERRORS, e, sleep_ms
                        );
                        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                        continue;
                    }
                };
                
                chunk_error_count = 0;
                chunk_timeout_count = 0;
                
                if data.is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
                
                file.write_all(&data).await.with_context(|| "写入失败")?;
                received += data.len() as u64;
                
                if reporter_tick.elapsed() > Duration::from_millis(500) {
                    if let Some(ref tx) = progress_tx {
                        let _ = tx.try_send((received, total_size));
                    }
                    
                    let pct = if total_size > 0 {
                        received as f64 / total_size as f64 * 100.0
                    } else {
                        0.0
                    };
                    let speed_kb =
                        received as f64 / 1024.0 / start.elapsed().as_secs_f64().max(0.1);
                    
                    println!(
                        "[Download]     ↓ 单线程进度: {:.1}% ({}/{} bytes) | 速度: {:.0} KB/s",
                        pct, received, total_size, speed_kb
                    );
                    
                    reporter_tick = Instant::now();
                }
                
                if received > last_received {
                    last_received = received;
                    last_bytes_tick = Instant::now();
                } else if last_bytes_tick.elapsed() > stall_timeout {
                    return Err(anyhow!("下载停滞超过 {}s", stall_timeout.as_secs()));
                }
            }
        }
    }
    
    file.flush().await.ok();
    drop(file);
    
    // 推送最终进度，保证至少到达 100% 一次
    if let Some(ref tx) = progress_tx {
        let _ = tx.try_send((received, total_size.max(received)));
    }
    
    if total_size > 0 && received != total_size {
        return Err(anyhow!(
            "大小不匹配: 期望 {}, 实际 {}",
            total_size,
            received
        ));
    }
    
    println!("[Download]   ✓ 单线程下载完成: {} bytes", received);
    Ok(())
}

async fn browser_style_download(
    url: &str,
    temp_path: &Path,
    sha1: &Option<String>,
    progress_tx: Option<mpsc::Sender<(u64, u64)>>,
    cancel: Arc<AtomicBool>,
    site_type: &str,
) -> Result<()> {
    println!("[Download]   浏览器风格下载: {} | 类型: {}", url, site_type);
    let start = Instant::now();
    let hard_timeout = Duration::from_secs(600);
    
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(30))
        .build()
        .with_context(|| "创建浏览器风格客户端失败")?;
    
    let mut request = client
        .get(url)
        .header("Accept", "*/*")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Accept-Encoding", "identity")
        .header("Connection", "keep-alive");
    
    request = match site_type {
        "curseforge" => request
            .header("Referer", "https://www.curseforge.com/")
            .header("Origin", "https://www.curseforge.com"),
        "modrinth" => request
            .header("Referer", "https://modrinth.com/")
            .header("Origin", "https://modrinth.com"),
        _ => request.header("Referer", "https://www.google.com/"),
    };
    
    let resp = request
        .send()
        .await
        .with_context(|| format!("请求失败: {}", url))?;
    
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {}: {}", resp.status(), url));
    }
    
    let total_size = resp.content_length().unwrap_or(0);
    println!("[Download]   文件大小: {} bytes", total_size);
    
    // 立刻推送一次初始进度
    if let Some(ref tx) = progress_tx {
        let _ = tx.try_send((0, total_size));
    }
    
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .with_context(|| format!("创建文件失败: {}", temp_path.display()))?;
    
    use futures::stream::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut received: u64 = 0;
    let mut reporter_tick = Instant::now();
    let mut last_bytes_tick = Instant::now();
    let mut last_received: u64 = 0;
    let stall_timeout = Duration::from_secs(120);
    let mut chunk_error_count: u32 = 0;
    let mut chunk_timeout_count: u32 = 0;
    const MAX_CHUNK_ERRORS: u32 = 30;
    const MAX_CHUNK_TIMEOUTS: u32 = 50;
    const READ_TIMEOUT_SECS: u64 = 120;
    
    loop {
        let chunk_with_timeout =
            tokio::time::timeout(Duration::from_secs(READ_TIMEOUT_SECS), stream.next()).await;
        
        match chunk_with_timeout {
            Err(_) => {
                chunk_timeout_count += 1;
                if chunk_timeout_count > MAX_CHUNK_TIMEOUTS {
                    return Err(anyhow!("读取超时 (连续 {} 次无数据)", MAX_CHUNK_TIMEOUTS));
                }
                let sleep_ms = 300u64 + (chunk_timeout_count as u64) * 100;
                println!(
                    "[Download]   ! chunk {}s无响应 ({}/{}), 等待 {}ms 后继续...",
                    READ_TIMEOUT_SECS, chunk_timeout_count, MAX_CHUNK_TIMEOUTS, sleep_ms
                );
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                continue;
            }
            Ok(None) => {
                // 流结束
                break;
            }
            Ok(Some(chunk_result)) => {
                if cancel.load(Ordering::SeqCst) {
                    return Err(anyhow!("已取消"));
                }
                
                if start.elapsed() > hard_timeout {
                    return Err(anyhow!("浏览器风格下载超时"));
                }
                
                let data = match chunk_result {
                    Ok(d) => d,
                    Err(e) => {
                        chunk_error_count += 1;
                        if chunk_error_count > MAX_CHUNK_ERRORS {
                            return Err(anyhow!(
                                "读取字节流失败 (连续 {} 次错误): {}",
                                MAX_CHUNK_ERRORS,
                                e
                            ));
                        }
                        let sleep_ms = 300u64 + (chunk_error_count as u64) * 150;
                        println!(
                            "[Download]   ! chunk错误 ({}/{}): {}, 等待 {}ms 后继续...",
                            chunk_error_count, MAX_CHUNK_ERRORS, e, sleep_ms
                        );
                        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                        continue;
                    }
                };
                
                chunk_error_count = 0;
                chunk_timeout_count = 0;
                
                if data.is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
                
                file.write_all(&data).await.with_context(|| "写入失败")?;
                received += data.len() as u64;
                
                if reporter_tick.elapsed() > Duration::from_millis(500) {
                    if let Some(ref tx) = progress_tx {
                        let _ = tx.try_send((received, total_size));
                    }
                    
                    let pct = if total_size > 0 {
                        received as f64 / total_size as f64 * 100.0
                    } else {
                        0.0
                    };
                    let speed_kb =
                        received as f64 / 1024.0 / start.elapsed().as_secs_f64().max(0.1);
                    
                    println!(
                        "[Download]     ↓ 浏览器风格进度: {:.1}% ({}/{} bytes) | 速度: {:.0} KB/s",
                        pct, received, total_size, speed_kb
                    );
                    
                    reporter_tick = Instant::now();
                }
                
                if received > last_received {
                    last_received = received;
                    last_bytes_tick = Instant::now();
                } else if last_bytes_tick.elapsed() > stall_timeout {
                    return Err(anyhow!("下载停滞超过 {}s", stall_timeout.as_secs()));
                }
            }
        }
    }
    
    file.flush().await.ok();
    drop(file);
    
    // 推送最终进度，保证至少到达 100% 一次
    if let Some(ref tx) = progress_tx {
        let _ = tx.try_send((received, total_size.max(received)));
    }
    
    if total_size > 0 && received != total_size {
        return Err(anyhow!(
            "大小不匹配: 期望 {}, 实际 {}",
            total_size,
            received
        ));
    }
    
    if let Some(sha) = sha1 {
        println!("[Download]   校验SHA1: {}", sha);
        if !verify_sha1(temp_path, sha) {
            return Err(anyhow!("SHA1 不匹配"));
        }
        println!("[Download]   ✓ SHA1匹配");
    }
    
    println!("[Download]   ✓ 浏览器风格下载完成: {} bytes", received);
    Ok(())
}

fn preallocate_file(path: &Path, size: u64) -> Result<()> {
    use std::fs::OpenOptions;
    let f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("创建文件失败: {}", path.display()))?;
    
    f.set_len(size)
        .with_context(|| format!("设置文件大小失败: {}", path.display()))?;
    
    f.sync_all().ok();
    drop(f);
    Ok(())
}

fn finalize_download(temp_path: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("创建目标目录失败"))?;
    }
    
    let part_path = target.with_extension(format!(
        "{}.part",
        target
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("download")
    ));
    
    if part_path.exists() {
        fs::remove_file(&part_path).ok();
    }
    
    fs::rename(temp_path, &part_path).with_context(|| format!("重命名到临时part文件失败"))?;
    
    if target.exists() {
        fs::remove_file(target).with_context(|| format!("删除已存在的目标文件失败"))?;
    }
    
    fs::rename(&part_path, target).with_context(|| format!("重命名part文件到最终目标失败"))?;
    Ok(())
}

fn rand_id() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0)
}

fn is_cf_api_url(url: &str) -> bool {
    url.contains("api.curseforge.com")
        || url.contains("www.curseforge.com")
        || url.contains("files.curseforge.com")
        || url.contains("files-cf.curseforge.com")
        || url.contains("edge.forgecdn.net")
}

fn extract_cf_ids(url: &str) -> Option<(u64, u64)> {
    // 匹配 api.curseforge.com/v1/mods/{pid}/files/{fid} 和
    // www.curseforge.com/minecraft/mc-mods/{pid}/.../files/{fid}/... 等
    let re1 = regex::Regex::new(r"/mods/(\d+)/files/(\d+)").ok()?;
    if let Some(caps) = re1.captures(url) {
        let pid = caps.get(1)?.as_str().parse().ok()?;
        let fid = caps.get(2)?.as_str().parse().ok()?;
        return Some((pid, fid));
    }

    // 匹配 www.curseforge.com/minecraft/mc-mods/{pid}/download/{fid}/file
    // （CurseForge 整合包生成的 URL 就是这种格式）
    let re2 = regex::Regex::new(r"/mc-mods/(\d+)/download/(\d+)").ok()?;
    if let Some(caps) = re2.captures(url) {
        let pid = caps.get(1)?.as_str().parse().ok()?;
        let fid = caps.get(2)?.as_str().parse().ok()?;
        return Some((pid, fid));
    }

    None
}

pub async fn resolve_cf_download_info(
    pid: u64,
    fid: u64,
) -> Option<(String, Option<String>, String)> {
    use serde_json::Value;
    let first4 = fid / 1000;
    let last3 = fid % 1000;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let api_url = format!("https://api.curseforge.com/v1/mods/{}/files/{}", pid, fid);
    let resp = match client
        .get(&api_url)
        .header(
            "x-api-key",
            "$2a$10$VTAFCxje5a1Jkqv0aGWjQ.fULedAEPctDqppOkNMRvV.edVnG7KQ6",
        )
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!(
                "[Download] CF-API 请求失败: pid={}, fid={}, error={}",
                pid, fid, e
            );
            return None;
        }
    };

    if !resp.status().is_success() {
        println!(
            "[Download] CF-API 返回状态 {}: pid={}, fid={}",
            resp.status(),
            pid,
            fid
        );
        return None;
    }

    let text = match resp.text().await {
        Ok(t) => t,
        Err(_) => return None,
    };

    let json = match serde_json::from_str::<Value>(&text) {
        Ok(j) => j,
        Err(_) => return None,
    };

    let is_valid = |name: &str| -> bool {
        !name.is_empty()
            && name != "file"
            && name != "download"
            && !name.starts_with("file.")
            && name.contains('.')
    };

    let file_name = json
        .get("data")
        .and_then(|d| d.get("fileName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| is_valid(s));

    let download_url = json
        .get("data")
        .and_then(|d| d.get("downloadUrl"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    match file_name {
        Some(name) => {
            let cdn = download_url.unwrap_or_else(|| {
                format!(
                    "https://edge.forgecdn.net/files/{}/{:03}/{}",
                    first4, last3, name
                )
            });
            println!("[Download] CF-API: pid={}, fid={}, name={}", pid, fid, name);
            Some((name, None, cdn))
        }
        None => None,
    }
}

pub async fn download_all_with_file_info(
    tasks: Vec<DownloadTask>,
    on_file_done: Option<mpsc::Sender<(usize, usize, String)>>,
) -> DownloadResult {
    let total = tasks.len();
    if total == 0 {
        return DownloadResult {
            success_count: 0,
            failures: Vec::new(),
        };
    }
    
    let done_counter = Arc::new(AtomicU64::new(0));
    
    let (success_count, failures) = smart_batch_download(
        tasks,
        total,
        done_counter.clone(),
        move |done, total_file_count, fname| {
            if let Some(ref tx) = on_file_done {
                let _ = tx.try_send((done, total_file_count, fname));
            }
        },
        Option::<fn(f64) -> ()>::None,
    )
    .await;
    
    DownloadResult {
        success_count,
        failures,
    }
}

pub async fn download_all(
    tasks: Vec<DownloadTask>,
    progress_tx: Option<mpsc::Sender<f64>>,
) -> DownloadResult {
    let total = tasks.len();
    if total == 0 {
        return DownloadResult {
            success_count: 0,
            failures: Vec::new(),
        };
    }

    let done_counter = Arc::new(AtomicU64::new(0));

    let (success_count, failures) = smart_batch_download(
        tasks,
        total,
        done_counter.clone(),
        |_done, _total, _fname| {},
        Some(move |percent| {
            if let Some(ref tx) = progress_tx {
                let _ = tx.try_send(percent);
            }
        }),
    )
    .await;

    DownloadResult {
        success_count,
        failures,
    }
}

pub async fn download_one(task: DownloadTask) -> Result<PathBuf> {
    match download_file(&task, None, None).await {
        SingleDownloadResult::Success { path, .. } => Ok(path),
        SingleDownloadResult::Failed { error, .. } => Err(anyhow!(error)),
    }
}

#[derive(Clone)]
struct AdaptiveSemaphore {
    sem: Arc<Semaphore>,
    completed: Arc<AtomicU64>,
    target_permits: Arc<AtomicU64>,
}

impl AdaptiveSemaphore {
    fn new(initial: usize) -> Self {
        Self {
            sem: Arc::new(Semaphore::new(initial.min(MAX_CONCURRENT_FILES))),
            completed: Arc::new(AtomicU64::new(0)),
            target_permits: Arc::new(AtomicU64::new(initial as u64)),
        }
    }
}

async fn spawn_adaptive_tuner(sem: AdaptiveSemaphore) {
    tokio::spawn(async move {
        let mut last_throughput: VecDeque<u64> = VecDeque::new();
        let mut last_time = Instant::now();
        let mut last_done: u64 = 0;
        let mut stable_high_count = 0usize;
        let mut low_count = 0usize;
        
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            let completed = sem.completed.load(Ordering::SeqCst);
            let target = sem.target_permits.load(Ordering::SeqCst) as usize;
            
            let now = Instant::now();
            let dt = now.duration_since(last_time).as_secs_f64().max(0.1);
            let dd = completed.saturating_sub(last_done);
            let current_bps = (dd as f64 / dt) as u64;
            
            last_throughput.push_back(current_bps);
            if last_throughput.len() > 5 {
                last_throughput.pop_front();
            }
            
            let avg_bps = if !last_throughput.is_empty() {
                last_throughput.iter().sum::<u64>() / last_throughput.len() as u64
            } else {
                0
            };
            
            last_done = completed;
            last_time = now;
            
            let current_permits = sem.sem.available_permits();
            let total_in_flight = target.saturating_sub(current_permits);
            
            // 吞吐量低时小幅降低并发（更温和）
            if avg_bps < STALL_BYTES_PER_SEC && total_in_flight > 0 {
                low_count += 1;
                if low_count >= 3 {
                    let new_target = (target as f64 * 0.85).max(4.0) as usize;
                    sem.target_permits
                        .store(new_target as u64, Ordering::SeqCst);
                    low_count = 0;
                }
            } else if avg_bps > STALL_BYTES_PER_SEC * 5 && current_permits == 0 {
                // 高吞吐量且满负载时，更激进地增加并发
                stable_high_count += 1;
                if stable_high_count >= 1 {
                    let multiplier = if avg_bps > STALL_BYTES_PER_SEC * 20 {
                        1.5
                    } else if avg_bps > STALL_BYTES_PER_SEC * 10 {
                        1.4
                    } else {
                        1.25
                    };
                    let new_target =
                        (target as f64 * multiplier).min(MAX_CONCURRENT_FILES as f64) as usize;
                    if new_target > target {
                        sem.target_permits
                            .store(new_target as u64, Ordering::SeqCst);
                    }
                    stable_high_count = 0;
                }
            } else {
                stable_high_count = 0;
                low_count = 0;
            }
        }
    });
}

fn global_connection_semaphore() -> Arc<Semaphore> {
    static SEM: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
    SEM.get_or_init(|| Arc::new(Semaphore::new(MAX_TOTAL_CONNECTIONS)))
        .clone()
}

fn is_server_overload_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("429")
        || lower.contains("too many request")
        || lower.contains("rate limit")
        || lower.contains("503")
        || lower.contains("service unavailable")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connection closed")
        || lower.contains("timeout")
        || lower.contains("timed out")
}

async fn smart_batch_download(
    tasks: Vec<DownloadTask>,
    total: usize,
    done_counter: Arc<AtomicU64>,
    on_any_done: impl Fn(usize, usize, String) + Clone + Send + Sync + 'static,
    on_percent: Option<impl Fn(f64) + Clone + Send + Sync + 'static>,
) -> (usize, Vec<DownloadFailure>) {
    if tasks.is_empty() {
        return (0, Vec::new());
    }

    let concurrency = Arc::new(AtomicU64::new(SMART_INITIAL_CONCURRENCY as u64));
    let conn_sem = global_connection_semaphore();

    let tasks_wrapped: Vec<DownloadTask> = tasks.into_iter().collect();
    let total_tasks = tasks_wrapped.len();
    let pending = Arc::new(StdMutex::new(VecDeque::from(tasks_wrapped)));
    let in_flight = Arc::new(AtomicU64::new(0));
    let successes_arc = Arc::new(AtomicU64::new(0));
    let recent_results: Arc<StdMutex<VecDeque<bool>>> = Arc::new(StdMutex::new(VecDeque::new()));
    let failures: Arc<StdMutex<Vec<DownloadFailure>>> = Arc::new(StdMutex::new(Vec::new()));
    let overload_detected = Arc::new(AtomicBool::new(false));

    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<(bool, String, String, Vec<String>)>();

    let on_any_done_clone = on_any_done.clone();
    let percent_tx = on_percent.clone();
    let done_counter_clone = done_counter.clone();
    let successes_clone = successes_arc.clone();
    let recent_clone = recent_results.clone();
    let failures_clone = failures.clone();
    let overload_detected_clone = overload_detected.clone();

    tokio::spawn(async move {
        while let Some((success, fname, err, urls_tried)) = done_rx.recv().await {
            let done = done_counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
            on_any_done_clone(done as usize, total, fname.clone());
            if let Some(ref pt) = percent_tx {
                pt(if total > 0 {
                    (done as f64 / total as f64) * 100.0
                } else {
                    100.0
                });
            }
            if success {
                successes_clone.fetch_add(1, Ordering::SeqCst);
                if let Ok(mut rr) = recent_clone.lock() {
                    rr.push_back(true);
                    if rr.len() > SMART_ADJUST_BATCH * 2 {
                        rr.pop_front();
                    }
                }
            } else {
                if is_server_overload_error(&err) {
                    overload_detected_clone.store(true, Ordering::SeqCst);
                }
                if let Ok(mut rr) = recent_clone.lock() {
                    rr.push_back(false);
                    if rr.len() > SMART_ADJUST_BATCH * 2 {
                        rr.pop_front();
                    }
                }
                if let Ok(mut fails) = failures_clone.lock() {
                    fails.push(DownloadFailure {
                        file_name: fname,
                        error: err,
                        urls_tried,
                    });
                }
            }
        }
    });

    let last_adjust_done = Arc::new(AtomicU64::new(0));

    println!(
        "[SmartDownload] 初始化: 并发={}, 总文件={}, 每{}个调整一次",
        SMART_INITIAL_CONCURRENCY, total_tasks, SMART_ADJUST_BATCH
    );

    let mut spawn_count: u64 = 0;
    loop {
        let current_concurrency = concurrency.load(Ordering::SeqCst) as usize;
        let current_in_flight = in_flight.load(Ordering::SeqCst) as usize;
        let done_count = done_counter.load(Ordering::SeqCst) as usize;

        let pending_empty = pending.lock().ok().map(|q| q.is_empty()).unwrap_or(true);
        let nothing_left = pending_empty && current_in_flight == 0;
        if nothing_left {
            break;
        }

        if done_count >= total_tasks {
            break;
        }

        let adjust_delta = done_count as u64 - last_adjust_done.load(Ordering::SeqCst);
        if adjust_delta >= SMART_ADJUST_BATCH as u64
            || (adjust_delta > 0 && overload_detected.swap(false, Ordering::SeqCst))
        {
            last_adjust_done.store(done_count as u64, Ordering::SeqCst);
            let recent = recent_results
                .lock()
                .ok()
                .map(|q| q.iter().cloned().collect::<Vec<bool>>())
                .unwrap_or_default();
            let recent_count = recent.len().max(1);
            let recent_successes = recent.iter().filter(|&&x| x).count();
            let success_rate = recent_successes as f64 / recent_count as f64;

            let new_conc = if success_rate >= 0.9
                && current_in_flight >= current_concurrency.saturating_sub(1)
            {
                (((current_concurrency as f64) * 1.25).round() as usize)
                    .max(current_concurrency + 1)
                    .min(SMART_MAX_CONCURRENCY)
            } else if success_rate >= 0.5 {
                (((current_concurrency as f64) * 0.75).round() as usize)
                    .max(SMART_MIN_CONCURRENCY)
            } else {
                (((current_concurrency as f64) * 0.55).round() as usize)
                    .max(SMART_MIN_CONCURRENCY)
            };
            if new_conc != current_concurrency {
                concurrency.store(new_conc as u64, Ordering::SeqCst);
                println!(
                    "[SmartDownload] 调整并发: {} → {} | 最近{}个成功率={:.0}% | 已完成{}/{}",
                    current_concurrency,
                    new_conc,
                    recent_count,
                    success_rate * 100.0,
                    done_count,
                    total_tasks
                );
            }
        }

        let can_spawn_more = {
            let queue_empty = pending.lock().ok().map(|q| q.is_empty()).unwrap_or(true);
            !queue_empty && current_in_flight < concurrency.load(Ordering::SeqCst) as usize
        };

        if can_spawn_more {
            let next_task = pending.lock().ok().and_then(|mut q| q.pop_front());
            if let Some(task) = next_task {
                in_flight.fetch_add(1, Ordering::SeqCst);
                let fname = task.file_name.clone();
                let done_tx_clone = done_tx.clone();
                let in_flight_clone = in_flight.clone();
                let conn_sem_clone = conn_sem.clone();
                spawn_count += 1;

                tokio::spawn(async move {
                    let _conn_permit = conn_sem_clone.acquire_owned().await;
                    let result = download_file(&task, None, None).await;
                    in_flight_clone.fetch_sub(1, Ordering::SeqCst);

                    match result {
                        SingleDownloadResult::Success { .. } => {
                            let _ = done_tx_clone.send((
                                true,
                                fname.clone(),
                                String::new(),
                                Vec::new(),
                            ));
                        }
                        SingleDownloadResult::Failed {
                            error, urls_tried, ..
                        } => {
                            let _ = done_tx_clone.send((false, fname.clone(), error, urls_tried));
                        }
                    }
                });
                if spawn_count % 10 == 1 {
                    tokio::time::sleep(Duration::from_millis(80)).await;
                }
                continue;
            }
        }

        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let success_count = successes_arc.load(Ordering::SeqCst) as usize;
    let fail_list = failures.lock().ok().map(|m| m.clone()).unwrap_or_default();
    (success_count, fail_list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read as _, Write as _};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::sync::atomic::AtomicUsize;

    // ---------- 退避与外层重试计划 ----------

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_backoff(0), Duration::ZERO);
        assert_eq!(retry_backoff(1), RETRY_BACKOFF_BASE);
        assert_eq!(retry_backoff(2), RETRY_BACKOFF_BASE * 2);
        assert_eq!(retry_backoff(3), RETRY_BACKOFF_BASE * 4);
        assert_eq!(retry_backoff(40), RETRY_BACKOFF_MAX);
        assert_eq!(retry_backoff(u32::MAX), RETRY_BACKOFF_MAX);
        for n in 1..64 {
            assert!(retry_backoff(n) >= retry_backoff(n - 1));
            assert!(retry_backoff(n) <= RETRY_BACKOFF_MAX);
        }
    }

    #[test]
    fn parse_retry_after_only_accepts_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);
        headers.insert(reqwest::header::RETRY_AFTER, "3".parse().unwrap());
        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(3)));
        headers.insert(
            reqwest::header::RETRY_AFTER,
            "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn url_plan_single_url_is_bounded_and_backs_off() {
        let mut plan = UrlRetryPlan::new(1, 3, 8);
        assert_eq!(plan.next(), Some((0, Duration::ZERO)));
        assert_eq!(plan.next(), Some((0, retry_backoff(1))));
        assert_eq!(plan.next(), Some((0, retry_backoff(2))));
        assert_eq!(plan.next(), None);
    }

    #[test]
    fn url_plan_rotates_mirrors_before_retrying_same_url() {
        let mut plan = UrlRetryPlan::new(3, 3, 8);
        let seq: Vec<(usize, Duration)> = std::iter::from_fn(|| plan.next()).collect();
        let z = Duration::ZERO;
        let b1 = retry_backoff(1);
        let b2 = retry_backoff(2);
        assert_eq!(
            seq,
            vec![(0, z), (1, z), (2, z), (0, b1), (1, b1), (2, b1), (0, b2), (1, b2)],
            "换镜像不等待，同一 URL 再试才退避，总次数受上限约束"
        );
    }

    #[test]
    fn url_plan_skips_exhausted_sources() {
        let mut plan = UrlRetryPlan::new(2, 3, 8);
        assert_eq!(plan.next(), Some((0, Duration::ZERO)));
        plan.mark_exhausted(0);
        assert_eq!(plan.next(), Some((1, Duration::ZERO)));
        assert_eq!(plan.next(), Some((1, retry_backoff(1))));
        assert_eq!(plan.next(), Some((1, retry_backoff(2))));
        assert_eq!(plan.next(), None);

        // 只有一个源且已耗尽：立即结束，不再整轮重试
        let mut single = UrlRetryPlan::new(1, 3, 8);
        assert!(single.next().is_some());
        single.mark_exhausted(0);
        assert_eq!(single.next(), None);

        assert_eq!(UrlRetryPlan::new(0, 3, 8).next(), None);
    }

    // ---------- 分块调度器 ----------

    fn scheduler(total: u64, chunk: u64) -> DynamicScheduler {
        DynamicScheduler::new(total, Arc::new(AtomicU64::new(0)), chunk)
    }

    fn set_retry_at(s: &mut DynamicScheduler, at: Instant) {
        for c in s.retry_queue.iter_mut() {
            c.retry_at = Some(at);
        }
    }

    #[test]
    fn failed_chunk_keeps_attempts_and_gives_up_at_limit() {
        let mut s = scheduler(1024 * 1024, 256 * 1024);
        let mut chunk = s.alloc_next().expect("应分配第一个分块");
        assert_eq!(chunk.attempts, 0);
        let (start, end) = (chunk.range.start, chunk.range.end);

        for expected in 1..MAX_ATTEMPTS_PER_CHUNK {
            assert_eq!(
                s.mark_chunk_failed(&chunk, None),
                ChunkRetryDecision::Retry {
                    attempts: expected,
                    delay: retry_backoff(expected),
                }
            );

            // 退避未结束：既不重发该分块，也不分配新分块（旧实现会立刻分配 attempts = 0 的同范围分块）
            set_retry_at(&mut s, Instant::now() + Duration::from_secs(60));
            assert!(s.alloc_next().is_none());
            assert!(!s.is_done());
            assert!(s.next_retry_wait().unwrap() > Duration::from_secs(50));

            // 退避结束：拿回的是同一范围，attempts 延续
            set_retry_at(&mut s, Instant::now());
            chunk = s.alloc_next().expect("退避结束后应重新分配失败的分块");
            assert_eq!((chunk.range.start, chunk.range.end), (start, end));
            assert_eq!(chunk.attempts, expected, "重试时 attempts 不应被清零");
        }

        assert_eq!(
            s.mark_chunk_failed(&chunk, None),
            ChunkRetryDecision::Exhausted {
                attempts: MAX_ATTEMPTS_PER_CHUNK
            }
        );
        assert!(s.retry_queue.is_empty());
        assert!(s.in_flight.is_empty());
    }

    #[test]
    fn retry_after_extends_backoff_but_is_capped() {
        let mut s = scheduler(1024 * 1024, 256 * 1024);
        let c = s.alloc_next().unwrap();
        let longer = retry_backoff(1) * 3;
        assert!(longer < RETRY_BACKOFF_MAX);
        assert_eq!(
            s.mark_chunk_failed(&c, Some(longer)),
            ChunkRetryDecision::Retry {
                attempts: 1,
                delay: longer,
            }
        );

        set_retry_at(&mut s, Instant::now());
        let c = s.alloc_next().unwrap();
        assert_eq!(
            s.mark_chunk_failed(&c, Some(Duration::from_secs(3600))),
            ChunkRetryDecision::Retry {
                attempts: 2,
                delay: RETRY_BACKOFF_MAX,
            }
        );
    }

    #[test]
    fn chunk_recovers_after_retry_and_scheduler_completes() {
        let total = 1024 * 1024;
        let mut s = scheduler(total, 256 * 1024);
        let first = s.alloc_next().unwrap();
        assert!(matches!(
            s.mark_chunk_failed(&first, None),
            ChunkRetryDecision::Retry { attempts: 1, .. }
        ));
        set_retry_at(&mut s, Instant::now());

        let mut covered = 0u64;
        while let Some(c) = s.alloc_next() {
            covered += c.range.size();
            s.mark_done(c.range.start, c.range.end);
        }
        assert_eq!(covered, total, "每个字节恰好下载一次");
        assert!(s.is_done());
    }

    // ---------- 用本地 HTTP 服务模拟源站 ----------

    const TEST_FILE_SIZE: usize = MEDIUM_CHUNK as usize + 300 * 1024;

    #[derive(Clone, Copy)]
    enum ChunkFailure {
        /// 返回 429 + Retry-After
        TooManyRequests,
        /// 返回 206，但只给一半数据（分块不完整）
        ShortBody,
    }

    struct TestServer {
        url: String,
        probe_requests: Arc<AtomicUsize>,
        chunk_requests: Arc<AtomicUsize>,
    }

    fn test_body() -> Arc<Vec<u8>> {
        Arc::new((0..TEST_FILE_SIZE).map(|i| (i % 251) as u8).collect())
    }

    /// 起一个只监听 127.0.0.1 的极简 HTTP 服务：支持 Range；
    /// 起始偏移为 0 的请求视为探测请求，其余为分块请求，前 `failing_chunk_requests` 个分块请求按 `failure` 失败。
    fn start_server(
        body: Arc<Vec<u8>>,
        failing_chunk_requests: usize,
        failure: ChunkFailure,
    ) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口失败");
        let addr = listener.local_addr().unwrap();
        let probe_requests = Arc::new(AtomicUsize::new(0));
        let chunk_requests = Arc::new(AtomicUsize::new(0));
        let (probe, chunks) = (probe_requests.clone(), chunk_requests.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let (body, probe, chunks) = (body.clone(), probe.clone(), chunks.clone());
                std::thread::spawn(move || {
                    handle_connection(
                        stream,
                        &body,
                        &probe,
                        &chunks,
                        failing_chunk_requests,
                        failure,
                    )
                });
            }
        });
        TestServer {
            url: format!("http://{}/files/test.bin", addr),
            probe_requests,
            chunk_requests,
        }
    }

    fn handle_connection(
        mut stream: TcpStream,
        body: &[u8],
        probe: &AtomicUsize,
        chunks: &AtomicUsize,
        failing_chunk_requests: usize,
        failure: ChunkFailure,
    ) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut req = Vec::new();
        let mut buf = [0u8; 4096];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => req.extend_from_slice(&buf[..n]),
            }
        }
        let req = String::from_utf8_lossy(&req).to_string();
        let len = body.len();
        let range = req.lines().find_map(|line| {
            let lower = line.to_ascii_lowercase();
            let spec = lower.strip_prefix("range:")?.trim().strip_prefix("bytes=")?;
            let (a, b) = spec.split_once('-')?;
            let start: usize = a.trim().parse().ok()?;
            let end: usize = b.trim().parse::<usize>().ok()?.min(len - 1);
            Some((start, end))
        });

        let partial = |start: usize, end: usize| -> Vec<u8> {
            let mut out = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                start,
                end,
                len,
                end - start + 1
            )
            .into_bytes();
            out.extend_from_slice(&body[start..=end]);
            out
        };

        let response = match range {
            None => {
                let mut out = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    len
                )
                .into_bytes();
                out.extend_from_slice(body);
                out
            }
            Some((0, end)) => {
                probe.fetch_add(1, Ordering::SeqCst);
                partial(0, end)
            }
            Some((start, end)) => {
                let n = chunks.fetch_add(1, Ordering::SeqCst);
                if n < failing_chunk_requests {
                    match failure {
                        ChunkFailure::TooManyRequests => b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                        ChunkFailure::ShortBody => {
                            let half_end = start + (end - start + 1) / 2 - 1;
                            partial(start, half_end)
                        }
                    }
                } else {
                    partial(start, end)
                }
            }
        };
        let _ = stream.write_all(&response);
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Write);
    }

    fn file_task(dir: &Path, name: &str, urls: Vec<String>, sha1: Option<String>) -> DownloadTask {
        DownloadTask {
            file_name: name.to_string(),
            target_dir: dir.to_path_buf(),
            urls,
            sha1,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn chunk_download_fails_fast_when_source_keeps_failing() {
        let server = start_server(test_body(), usize::MAX, ChunkFailure::TooManyRequests);
        let dir = tempfile::tempdir().unwrap();
        let task = file_task(dir.path(), "always-429.bin", vec![server.url.clone()], None);

        let started = Instant::now();
        // 修复前：分块 attempts 每次都被清零，只能等 worker 120s 超时，再被外层对同一 URL 重试 8 轮
        let result = tokio::time::timeout(Duration::from_secs(30), download_file(&task, None, None))
            .await
            .expect("应在分块重试上限内快速失败，而不是一直重试到总超时");
        let elapsed = started.elapsed();

        match result {
            SingleDownloadResult::Failed { error, urls_tried } => {
                assert!(error.contains("重试上限"), "错误信息应说明已达重试上限: {error}");
                assert!(error.contains("429"), "错误信息应带上最后一次的失败原因: {error}");
                assert_eq!(
                    urls_tried,
                    vec![server.url.clone()],
                    "分块重试已用完的源不应再被外层整轮重试"
                );
            }
            SingleDownloadResult::Success { .. } => panic!("源站一直返回 429，不应下载成功"),
        }
        assert_eq!(
            server.chunk_requests.load(Ordering::SeqCst),
            MAX_ATTEMPTS_PER_CHUNK as usize,
            "同一分块的请求次数应等于重试上限"
        );
        assert_eq!(server.probe_requests.load(Ordering::SeqCst), 1);
        assert!(elapsed < Duration::from_secs(10), "失败应及时返回, 实际耗时 {elapsed:?}");
        let leftovers: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert!(leftovers.is_empty(), "失败后不应残留临时文件: {leftovers:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn chunk_retry_recovers_with_correct_content_and_progress() {
        let body = test_body();
        let server = start_server(body.clone(), 2, ChunkFailure::ShortBody);
        let dir = tempfile::tempdir().unwrap();
        let expected_sha1 = hex::encode(Sha1::digest(body.as_slice()));
        let task = file_task(
            dir.path(),
            "flaky.bin",
            vec![server.url.clone()],
            Some(expected_sha1),
        );
        let (tx, mut rx) = mpsc::channel(4096);

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            download_file(&task, Some(tx), None),
        )
        .await
        .expect("偶发失败的分块应在重试后完成");

        match result {
            SingleDownloadResult::Success {
                path,
                used_url,
                size,
                ..
            } => {
                assert_eq!(used_url, server.url);
                assert_eq!(size, TEST_FILE_SIZE as u64);
                assert!(fs::read(&path).unwrap() == *body, "重试后文件内容应与源文件一致");
            }
            SingleDownloadResult::Failed { error, .. } => {
                panic!("偶发失败的分块应在重试后成功: {error}")
            }
        }
        assert!(server.chunk_requests.load(Ordering::SeqCst) >= 3);
        assert_eq!(server.probe_requests.load(Ordering::SeqCst), 1);
        while let Ok((done, total)) = rx.try_recv() {
            if total > 0 {
                assert!(done <= total, "失败分块的字节不应重复计入进度: {done}/{total}");
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exhausted_source_switches_to_next_mirror() {
        let body = test_body();
        let bad = start_server(body.clone(), usize::MAX, ChunkFailure::TooManyRequests);
        let good = start_server(body.clone(), 0, ChunkFailure::TooManyRequests);
        let dir = tempfile::tempdir().unwrap();
        let task = file_task(
            dir.path(),
            "mirror.bin",
            vec![bad.url.clone(), good.url.clone()],
            None,
        );

        let result = tokio::time::timeout(Duration::from_secs(30), download_file(&task, None, None))
            .await
            .expect("第一个源耗尽后应切换到镜像");

        match result {
            SingleDownloadResult::Success { path, used_url, .. } => {
                assert_eq!(used_url, good.url);
                assert!(fs::read(&path).unwrap() == *body);
            }
            SingleDownloadResult::Failed { error, .. } => panic!("镜像可用时不应失败: {error}"),
        }
        assert_eq!(bad.chunk_requests.load(Ordering::SeqCst), MAX_ATTEMPTS_PER_CHUNK as usize);
        assert_eq!(bad.probe_requests.load(Ordering::SeqCst), 1);
        assert_eq!(good.probe_requests.load(Ordering::SeqCst), 1);
    }
}