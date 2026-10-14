use crate::model::{EngineError, JobEvent, JobId, JobSpec, JobState};
use crate::store::JobStore;
use crate::validation::validate_job_spec;
use reqwest::Client;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

fn sanitize_source(source: &str) -> String {
    Url::parse(source)
        .ok()
        .map(|url| {
            format!(
                "{}{}",
                url.host_str().unwrap_or("<unknown>"),
                url.path()
            )
        })
        .unwrap_or_else(|| "<invalid-url>".into())
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub destination_root: PathBuf,
    pub inactivity_timeout: Duration,
}

pub struct TransferEngine {
    pub store: JobStore,
    destination_root: PathBuf,
    client: Client,
    operational_sources: Mutex<HashMap<JobId, String>>,
    active_cancellations: Mutex<HashMap<JobId, tokio::sync::watch::Sender<bool>>>,
    pub event_tx: tokio::sync::broadcast::Sender<JobEvent>,
    pub limiter: Arc<crate::limiter::SpeedLimiter>,
    pub max_concurrent_jobs: Arc<std::sync::atomic::AtomicUsize>,
    inactivity_timeout: Duration,
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Mutex<Option<Arc<TransferEngine>>>>,
    destination_root: PathBuf,
    inactivity_timeout: Duration,
}

impl Engine {
    pub fn new(destination_root: PathBuf) -> Result<Self, EngineError> {
        Self::with_timeouts(destination_root, Duration::from_secs(30))
    }

    pub fn with_timeouts(
        destination_root: PathBuf,
        inactivity_timeout: Duration,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            inner: Arc::new(Mutex::new(None)),
            destination_root,
            inactivity_timeout,
        })
    }

    fn get_or_init(&self) -> Result<Arc<TransferEngine>, EngineError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| EngineError::InvalidJob("engine lock poisoned".into()))?;
        if guard.is_none() {
            let engine = Arc::new(TransferEngine::new(EngineConfig {
                destination_root: self.destination_root.clone(),
                inactivity_timeout: self.inactivity_timeout,
            })?);
            *guard = Some(engine);
        }
        Ok(Arc::clone(guard.as_ref().unwrap()))
    }

    pub fn subscribe_events(&self) -> Result<tokio::sync::broadcast::Receiver<JobEvent>, EngineError> {
        let engine = self.get_or_init()?;
        Ok(engine.event_tx.subscribe())
    }

    pub async fn create_job(&self, spec: JobSpec) -> Result<JobId, EngineError> {
        let engine = self.get_or_init()?;
        engine.create_job(spec)
    }

    pub async fn start_job(&self, id: JobId) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        // CRIT-01 fix: spawn the transfer as a background task so we return
        // immediately, releasing the Tauri engine mutex before any network I/O.
        // start_job_async itself handles the Queued early-exit when at capacity.
        let self_clone = self.clone();
        tokio::spawn(async move {
            let _ = engine.start_job_async(id).await;
            let _ = self_clone.start_next_queued().await;
        });
        Ok(())
    }

    pub async fn list_jobs(&self) -> Result<Vec<crate::store::StoredJob>, EngineError> {
        let engine = self.get_or_init()?;
        engine.store.list_jobs()
    }

    pub async fn pause_job(&self, id: JobId) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        let res = engine.stop_job(&id, JobState::Paused).await;
        let _ = self.start_next_queued().await;
        res
    }

    pub async fn resume_job(&self, id: JobId) -> Result<(), EngineError> {
        self.set_state(id.clone(), JobState::Queued).await?;
        let _ = self.start_next_queued().await;
        Ok(())
    }

    pub async fn cancel_job(&self, id: JobId) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        let res = engine.stop_job(&id, JobState::Canceled).await;
        let _ = self.start_next_queued().await;
        res
    }

    pub async fn retry_job(&self, id: JobId) -> Result<(), EngineError> {
        self.set_state(id.clone(), JobState::Queued).await?;
        let _ = self.start_next_queued().await;
        Ok(())
    }

    pub async fn refresh_source(&self, id: JobId, new_source: Url) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        engine.refresh_source(id, new_source)
    }

    pub fn set_speed_limit(&self, limit_bytes_per_sec: u64) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        engine.limiter.set_limit(limit_bytes_per_sec);
        Ok(())
    }

    pub fn get_speed_limit(&self) -> Result<u64, EngineError> {
        let engine = self.get_or_init()?;
        Ok(engine.limiter.get_limit())
    }

    pub fn set_max_concurrent_jobs(&self, max: usize) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        engine.max_concurrent_jobs.store(max, Ordering::Relaxed);
        Ok(())
    }

    pub fn get_max_concurrent_jobs(&self) -> Result<usize, EngineError> {
        let engine = self.get_or_init()?;
        Ok(engine.max_concurrent_jobs.load(Ordering::Relaxed))
    }

    pub async fn preview_zip_archive(&self, url: &str, total_bytes: u64) -> Result<Vec<crate::archive::ZipEntryPreview>, EngineError> {
        let engine = self.get_or_init()?;
        crate::archive::preview_remote_zip(&engine.client, url, total_bytes).await
    }

    pub async fn start_next_queued(&self) -> Result<Option<JobId>, EngineError> {
        let engine = self.get_or_init()?;
        let active_count = engine.active_cancellations.lock().map(|g| g.len()).unwrap_or(0);
        let max_concurrency = engine.max_concurrent_jobs.load(Ordering::Relaxed);
        if max_concurrency > 0 && active_count >= max_concurrency {
            return Ok(None);
        }

        if let Ok(jobs) = engine.store.list_jobs() {
            for job in jobs {
                if job.state == JobState::Queued {
                    let next_id = job.id.clone();
                    let engine_clone = Arc::clone(&engine);
                    tokio::spawn(async move {
                        let _ = engine_clone.start_job_async(next_id).await;
                    });
                    return Ok(Some(job.id));
                }
            }
        }
        Ok(None)
    }

    async fn set_state(&self, id: JobId, state: JobState) -> Result<(), EngineError> {
        let engine = self.get_or_init()?;
        engine.store.set_state(id, state)
    }
}

impl TransferEngine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        fs::create_dir_all(&config.destination_root)
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        let store = JobStore::open(&config.destination_root.join("jobs.db"))?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .connect_timeout(config.inactivity_timeout)
            .build()
            .map_err(|error| EngineError::InvalidJob(error.to_string()))?;
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Ok(Self {
            store,
            destination_root: config.destination_root,
            client,
            operational_sources: Mutex::new(HashMap::new()),
            active_cancellations: Mutex::new(HashMap::new()),
            event_tx,
            limiter: Arc::new(crate::limiter::SpeedLimiter::default()),
            max_concurrent_jobs: Arc::new(std::sync::atomic::AtomicUsize::new(3)),
            inactivity_timeout: config.inactivity_timeout,
        })
    }

    pub fn emit_event(&self, event: JobEvent) {
        let _ = self.event_tx.send(event);
    }

    pub fn create_job(&self, spec: JobSpec) -> Result<JobId, EngineError> {
        validate_job_spec(&spec)?;
        let destination = self.destination_root.join(&spec.destination);
        if destination.exists() && matches!(spec.conflict_policy, crate::model::ConflictPolicy::Reject) {
            return Err(EngineError::DestinationExists);
        }
        let id = self.store.create_job(&spec)?;
        self.operational_sources
            .lock()
            .map_err(|_| EngineError::InvalidJob("source lock poisoned".into()))?
            .insert(id.clone(), spec.source.to_string());
        Ok(id)
    }

    pub fn refresh_source(&self, id: JobId, new_source: Url) -> Result<(), EngineError> {
        self.operational_sources
            .lock()
            .map_err(|_| EngineError::InvalidJob("source lock poisoned".into()))?
            .insert(id.clone(), new_source.to_string());
        self.store.update_source(&id, &new_source)?;
        Ok(())
    }

    fn fail_active_job(
        &self,
        id: &JobId,
        completed_bytes: u64,
        total_bytes: Option<u64>,
        reason: &str,
    ) {
        let _ = self
            .active_cancellations
            .lock()
            .map(|mut guard| guard.remove(id));
        let _ = self.store.update_checkpoint(id.clone(), completed_bytes);
        let _ = self.store.set_state(id.clone(), JobState::Failed);
        eprintln!(
            "[hopper] job={} state=failed bytes_written={} total_bytes={:?} reason={}",
            id.0, completed_bytes, total_bytes, reason
        );
        self.emit_event(JobEvent {
            job_id: id.clone(),
            state: JobState::Failed,
            completed_bytes,
            total_bytes,
            speed_bytes_per_sec: 0,
            retry_count: 0,
        });
    }

    pub async fn stop_job(&self, id: &JobId, state: JobState) -> Result<(), EngineError> {
        if let Some(sender) = self
            .active_cancellations
            .lock()
            .map_err(|_| EngineError::InvalidJob("lock poisoned".into()))?
            .remove(id)
        {
            let _ = sender.send(true);
        }
        self.store.set_state(id.clone(), state)?;
        self.emit_event(JobEvent {
            job_id: id.clone(),
            state,
            completed_bytes: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
            retry_count: 0,
        });
        Ok(())
    }

    /// Synchronous wrapper for non-async test contexts
    pub fn start_job(&self, id: JobId) -> Result<(), EngineError> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(self.start_job_async(id))),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
                rt.block_on(self.start_job_async(id))
            }
        }
    }

    pub async fn start_job_async(&self, id: JobId) -> Result<(), EngineError> {
        let job = self.store.get_job(id.clone())?;
        let active_count = self.active_cancellations.lock().map(|g| g.len()).unwrap_or(0);
        let max_concurrency = self.max_concurrent_jobs.load(Ordering::Relaxed);
        if max_concurrency > 0 && active_count >= max_concurrency {
            self.store.set_state(id.clone(), JobState::Queued)?;
            self.emit_event(JobEvent {
                job_id: id,
                state: JobState::Queued,
                completed_bytes: job.completed_bytes,
                total_bytes: job.total_bytes,
                speed_bytes_per_sec: 0,
                retry_count: 0,
            });
            return Ok(());
        }

        let source = self
            .operational_sources
            .lock()
            .map_err(|_| EngineError::InvalidJob("source lock poisoned".into()))?
            .get(&id)
            .cloned()
            .ok_or_else(|| EngineError::InvalidJob("reauthorization required after restart".into()))?;

        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        self.active_cancellations
            .lock()
            .map_err(|_| EngineError::InvalidJob("lock poisoned".into()))?
            .insert(id.clone(), cancel_tx);

        self.store.set_state(id.clone(), JobState::Active)?;
        self.emit_event(JobEvent {
            job_id: id.clone(),
            state: JobState::Active,
            completed_bytes: 0,
            total_bytes: job.total_bytes,
            speed_bytes_per_sec: 0,
            retry_count: 0,
        });

        let temporary = self.destination_root.join(format!(".{}.part", id.0));
        let mut final_path = self.destination_root.join(&job.destination);
        if final_path.exists() && matches!(job.conflict_policy, crate::model::ConflictPolicy::Rename) {
            let stem = final_path.file_stem().and_then(|v| v.to_str()).unwrap_or("download");
            let extension = final_path.extension().and_then(|v| v.to_str()).unwrap_or("");
            let mut index = 1;
            loop {
                let name = if extension.is_empty() {
                    format!("{stem} ({index})")
                } else {
                    format!("{stem} ({index}).{extension}")
                };
                let candidate = final_path.with_file_name(name);
                if !candidate.exists() {
                    final_path = candidate;
                    break;
                }
                index += 1;
            }
        }

        let file = match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&temporary)
        {
            Ok(file) => Arc::new(file),
            Err(error) => {
                let reason = format!("file open failed: {error}");
                self.fail_active_job(&id, 0, job.total_bytes, &reason);
                return Err(EngineError::InvalidJob(reason));
            }
        };

        // Send initial GET request
        let request_started = std::time::Instant::now();
        eprintln!(
            "[hopper] job={} request_start source={}",
            id.0,
            sanitize_source(&source)
        );
        let initial_response = match tokio::time::timeout(
            self.inactivity_timeout,
            self.client.get(&source).send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let reason = format!("request failed: {error}");
                self.fail_active_job(&id, 0, job.total_bytes, &reason);
                return Err(EngineError::InvalidJob(reason));
            }
            Err(_) => {
                let reason = format!(
                    "request timed out after {} ms",
                    self.inactivity_timeout.as_millis()
                );
                self.fail_active_job(&id, 0, job.total_bytes, &reason);
                return Err(EngineError::InvalidJob(reason));
            }
        };
        eprintln!(
            "[hopper] job={} response status={} latency_ms={} content_type={} content_length={:?}",
            id.0,
            initial_response.status(),
            request_started.elapsed().as_millis(),
            initial_response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            initial_response.content_length()
        );

        if !initial_response.status().is_success() {
            // CRIT-02 fix: remove the active slot BEFORE returning so it is not
            // permanently leaked when the server returns 4xx/5xx.
            let _ = self
                .active_cancellations
                .lock()
                .map(|mut g| g.remove(&id));
            let _ = self.store.set_state(id.clone(), JobState::Failed);
            self.emit_event(JobEvent {
                job_id: id.clone(),
                state: JobState::Failed,
                completed_bytes: 0,
                total_bytes: None,
                speed_bytes_per_sec: 0,
                retry_count: 0,
            });
            return Err(EngineError::InvalidJob(format!(
                "HTTP {}",
                initial_response.status()
            )));
        }

        let content_length = initial_response.content_length();
        let accept_ranges = initial_response
            .headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);

        if let Some(total) = content_length {
            let _ = self.store.update_total_bytes(&id, total);
        }

        let result: Result<(), EngineError> = if accept_ranges && content_length.unwrap_or(0) >= 512 * 1024 {
            let total_bytes = content_length.unwrap();
            let _ = file.set_len(total_bytes);

            let mut segments = self.store.load_segments(&id).unwrap_or_default();
            if segments.is_empty() {
                segments = crate::transfer::create_initial_segments(total_bytes, 8);
                let _ = self.store.save_segments(&id, &segments);
            }

            self.download_multisegment(&id, &source, &file, segments, total_bytes, cancel_rx).await
        } else {
            // Single stream using the response we already received
            self.stream_response_to_file(&id, initial_response, &file, content_length, cancel_rx).await
        };

        self.active_cancellations
            .lock()
            .map_err(|_| EngineError::InvalidJob("lock poisoned".into()))?
            .remove(&id);

        match result {
            Ok(()) => {
                fs::rename(&temporary, &final_path)
                    .map_err(|e| EngineError::InvalidJob(e.to_string()))?;
                self.store.set_state(id.clone(), JobState::Completed)?;
                let final_bytes = content_length.unwrap_or_else(|| {
                    std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0)
                });
                eprintln!(
                    "[hopper] job={} state=completed bytes_written={} total_bytes={}",
                    id.0, final_bytes, final_bytes
                );
                self.emit_event(JobEvent {
                    job_id: id,
                    state: JobState::Completed,
                    completed_bytes: final_bytes,
                    total_bytes: Some(final_bytes),
                    speed_bytes_per_sec: 0,
                    retry_count: 0,
                });
                Ok(())
            }
            Err(e) => {
                let current_state = self.store.get_job(id.clone())?.state;
                if current_state != JobState::Paused && current_state != JobState::Canceled {
                    let completed_bytes = self.store.get_job(id.clone())?.completed_bytes;
                    let _ = self.store.set_state(id.clone(), JobState::Failed);
                    eprintln!(
                        "[hopper] job={} state=failed bytes_written={} reason={}",
                        id.0, completed_bytes, e
                    );
                    self.emit_event(JobEvent {
                        job_id: id,
                        state: JobState::Failed,
                        completed_bytes,
                        total_bytes: content_length,
                        speed_bytes_per_sec: 0,
                        retry_count: 0,
                    });
                }
                Err(e)
            }
        }
    }

    async fn stream_response_to_file(
        &self,
        id: &JobId,
        mut response: reqwest::Response,
        file: &Arc<File>,
        total_bytes: Option<u64>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let mut current_offset = 0;
        let mut last_instant = std::time::Instant::now();
        let mut last_bytes = 0u64;

        let mut first_chunk = true;
        loop {
            if *cancel_rx.borrow() {
                self.store.update_checkpoint(id.clone(), current_offset)?;
                return Err(EngineError::InvalidJob("job paused or canceled".into()));
            }
            match tokio::time::timeout(self.inactivity_timeout, response.chunk()).await {
                Ok(Ok(Some(chunk))) => {
                    if first_chunk {
                        eprintln!(
                            "[hopper] job={} first_chunk bytes={} elapsed_ms={}",
                            id.0,
                            chunk.len(),
                            last_instant.elapsed().as_millis()
                        );
                        first_chunk = false;
                    }
                    let chunk_len = chunk.len() as u64;
                    self.limiter.consume(chunk_len).await;
                    crate::transfer::write_at(file, &chunk, current_offset).map_err(|e| {
                        EngineError::InvalidJob(format!("file write failed after {} bytes: {e}", current_offset))
                    })?;
                    current_offset += chunk_len;

                    if last_instant.elapsed() >= Duration::from_millis(400) {
                        let elapsed = last_instant.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            (current_offset.saturating_sub(last_bytes) as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        last_bytes = current_offset;
                        last_instant = std::time::Instant::now();
                        self.emit_event(JobEvent {
                            job_id: id.clone(),
                            state: JobState::Active,
                            completed_bytes: current_offset,
                            total_bytes,
                            speed_bytes_per_sec: speed,
                            retry_count: 0,
                        });
                    }
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    let reason = format!("response body failed after {} bytes: {error}", current_offset);
                    eprintln!("[hopper] job={} body_error={}", id.0, reason);
                    self.store.update_checkpoint(id.clone(), current_offset)?;
                    return Err(EngineError::InvalidJob(reason));
                }
                Err(_) => {
                    let reason = format!(
                        "response body timed out after {} bytes ({} ms inactivity)",
                        current_offset,
                        self.inactivity_timeout.as_millis()
                    );
                    eprintln!("[hopper] job={} timeout={}", id.0, reason);
                    self.store.update_checkpoint(id.clone(), current_offset)?;
                    return Err(EngineError::InvalidJob(reason));
                }
            }
        }

        if let Some(expected) = total_bytes {
            if current_offset != expected {
                let reason = format!(
                    "incomplete response: wrote {} of {} bytes",
                    current_offset, expected
                );
                self.store.update_checkpoint(id.clone(), current_offset)?;
                return Err(EngineError::InvalidJob(reason));
            }
        }

        self.store.update_checkpoint(id.clone(), current_offset)?;
        eprintln!(
            "[hopper] job={} bytes_written={} final=stream_complete",
            id.0, current_offset
        );
        Ok(())
    }

    async fn download_multisegment(
        &self,
        id: &JobId,
        source: &str,
        file: &Arc<File>,
        initial_segments: Vec<crate::model::Segment>,
        total_bytes: u64,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let segments = Arc::new(Mutex::new(initial_segments));
        let initial_completed: u64 = segments.lock().unwrap().iter().map(|s| s.completed).sum();
        let total_completed = Arc::new(AtomicU64::new(initial_completed));

        // Background progress reporter task
        let reporter_id = id.clone();
        let reporter_completed = Arc::clone(&total_completed);
        let reporter_tx = self.event_tx.clone();
        let mut reporter_cancel = cancel_rx.clone();

        let progress_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(400));
            let mut last_bytes = reporter_completed.load(Ordering::Relaxed);
            let mut last_instant = std::time::Instant::now();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let current = reporter_completed.load(Ordering::Relaxed);
                        let elapsed = last_instant.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            (current.saturating_sub(last_bytes) as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        last_bytes = current;
                        last_instant = std::time::Instant::now();

                        let _ = reporter_tx.send(JobEvent {
                            job_id: reporter_id.clone(),
                            state: JobState::Active,
                            completed_bytes: current,
                            total_bytes: Some(total_bytes),
                            speed_bytes_per_sec: speed,
                            retry_count: 0,
                        });
                    }
                    _ = reporter_cancel.changed() => {
                        break;
                    }
                }
            }
        });

        let concurrency = 8;
        let mut tasks = Vec::new();

        for _ in 0..concurrency {
            let segments = Arc::clone(&segments);
            let total_completed = Arc::clone(&total_completed);
            let file = Arc::clone(file);
            let client = self.client.clone();
            let source = source.to_string();
            let cancel_rx = cancel_rx.clone();
            let limiter = Arc::clone(&self.limiter);
            let inactivity_timeout = self.inactivity_timeout;

            let task = tokio::spawn(async move {
                loop {
                    if *cancel_rx.borrow() {
                        break;
                    }

                    // 1. Find next unclaimed+uncompleted segment, or split the largest.
                    // HIGH-02 fix: check `claimed` flag so multiple workers cannot race
                    // on the same segment and corrupt the output file.
                    let (seg_idx, current_start, end) = {
                        let mut segs = segments.lock().unwrap();
                        let found = segs.iter().enumerate().find_map(|(i, s)| {
                            let remaining = (s.end + 1).saturating_sub(s.start + s.completed);
                            if remaining > 0 && !s.claimed {
                                Some((i, s.start + s.completed, s.end))
                            } else {
                                None
                            }
                        });

                        match found {
                            Some((idx, start, end)) => {
                                // Claim the segment before releasing the lock.
                                segs[idx].claimed = true;
                                (idx, start, end)
                            }
                            None => {
                                if let Some(new_seg) =
                                    crate::transfer::try_split_largest_segment(&mut segs, 256 * 1024)
                                {
                                    let new_idx = segs.len();
                                    let start = new_seg.start;
                                    let end = new_seg.end;
                                    // New segment is implicitly claimed by this worker.
                                    let mut claimed_seg = new_seg;
                                    claimed_seg.claimed = true;
                                    segs.push(claimed_seg);
                                    (new_idx, start, end)
                                } else {
                                    break;
                                }
                            }
                        }
                    };

                    let range_header = format!("bytes={current_start}-{end}");
                    let mut resp = match tokio::time::timeout(
                        inactivity_timeout,
                        client.get(&source).header("Range", &range_header).send(),
                    )
                    .await
                    {
                        Ok(Ok(r)) if r.status() == reqwest::StatusCode::PARTIAL_CONTENT => r,
                        Ok(Ok(r)) => {
                            let mut segs = segments.lock().unwrap();
                            if let Some(seg) = segs.get_mut(seg_idx) {
                                seg.claimed = false;
                            }
                            return Err(format!(
                                "range {} returned unexpected HTTP status {}",
                                range_header,
                                r.status()
                            ));
                        }
                        Ok(Err(error)) => return Err(format!("range request failed: {error}")),
                        Err(_) => return Err(format!(
                            "range request timed out after {} ms",
                            inactivity_timeout.as_millis()
                        )),
                    };
                    let content_range = resp
                        .headers()
                        .get(reqwest::header::CONTENT_RANGE)
                        .and_then(|value| value.to_str().ok())
                        ;
                    crate::transfer::validate_range_response(
                        resp.status(),
                        content_range,
                        current_start,
                        end,
                        total_bytes,
                    )
                    .map_err(|error| error.to_string())?;

                    let mut current_offset = current_start;
                    loop {
                        let chunk = match tokio::time::timeout(inactivity_timeout, resp.chunk()).await {
                            Ok(Ok(Some(chunk))) => chunk,
                            Ok(Ok(None)) => break,
                            Ok(Err(error)) => {
                                return Err(format!(
                                    "range body failed after {} bytes: {error}",
                                    current_offset
                                ));
                            }
                            Err(_) => {
                                return Err(format!(
                                    "range body timed out after {} bytes",
                                    current_offset
                                ));
                            }
                        };
                        if *cancel_rx.borrow() {
                            break;
                        }

                        // Check current bounds in case segment was dynamically split in half
                        let target_end = {
                            let segs = segments.lock().unwrap();
                            segs.get(seg_idx).map(|s| s.end).unwrap_or(end)
                        };

                        if current_offset > target_end {
                            break;
                        }

                        let chunk_len = chunk.len() as u64;
                        let write_len = if current_offset + chunk_len > target_end + 1 {
                            (target_end + 1 - current_offset) as usize
                        } else {
                            chunk_len as usize
                        };

                        if write_len > 0 {
                            limiter.consume(write_len as u64).await;
                            crate::transfer::write_at(&file, &chunk[..write_len], current_offset)
                                .map_err(|error| format!("range file write failed: {error}"))?;
                            current_offset += write_len as u64;
                            total_completed.fetch_add(write_len as u64, Ordering::Relaxed);

                            let mut segs = segments.lock().unwrap();
                            if let Some(seg) = segs.get_mut(seg_idx) {
                                seg.completed += write_len as u64;
                            }
                        }

                        if write_len < chunk_len as usize {
                            // Truncated at dynamic split boundary
                            break;
                        }
                    }

                    if *cancel_rx.borrow() {
                        let mut segs = segments.lock().unwrap();
                        if let Some(seg) = segs.get_mut(seg_idx) {
                            if (seg.end + 1).saturating_sub(seg.start + seg.completed) > 0 {
                                seg.claimed = false;
                            }
                        }
                        break;
                    }
                }
                Ok(())
            });
            tasks.push(task);
        }

        let mut task_error = None;
        for task in tasks {
            match task.await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if task_error.is_none() {
                        task_error = Some(error);
                    }
                }
                Err(error) => {
                    if task_error.is_none() {
                        task_error = Some(format!("range worker failed: {error}"));
                    }
                }
            }
        }

        progress_task.abort();

        if let Some(error) = task_error {
            let completed = total_completed.load(Ordering::Relaxed);
            self.store.update_checkpoint(id.clone(), completed)?;
            return Err(EngineError::InvalidJob(error));
        }

        let current_segments = segments.lock().unwrap().clone();
        let _ = self.store.save_segments(id, &current_segments);
        let completed = total_completed.load(Ordering::Relaxed);
        self.store.update_checkpoint(id.clone(), completed)?;

        if *cancel_rx.borrow() {
            return Err(EngineError::InvalidJob("job paused or canceled".into()));
        }

        if completed < total_bytes {
            return Err(EngineError::InvalidJob("transfer interrupted".into()));
        }

        Ok(())
    }
}
