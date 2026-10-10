mod ai;
mod body;
mod cpu;
mod model_recovery;
use frameiq_contracts::*;
use frameiq_persistence::Database;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
type EventSink = Arc<dyn Fn(JobEvent) + Send + Sync>;
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::new("APPLICATION_ERROR", e.to_string())
}
fn lock<T>(m: &Mutex<T>) -> AppResult<MutexGuard<'_, T>> {
    m.lock()
        .map_err(|_| AppError::new("INTERNAL_ERROR", "An application resource was interrupted."))
}
fn lock_cancellable<T>(m: &Mutex<T>, cancelled: impl Fn() -> bool) -> AppResult<MutexGuard<'_, T>> {
    loop {
        if cancelled() {
            return Err(AppError::new("CANCELLED", "Queued work cancelled."));
        }
        match m.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(_) => return Err(error("Worker lane interrupted")),
        }
    }
}
struct Inner {
    _instance_lock: File,
    db: Mutex<Database>,
    worker: PathBuf,
    cache: PathBuf,
    sink: EventSink,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    background: Mutex<()>,
    interactive: Mutex<()>,
    preview_generation: AtomicU64,
    ai: Mutex<Option<ai::AiProcess>>,
    body: Mutex<()>,
    cpu: cpu::Budget,
}
#[derive(Clone)]
pub struct Application {
    inner: Arc<Inner>,
}
impl Application {
    pub fn new(data: &Path, worker: PathBuf, sink: EventSink) -> AppResult<Self> {
        let cache = data.join("cache");
        fs::create_dir_all(&cache).map_err(error)?;
        let instance_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data.join("application.lock"))
            .map_err(error)?;
        instance_lock.try_lock().map_err(|_| {
            AppError::new(
                "INSTANCE_RUNNING",
                "Another FrameIQ instance is using this project database.",
            )
        })?;
        // This directory contains only app-owned intermediate export files. The instance
        // lock guarantees that recovery cannot remove another running export's files.
        let export_staging = cache.join("export-staging");
        if export_staging.exists() || export_staging.is_symlink() {
            let metadata = fs::symlink_metadata(&export_staging).map_err(error)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(error(
                    "Export staging must be an application-owned directory",
                ));
            }
            fs::remove_dir_all(&export_staging).map_err(error)?;
        }
        fs::create_dir(&export_staging).map_err(error)?;
        if !worker.is_file() {
            return Err(AppError::new(
                "WORKER_MISSING",
                "Build the FrameIQ worker before starting the desktop application.",
            ));
        }
        let db = Database::open(&data.join("frameiq.db"))?;
        body::recover_mask_orphans(&cache, &db)?;
        model_recovery::recover(data, &db)?;
        Ok(Self {
            inner: Arc::new(Inner {
                _instance_lock: instance_lock,
                db: Mutex::new(db),
                worker,
                cache,
                sink,
                jobs: Mutex::new(HashMap::new()),
                background: Mutex::new(()),
                interactive: Mutex::new(()),
                preview_generation: AtomicU64::new(0),
                ai: Mutex::new(None),
                body: Mutex::new(()),
                cpu: cpu::Budget::new(
                    (std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(1)
                        / 2)
                    .clamp(1, 4),
                ),
            }),
        })
    }
    pub fn presets(&self) -> AppResult<Vec<PresetV1>> {
        let mut presets = frameiq_analysis::presets::builtins();
        presets.extend(lock(&self.inner.db)?.presets()?);
        Ok(presets)
    }
    pub fn save_preset(&self, mut preset: PresetV1) -> AppResult<PresetV1> {
        frameiq_analysis::presets::validate(&preset)?;
        preset.id = Uuid::new_v4().to_string();
        preset.version = 1;
        preset.automatic_available = false;
        preset.unavailable_reason =
            Some("Automatic regions require validated feature packs.".into());
        let sum = preset.weights.iter().sum::<f64>();
        for w in &mut preset.weights {
            *w = *w / sum * 100.;
        }
        lock(&self.inner.db)?.save_preset(&preset)?;
        Ok(preset)
    }
    pub fn regions(&self, source: &str) -> AppResult<Vec<GuidedRegionV1>> {
        lock(&self.inner.db)?.regions(source)
    }
    pub fn remove_region(&self, id: &str) -> AppResult<()> {
        lock(&self.inner.db)?.remove_region(id)
    }
    pub fn measure_region(
        &self,
        source: &str,
        n: &str,
        preset: &str,
        rect: RegionRectV1,
    ) -> AppResult<JobEvent> {
        frameiq_analysis::presets::validate_rectangle(&rect)?;
        if !self.presets()?.iter().any(|p| p.id == preset) {
            return Err(AppError::new(
                "INVALID_PRESET",
                "Choose an existing preset.",
            ));
        }
        // Snapshot the explicitly chosen subject before scheduling; later changes do not relabel a job.
        let subject = self.subject_choice(source, n)?.map(|choice| choice.subject);
        let source = source.to_owned();
        let n = n.to_owned();
        let preset = preset.to_owned();
        let (mut event, cancel) = self.job("region_measurement", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source))?;
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Region measurement cancelled."));
                }
                let view = app.render_task(&source, &n, &cancel)?;
                let image = image::open(&view.image_path).map_err(error)?;
                let x = (rect.x * view.width as f64).floor() as u32;
                let y = (rect.y * view.height as f64).floor() as u32;
                let width = ((rect.width * view.width as f64).round() as u32).min(view.width - x);
                let height =
                    ((rect.height * view.height as f64).round() as u32).min(view.height - y);
                if width < 16 || height < 16 {
                    return Err(AppError::new(
                        "REGION_TOO_SMALL",
                        "Choose a region at least 16 pixels wide and high.",
                    ));
                }
                let crop = image.crop_imm(x, y, width, height).resize(
                    640,
                    640,
                    image::imageops::FilterType::Triangle,
                );
                let region = GuidedRegionV1 {
                    subject,
                    id: Uuid::new_v4().to_string(),
                    preset_id: preset,
                    frame: Selection {
                        source_id: source.clone(),
                        frame_ref: view.frame_ref,
                        time_seconds: view.time_seconds,
                    },
                    rectangle: rect,
                    metrics: frameiq_analysis::measure(&crop),
                    origin: "user_drawn_untracked".into(),
                    algorithm: frameiq_analysis::ALGORITHM.into(),
                    color_policy: "reference_preview_sdr".into(),
                };
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Region measurement cancelled."));
                }
                let db = lock(&app.inner.db)?;
                let current = db.source(&source)?;
                db.validate_source(&current)?;
                if current.index_generation != region.frame.frame_ref.index_generation {
                    return Err(AppError::new(
                        "SOURCE_CHANGED",
                        "The source changed during measurement.",
                    ));
                }
                db.save_region(&region)?;
                event.result_ids = vec![region.id];
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, result, "REGION_MEASURED");
        });
        Ok(first)
    }
    pub fn projects(&self) -> AppResult<Vec<Project>> {
        lock(&self.inner.db)?.projects()
    }
    pub fn create_project(&self, name: &str) -> AppResult<Project> {
        lock(&self.inner.db)?.create_project(name)
    }
    pub fn sources(&self, project: &str) -> AppResult<Vec<SourceSummary>> {
        lock(&self.inner.db)?.sources(project)
    }
    pub fn frames(&self, source: &str, start: u64, limit: u32) -> AppResult<Vec<IndexedFrame>> {
        let db = lock(&self.inner.db)?;
        let s = db.source(source)?;
        db.frames(&s.index_generation, start, limit)
    }
    pub fn resolve(&self, source: &str, time: f64) -> AppResult<IndexedFrame> {
        let db = lock(&self.inner.db)?;
        let s = db.source(source)?;
        db.resolve(&s.index_generation, time)
    }
    pub fn selections(&self, project: &str) -> AppResult<Vec<Selection>> {
        lock(&self.inner.db)?.selections(project)
    }
    pub fn select(&self, source: &str, n: &str, selected: bool) -> AppResult<()> {
        let db = lock(&self.inner.db)?;
        let s = db.source(source)?;
        db.select(&s, n, selected)
    }
    pub fn list_jobs(&self) -> AppResult<Vec<JobEvent>> {
        lock(&self.inner.db)?.list_jobs()
    }
    pub fn get_job(&self, id: &str) -> AppResult<JobEvent> {
        lock(&self.inner.db)?.get_job(id)
    }
    pub fn cancel(&self, id: &str) -> AppResult<()> {
        {
            let jobs = lock(&self.inner.jobs)?;
            let c = jobs.get(id).ok_or_else(|| {
                AppError::new("JOB_NOT_RUNNING", "This job is no longer running.")
            })?;
            c.store(true, Ordering::SeqCst);
        }
        let mut event = self.get_job(id)?;
        if matches!(
            event.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return Err(AppError::new(
                "JOB_NOT_RUNNING",
                "This job is no longer running.",
            ));
        }
        event.status = JobStatus::Cancelling;
        event.message_code = "CANCELLING".into();
        self.emit(&event, None)
    }
    fn emit(&self, event: &JobEvent, source: Option<&str>) -> AppResult<()> {
        let cancelling = lock(&self.inner.jobs)?
            .get(&event.job_id)
            .is_some_and(|c| c.load(Ordering::SeqCst));
        let mut e = event.clone();
        if cancelling && matches!(e.status, JobStatus::Queued | JobStatus::Running) {
            e.status = JobStatus::Cancelling;
        }
        {
            let db = lock(&self.inner.db)?;
            if let Ok(previous) = db.get_job(&e.job_id) {
                e.sequence = e.sequence.max(previous.sequence.saturating_add(1));
            }
            db.save_job(&e, source)?;
        }
        (self.inner.sink)(e);
        Ok(())
    }
    fn job(&self, stage: &str, source: Option<&str>) -> AppResult<(JobEvent, Arc<AtomicBool>)> {
        let event = JobEvent {
            schema_version: SCHEMA_VERSION,
            job_id: Uuid::new_v4().to_string(),
            sequence: 0,
            status: JobStatus::Queued,
            stage: stage.into(),
            completed_units: "0".into(),
            total_units: None,
            message_code: "QUEUED".into(),
            result_ids: vec![],
            error: None,
        };
        let c = Arc::new(AtomicBool::new(false));
        lock(&self.inner.jobs)?.insert(event.job_id.clone(), c.clone());
        self.emit(&event, source)?;
        Ok((event, c))
    }
    pub fn import_video(
        &self,
        project: &str,
        path: &Path,
        stream: Option<u32>,
    ) -> AppResult<JobEvent> {
        let existing = lock(&self.inner.db)?.source_for_path(project, path)?;
        if let Some(source) = existing {
            if source.status == "ready"
                && stream.is_none()
                && lock(&self.inner.db)?.validate_source(&source).is_ok()
            {
                let (mut e, _) = self.job("indexing", Some(&source.id))?;
                e.status = JobStatus::Succeeded;
                e.completed_units = source.frame_count.clone();
                e.total_units = Some(source.frame_count);
                e.result_ids = vec![source.id.clone()];
                e.message_code = "INDEX_REUSED".into();
                self.emit(&e, Some(&source.id))?;
                lock(&self.inner.jobs)?.remove(&e.job_id);
                return self.get_job(&e.job_id);
            }
        }
        let source = lock(&self.inner.db)?.create_source(project, path)?;
        let (mut event, cancel) = self.job("indexing", Some(&source.id))?;
        event.result_ids = vec![source.id.clone()];
        self.emit(&event, Some(&source.id))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| -> AppResult<()> {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Indexing was cancelled."));
                }
                event.status = JobStatus::Running;
                event.sequence += 1;
                event.message_code = "INDEXING".into();
                app.emit(&event, Some(&source.id))?;
                let mut metadata = false;
                let mut revision = false;
                let mut last_progress = Instant::now();
                app.process(
                    WorkerRequest::Index {
                        path: source.path.clone(),
                        stream,
                    },
                    Duration::from_secs(7200),
                    || cancel.load(Ordering::SeqCst),
                    |message| {
                        match message {
                            WorkerMessage::Metadata { metadata: meta } => {
                                lock(&app.inner.db)?.set_metadata(&source, &meta)?;
                                metadata = true;
                                event.stage = "hashing".into();
                            }
                            WorkerMessage::Revision { sha256, bytes } => {
                                lock(&app.inner.db)?.set_revision(&source, &sha256, bytes)?;
                                revision = true;
                                event.stage = "indexing".into();
                            }
                            WorkerMessage::Frames { frames } => {
                                let last = frames.last().map(|f| f.ordinal.clone());
                                lock(&app.inner.db)?
                                    .append_frames(&source.index_generation, &frames)?;
                                if let Some(n) = last {
                                    event.completed_units =
                                        (n.parse::<u64>().unwrap_or(0) + 1).to_string();
                                }
                                if last_progress.elapsed() < Duration::from_millis(100) {
                                    return Ok(());
                                }
                                last_progress = Instant::now();
                            }
                            _ => {}
                        }
                        event.sequence += 1;
                        app.emit(&event, Some(&source.id))
                    },
                )?;
                if !metadata || !revision || event.completed_units == "0" {
                    return Err(AppError::new(
                        "EMPTY_VIDEO",
                        "No complete video frames were decoded.",
                    ));
                }
                lock(&app.inner.db)?.validate_source(&source)?;
                lock(&app.inner.db)?.finish_index(&source.index_generation, "ready", None)?;
                Ok(())
            })();
            match &result {
                Ok(()) => {
                    event.status = JobStatus::Succeeded;
                    event.total_units = Some(event.completed_units.clone());
                    event.message_code = "INDEX_READY".into();
                }
                Err(e) => {
                    let cancelled = e.code == "CANCELLED";
                    event.status = if cancelled {
                        JobStatus::Cancelled
                    } else {
                        JobStatus::Failed
                    };
                    event.error = Some(e.clone());
                    event.message_code = e.code.clone();
                    let _ = lock(&app.inner.db).and_then(|db| {
                        db.finish_index(
                            &source.index_generation,
                            if cancelled { "cancelled" } else { "failed" },
                            Some(e),
                        )
                    });
                }
            }
            event.sequence += 1;
            let _ = app.emit(&event, Some(&source.id));
            if let Ok(mut jobs) = lock(&app.inner.jobs) {
                jobs.remove(&event.job_id);
            }
        });
        Ok(first)
    }
    fn process(
        &self,
        request: WorkerRequest,
        timeout: Duration,
        cancel: impl Fn() -> bool,
        receive: impl FnMut(WorkerMessage) -> AppResult<()>,
    ) -> AppResult<()> {
        self.process_priority(request, timeout, cancel, receive, false)
    }
    fn process_priority(
        &self,
        request: WorkerRequest,
        timeout: Duration,
        cancel: impl Fn() -> bool,
        mut receive: impl FnMut(WorkerMessage) -> AppResult<()>,
        interactive: bool,
    ) -> AppResult<()> {
        let _cpu = self.inner.cpu.acquire(interactive, &cancel)?;
        let mut child = Command::new(&self.inner.worker)
            .env("FRAMEIQ_CPU_THREADS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                AppError::new("WORKER_LAUNCH", "The media worker could not be started.")
            })?;
        let run = (|| -> AppResult<()> {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| error("Worker stdin unavailable"))?;
            serde_json::to_writer(&mut stdin, &request).map_err(error)?;
            stdin.write_all(b"\n").map_err(error)?;
            drop(stdin);
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| error("Worker stdout unavailable"))?;
            let (tx, rx) = mpsc::sync_channel(4);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let mut line = String::new();
                    let message = match (&mut reader).take(2 * 1024 * 1024 + 1).read_line(&mut line)
                    {
                        Ok(0) => break,
                        Ok(n) if n > 2 * 1024 * 1024 => Err(AppError::new(
                            "WORKER_PROTOCOL",
                            "Worker message exceeds limit.",
                        )),
                        Ok(_) => serde_json::from_str::<WorkerMessage>(&line).map_err(error),
                        Err(e) => Err(error(e)),
                    };
                    let failed = message.is_err();
                    if tx.send(message).is_err() || failed {
                        break;
                    }
                }
            });
            let start = Instant::now();
            let mut complete = false;
            loop {
                if cancel() {
                    return Err(AppError::new("CANCELLED", "Task cancelled."));
                }
                if start.elapsed() > timeout {
                    return Err(AppError::new(
                        "WORKER_TIMEOUT",
                        "The media operation exceeded its time limit.",
                    ));
                }
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(WorkerMessage::Error { error })) => return Err(error),
                    Ok(Ok(WorkerMessage::Complete)) => {
                        complete = true;
                    }
                    Ok(Ok(message)) => receive(message)?,
                    Ok(Err(e)) => return Err(e),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let status = child.wait().map_err(error)?;
            if !status.success() || !complete {
                return Err(AppError::new(
                    "WORKER_CRASH",
                    "The media worker stopped before completing its operation.",
                ));
            }
            Ok(())
        })();
        if run.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        run
    }
    pub fn preview(&self, source_id: &str, n: &str) -> AppResult<FrameView> {
        self.render(source_id, n, false)
    }
    pub fn thumbnail(&self, source_id: &str, n: &str) -> AppResult<FrameView> {
        self.render(source_id, n, true)
    }
    fn render(&self, source_id: &str, n: &str, thumbnail: bool) -> AppResult<FrameView> {
        self.render_internal(source_id, n, thumbnail, None)
    }
    fn render_task(&self, source_id: &str, n: &str, cancel: &AtomicBool) -> AppResult<FrameView> {
        self.render_internal(source_id, n, false, Some(cancel))
    }
    fn render_internal(
        &self,
        source_id: &str,
        n: &str,
        thumbnail: bool,
        task: Option<&AtomicBool>,
    ) -> AppResult<FrameView> {
        let is_interactive = !thumbnail && task.is_none();
        let generation = if !is_interactive {
            0
        } else {
            self.inner.preview_generation.fetch_add(1, Ordering::SeqCst) + 1
        };
        let cancelled = || {
            task.is_some_and(|c| c.load(Ordering::SeqCst))
                || (is_interactive
                    && self.inner.preview_generation.load(Ordering::SeqCst) != generation)
        };
        let _lane = lock_cancellable(
            if is_interactive {
                &self.inner.interactive
            } else {
                &self.inner.background
            },
            cancelled,
        )?;
        if cancelled() {
            return Err(AppError::new("CANCELLED", "A newer frame was requested."));
        }
        let (s, f, frame_ref, anchor) = {
            let db = lock(&self.inner.db)?;
            let s = db.source(source_id)?;
            db.validate_source(&s)?;
            let f = db.frame(&s.index_generation, n)?;
            let r = db.frame_ref(&s, &f)?;
            let a = db.anchor(&s.index_generation, n)?;
            (s, f, r, a)
        };
        let path = self.inner.cache.join(format!(
            "{}-{}{}.png",
            s.index_generation,
            n,
            if thumbnail {
                "-thumb"
            } else if task.is_some() {
                "-analysis"
            } else {
                ""
            }
        ));
        let mut dimensions = None;
        if path.is_file() {
            // Dimensions are checked from the PNG header, not assumed from stream metadata.
            let mut file = File::open(&path).map_err(error)?;
            let mut header = [0u8; 24];
            file.read_exact(&mut header).map_err(error)?;
            if &header[..8] != b"\x89PNG\r\n\x1a\n" {
                fs::remove_file(&path).map_err(error)?;
            } else {
                dimensions = Some((
                    u32::from_be_bytes(header[16..20].try_into().unwrap()),
                    u32::from_be_bytes(header[20..24].try_into().unwrap()),
                ));
            }
        }
        if dimensions.is_none() {
            self.process_priority(
                WorkerRequest::Decode {
                    path: s.path.clone(),
                    stream: frame_ref.stream_index,
                    ordinal: n.parse().map_err(error)?,
                    expected_pts: f
                        .pts_ticks
                        .as_ref()
                        .map(|p| p.parse())
                        .transpose()
                        .map_err(error)?,
                    anchor,
                    output: path.to_string_lossy().into(),
                    max_width: if thumbnail { Some(160) } else { None },
                    options: ExportOptions::default(),
                },
                Duration::from_secs(30),
                cancelled,
                |m| {
                    if let WorkerMessage::Image { width, height, .. } = m {
                        dimensions = Some((width, height));
                    }
                    Ok(())
                },
                is_interactive,
            )?;
        }
        if cancelled() {
            return Err(AppError::new("CANCELLED", "Frame decoding cancelled."));
        }
        lock(&self.inner.db)?.validate_source(&s)?;
        let (w, h) = dimensions.ok_or_else(|| error("Worker returned no image"))?;
        self.prune_cache(&path)?;
        Ok(FrameView {
            frame_ref,
            time_seconds: f.time_seconds,
            width: w,
            height: h,
            image_path: path.to_string_lossy().into(),
        })
    }
    fn prune_cache(&self, keep: &Path) -> AppResult<()> {
        let mut entries = fs::read_dir(&self.inner.cache)
            .map_err(error)?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                if !m.is_file() {
                    return None;
                }
                Some((e.path(), m.len(), m.modified().ok()?))
            })
            .collect::<Vec<_>>();
        let mut total = entries.iter().map(|(_, n, _)| n).sum::<u64>();
        if total > 5 * 1024 * 1024 * 1024 {
            entries.sort_by_key(|(_, _, t)| *t);
            for (path, n, _) in entries {
                if total <= 4 * 1024 * 1024 * 1024 {
                    break;
                }
                if path != keep && fs::remove_file(path).is_ok() {
                    total -= n;
                }
            }
        }
        Ok(())
    }
    /// Read completed, versioned measurements without decoding or changing ranking.
    pub fn quality_measurements(&self, source_id: &str) -> AppResult<Vec<QualityFrameV1>> {
        let db = lock(&self.inner.db)?;
        let source = db.source(source_id)?;
        db.validate_source(&source)?;
        db.quality_frames(&source, frameiq_analysis::ALGORITHM)
    }
    pub fn recommendations(
        &self,
        source_id: &str,
        config: RankingConfigV1,
    ) -> AppResult<Option<RecommendationSetV1>> {
        let db = lock(&self.inner.db)?;
        let source = db.source(source_id)?;
        db.validate_source(&source)?;
        let Some((run, complete)) = db.latest_analysis(&source, frameiq_analysis::ALGORITHM)?
        else {
            return Ok(None);
        };
        let features = db.quality_frames(&source, frameiq_analysis::ALGORITHM)?;
        let status = db.get_job(&run)?.status;
        let set = RecommendationSetV1 {
            id: Uuid::new_v4().to_string(),
            source_id: source.id,
            index_generation: source.index_generation,
            status: serde_json::to_value(status)
                .map_err(error)?
                .as_str()
                .unwrap()
                .into(),
            coverage_complete: complete,
            frames: frameiq_analysis::rank(&features, &config)?,
            config,
        };
        db.save_recommendations(&run, &set)?;
        Ok(Some(set))
    }
    pub fn analyze(&self, source_id: &str) -> AppResult<JobEvent> {
        let source = lock(&self.inner.db)?.source(source_id)?;
        if source.status != "ready" {
            return Err(AppError::new(
                "INDEX_PENDING",
                "Finish indexing before analyzing this source.",
            ));
        }
        lock(&self.inner.db)?.validate_source(&source)?;
        if self.list_jobs()?.iter().any(|j| {
            j.stage == "analyzing"
                && j.result_ids.contains(&source.id)
                && matches!(
                    j.status,
                    JobStatus::Queued | JobStatus::Running | JobStatus::Cancelling
                )
        }) {
            return Err(AppError::new(
                "ANALYSIS_BUSY",
                "This source is already being analyzed.",
            ));
        }
        let (mut event, cancel) = self.job("analyzing", Some(source_id))?;
        event.result_ids = vec![source_id.into()];
        lock(&self.inner.db)?.begin_analysis(
            &event.job_id,
            &source,
            frameiq_analysis::ALGORITHM,
        )?;
        self.emit(&event, Some(source_id))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                let count = source.frame_count.parse::<u64>().map_err(error)?;
                if count == 0 {
                    return Err(AppError::new(
                        "NO_FRAMES",
                        "No indexed frames are available.",
                    ));
                }
                let last = lock(&app.inner.db)?
                    .frame(&source.index_generation, &(count - 1).to_string())?;
                let duration =
                    (last.time_seconds + last.duration_seconds.unwrap_or(0.001)).max(0.001);
                let samples = ((duration * 2.).ceil() as usize).clamp(1, 2000);
                let mut targets = std::collections::BTreeMap::new();
                {
                    let db = lock(&app.inner.db)?;
                    for i in 0..samples {
                        let f = db.resolve(
                            &source.index_generation,
                            duration * i as f64 / samples as f64,
                        )?;
                        targets.insert(f.ordinal.parse::<u64>().map_err(error)?, f);
                    }
                    targets.insert(count - 1, last);
                }
                event.status = JobStatus::Running;
                let mut completed = 0u64;
                for pass in 0..3 {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Analysis cancelled; completed measurements are retained.",
                        ));
                    }
                    let cached = lock(&app.inner.db)?
                        .quality_frames(&source, frameiq_analysis::ALGORITHM)?;
                    let cached_ids: std::collections::HashSet<_> = cached
                        .iter()
                        .map(|f| f.frame.frame_ref.presentation_ordinal.clone())
                        .collect();
                    let pending: Vec<_> = targets
                        .values()
                        .filter(|f| !cached_ids.contains(&f.ordinal))
                        .map(|f| {
                            Ok(AnalysisTarget {
                                ordinal: f.ordinal.parse().map_err(error)?,
                                expected_pts: f
                                    .pts_ticks
                                    .as_ref()
                                    .map(|p| p.parse())
                                    .transpose()
                                    .map_err(error)?,
                            })
                        })
                        .collect::<AppResult<_>>()?;
                    event.total_units = Some((completed + pending.len() as u64).to_string());
                    event.message_code = if pass == 0 {
                        "COARSE_SAMPLING"
                    } else {
                        "LOCAL_REFINEMENT"
                    }
                    .into();
                    app.emit(&event, None)?;
                    let mut updated = Instant::now();
                    if !pending.is_empty() {
                        app.process(
                            WorkerRequest::AnalyzeBatch {
                                path: source.path.clone(),
                                stream: source.metadata.as_ref().unwrap().selected_stream,
                                targets: pending,
                            },
                            Duration::from_secs(3600),
                            || cancel.load(Ordering::SeqCst),
                            |m| {
                                if let WorkerMessage::Quality { ordinal, metrics } = m {
                                    lock(&app.inner.db)?.record_quality(
                                        &event.job_id,
                                        &source.index_generation,
                                        &ordinal,
                                        frameiq_analysis::ALGORITHM,
                                        &metrics,
                                    )?;
                                    completed += 1;
                                    event.completed_units = completed.to_string();
                                    if updated.elapsed() >= Duration::from_millis(150) {
                                        app.emit(&event, None)?;
                                        updated = Instant::now();
                                    }
                                }
                                Ok(())
                            },
                        )?;
                    }
                    if pass == 2 {
                        break;
                    }
                    let features = lock(&app.inner.db)?
                        .quality_frames(&source, frameiq_analysis::ALGORITHM)?;
                    let shortlist = frameiq_analysis::rank(
                        &features,
                        &RankingConfigV1 {
                            count: 32,
                            ..Default::default()
                        },
                    )?;
                    targets.clear();
                    let db = lock(&app.inner.db)?;
                    for peak in shortlist {
                        let center = peak.quality.frame.time_seconds;
                        if pass == 0 {
                            for offset in -4..=4 {
                                let f = db.resolve(
                                    &source.index_generation,
                                    (center + offset as f64 / 8.).max(0.),
                                )?;
                                targets.insert(f.ordinal.parse::<u64>().map_err(error)?, f);
                            }
                        } else {
                            let first =
                                db.resolve(&source.index_generation, (center - 0.15).max(0.))?;
                            let mut n = first.ordinal.parse::<u64>().map_err(error)?;
                            while n < count && targets.len() < 20000 {
                                let f = db.frame(&source.index_generation, &n.to_string())?;
                                if f.time_seconds > center + 0.15 {
                                    break;
                                }
                                targets.insert(n, f);
                                n += 1;
                            }
                        }
                    }
                    // Refine sampled scene boundaries without full-resolution conversion.
                    if pass == 0 {
                        for pair in features.windows(2) {
                            if frameiq_analysis::similarity(
                                &pair[0].metrics.structure,
                                &pair[1].metrics.structure,
                            ) < 0.4
                            {
                                let f = db.resolve(
                                    &source.index_generation,
                                    (pair[0].frame.time_seconds + pair[1].frame.time_seconds) / 2.,
                                )?;
                                targets.insert(f.ordinal.parse::<u64>().map_err(error)?, f);
                            }
                        }
                    }
                }
                lock(&app.inner.db)?.validate_source(&source)?;
                lock(&app.inner.db)?.complete_analysis(&event.job_id)?;
                event.total_units = Some(event.completed_units.clone());
                Ok(())
            })();
            app.finish_job(&mut event, result, "ANALYSIS_COMPLETE");
        });
        Ok(first)
    }
    pub fn relocate_source(&self, source_id: &str, path: &Path) -> AppResult<JobEvent> {
        let source = lock(&self.inner.db)?.source(source_id)?;
        let path = path.canonicalize().map_err(error)?;
        let stamp = frameiq_persistence::file_stamp(&path)?;
        let (mut event, cancel) = self.job("relocating", Some(source_id))?;
        event.result_ids = vec![source_id.into()];
        self.emit(&event, Some(source_id))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source.id))?;
                let hash = hash_file(&path, || cancel.load(Ordering::SeqCst))?;
                if frameiq_persistence::file_stamp(&path)? != stamp {
                    return Err(AppError::new(
                        "SOURCE_CHANGED",
                        "The selected file changed during verification.",
                    ));
                }
                lock(&app.inner.db)?.relocate_source(&source.id, &path, &hash)?;
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, result, "SOURCE_RELOCATED");
        });
        Ok(first)
    }
    pub fn export_attempt_summary(&self, job: &str) -> AppResult<ExportAttemptSummaryV1> {
        lock(&self.inner.db)?.export_attempt_summary(job)
    }
    pub fn export_plans(&self, project: &str) -> AppResult<Vec<ExportPlanV1>> {
        lock(&self.inner.db)?.export_plans(project)
    }
    pub fn dismiss_export(&self, plan: &str) -> AppResult<()> {
        lock(&self.inner.db)?.dismiss_export(plan)
    }
    pub fn export(
        &self,
        project: &str,
        destination: &Path,
        options: ExportOptions,
    ) -> AppResult<JobEvent> {
        if !destination.is_dir() {
            return Err(AppError::new(
                "INVALID_EXPORT",
                "Choose an existing output folder.",
            ));
        }
        if !(1..=100).contains(&options.jpeg_quality) {
            return Err(AppError::new(
                "INVALID_EXPORT",
                "JPEG quality must be between 1 and 100.",
            ));
        }
        if options.color_mode == ColorMode::Preserve16Bit && options.format != ExportFormat::Png {
            return Err(AppError::new(
                "INVALID_EXPORT",
                "16-bit output requires PNG.",
            ));
        }
        let selections = self.selections(project)?;
        if selections.is_empty() {
            return Err(AppError::new(
                "NO_SELECTION",
                "Select at least one frame to export.",
            ));
        }
        let (mut event, cancel) = self.job("exporting", None)?;
        let result: AppResult<ExportPlanV1> = (|| {
            let destination = destination
                .canonicalize()
                .map_err(error)?
                .join(format!("FrameIQ-export-{}", event.job_id));
            let mut items = Vec::new();
            {
                let db = lock(&self.inner.db)?;
                for selection in selections {
                    let source = db.source(&selection.source_id)?;
                    let hash = db.revision_hash(&source)?;
                    let name = format!(
                        "{}-frame-{:010}.{}",
                        source.id,
                        selection
                            .frame_ref
                            .presentation_ordinal
                            .parse::<u64>()
                            .map_err(error)?,
                        options.format.extension()
                    );
                    items.push(ExportItemV1 {
                        source_id: source.id,
                        frame: selection,
                        source_hash: hash,
                        image_path: destination.join(name).to_string_lossy().into(),
                        checksum: None,
                        sidecar_checksum: None,
                        status: "pending".into(),
                        error: None,
                    });
                }
            }
            let plan = ExportPlanV1 {
                id: event.job_id.clone(),
                project_id: project.into(),
                destination: destination.to_string_lossy().into(),
                options,
                items,
                active: true,
            };
            lock(&self.inner.db)?.save_export_plan(&plan)?;
            fs::create_dir(&destination).map_err(error)?;
            Ok(plan)
        })();
        match result {
            Ok(plan) => self.run_export(event, cancel, plan),
            Err(e) => {
                self.finish_job(&mut event, Err(e.clone()), "EXPORT_COMPLETE");
                Err(e)
            }
        }
    }
    pub fn resume_export(&self, id: &str) -> AppResult<JobEvent> {
        // Serialize the active check and attempt reservation to prevent double resumes.
        let db = lock(&self.inner.db)?;
        let plan = db.export_plan(id)?;
        if plan.active {
            return Err(AppError::new(
                "EXPORT_BUSY",
                "This export already has an active attempt.",
            ));
        }
        let destination = Path::new(&plan.destination);
        if !destination.exists()
            && destination
                .file_name()
                .is_some_and(|n| n == std::ffi::OsStr::new(&format!("FrameIQ-export-{}", plan.id)))
            && destination.parent().is_some_and(Path::is_dir)
        {
            fs::create_dir(destination).map_err(error)?;
        }
        if !destination.is_dir() {
            return Err(AppError::new(
                "DESTINATION_MISSING",
                "Reconnect the original export folder before resuming.",
            ));
        }
        let event = JobEvent {
            schema_version: 1,
            job_id: Uuid::new_v4().to_string(),
            sequence: 0,
            status: JobStatus::Queued,
            stage: "exporting".into(),
            completed_units: "0".into(),
            total_units: Some(plan.items.len().to_string()),
            message_code: "QUEUED".into(),
            result_ids: vec![plan.destination.clone()],
            error: None,
        };
        db.save_job(&event, None)?;
        db.export_attempt(&event.job_id, id)?;
        drop(db);
        let cancel = Arc::new(AtomicBool::new(false));
        lock(&self.inner.jobs)?.insert(event.job_id.clone(), cancel.clone());
        self.run_export(event, cancel, plan)
    }
    fn finish_job(&self, event: &mut JobEvent, result: AppResult<()>, success: &str) {
        match result {
            Ok(()) => {
                event.status = JobStatus::Succeeded;
                event.message_code = success.into();
                event.error = None;
            }
            Err(e) => {
                event.status = if e.code == "CANCELLED" {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Failed
                };
                event.message_code = e.code.clone();
                event.error = Some(e);
            }
        }
        let _ = self.emit(event, None);
        if let Ok(mut jobs) = lock(&self.inner.jobs) {
            jobs.remove(&event.job_id);
        }
    }
    fn run_export(
        &self,
        mut event: JobEvent,
        cancel: Arc<AtomicBool>,
        mut plan: ExportPlanV1,
    ) -> AppResult<JobEvent> {
        event.result_ids = vec![plan.destination.clone()];
        event.total_units = Some(plan.items.len().to_string());
        self.emit(&event, None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let mut verified = std::collections::HashSet::new();
                for (i, item) in plan.items.iter_mut().enumerate() {
                    let result =
                        app.export_item(&plan.id, &plan.options, item, &cancel, &mut verified);
                    let written = match result {
                        Ok(written) => written,
                        Err(e) => {
                            item.status = "failed".into();
                            item.error = Some(e.clone());
                            let db = lock(&app.inner.db)?;
                            db.update_export_item(&plan.id, item)?;
                            db.export_attempt_outcome(
                                &event.job_id,
                                item,
                                if e.code == "CANCELLED" {
                                    "cancelled"
                                } else {
                                    "failed"
                                },
                            )?;
                            return Err(e);
                        }
                    };
                    lock(&app.inner.db)?.export_attempt_outcome(
                        &event.job_id,
                        item,
                        if written { "completed" } else { "skipped" },
                    )?;
                    event.completed_units = (i + 1).to_string();
                    event.message_code = if written {
                        "FRAME_EXPORTED"
                    } else {
                        "FRAME_SKIPPED"
                    }
                    .into();
                    app.emit(&event, None)?;
                }
                Ok(())
            })();
            app.finish_job(&mut event, result, "EXPORT_COMPLETE");
        });
        Ok(first)
    }
    fn export_item(
        &self,
        plan: &str,
        options: &ExportOptions,
        item: &mut ExportItemV1,
        cancel: &AtomicBool,
        verified: &mut std::collections::HashSet<String>,
    ) -> AppResult<bool> {
        let cancelled = || cancel.load(Ordering::SeqCst);
        if cancelled() {
            return Err(AppError::new(
                "CANCELLED",
                "Export cancelled. Completed files are retained.",
            ));
        }
        let path = Path::new(&item.image_path);
        let sidecar_path = path.with_extension(format!("{}.json", options.format.extension()));
        if path.is_file() {
            let hash = hash_file(path, cancelled)?;
            if item.checksum.as_deref() != Some(&hash) {
                return Err(AppError::new("OUTPUT_CONFLICT","An output file cannot be verified. Move it out of the export folder before resuming."));
            }
            if sidecar_path.is_file() {
                let hash = hash_file(&sidecar_path, cancelled)?;
                let conflict = || {
                    AppError::new("OUTPUT_CONFLICT", "An export sidecar differs from its saved plan. Move it out of the export folder before resuming.")
                };
                if item.sidecar_checksum.as_deref() != Some(&hash) {
                    return Err(conflict());
                }
                let json: serde_json::Value =
                    serde_json::from_slice(&fs::read(&sidecar_path).map_err(error)?)
                        .map_err(|_| conflict())?;
                if json["frameRef"] != serde_json::to_value(&item.frame.frame_ref).map_err(error)?
                    || json["options"] != serde_json::to_value(options).map_err(error)?
                    || json["sourceSha256"] != item.source_hash
                {
                    return Err(conflict());
                }
                item.status = "succeeded".into();
                item.error = None;
                lock(&self.inner.db)?.update_export_item(plan, item)?;
                return Ok(false);
            }
        } else if sidecar_path.exists() {
            if item.sidecar_checksum.as_deref() != Some(&hash_file(&sidecar_path, cancelled)?) {
                return Err(AppError::new(
                    "OUTPUT_CONFLICT",
                    "An unverified sidecar occupies the output path.",
                ));
            }
        }
        let (source, frame, anchor) = {
            let db = lock(&self.inner.db)?;
            let source = db.source(&item.source_id)?;
            db.validate_source(&source)?;
            if source.index_generation != item.frame.frame_ref.index_generation
                || source.revision_id != item.frame.frame_ref.source_revision_id
            {
                return Err(AppError::new(
                    "STALE_SELECTION",
                    "This export requires the original indexed source revision.",
                ));
            }
            let f = db.frame(
                &source.index_generation,
                &item.frame.frame_ref.presentation_ordinal,
            )?;
            let anchor = db.anchor(&source.index_generation, &f.ordinal)?;
            (source, f, anchor)
        };
        if verified.insert(source.id.clone())
            && hash_file(Path::new(&source.path), cancelled)? != item.source_hash
        {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "Source content no longer matches the export plan.",
            ));
        }
        let staging =
            tempfile::tempdir_in(self.inner.cache.join("export-staging")).map_err(error)?;
        let output = staging.path().join("image");
        let mut checksum = None;
        let mut conversion = serde_json::Value::Null;
        self.process(
            WorkerRequest::Decode {
                path: source.path.clone(),
                stream: item.frame.frame_ref.stream_index,
                ordinal: frame.ordinal.parse().map_err(error)?,
                expected_pts: frame
                    .pts_ticks
                    .as_ref()
                    .map(|p| p.parse())
                    .transpose()
                    .map_err(error)?,
                anchor,
                output: output.to_string_lossy().into(),
                max_width: None,
                options: options.clone(),
            },
            Duration::from_secs(120),
            cancelled,
            |m| {
                if let WorkerMessage::Image {
                    sha256,
                    conversion: report,
                    ..
                } = m
                {
                    checksum = Some(sha256);
                    conversion = report;
                }
                Ok(())
            },
        )?;
        lock(&self.inner.db)?.validate_source(&source)?;
        let checksum = checksum.ok_or_else(|| error("No output checksum"))?;
        if path.exists() && item.checksum.as_ref() != Some(&checksum) {
            return Err(AppError::new(
                "OUTPUT_CONFLICT",
                "Regenerated output differs from the retained image.",
            ));
        }
        let sidecar = serde_json::json!({"schemaVersion":1,"frameRef":item.frame.frame_ref,"sourceSha256":item.source_hash,"imageSha256":checksum,"sourceMetadata":source.metadata,"timeSeconds":frame.time_seconds,"options":options,"conversion":conversion});
        let bytes = serde_json::to_vec_pretty(&sidecar).map_err(error)?;
        let sidecar_hash = format!("{:x}", Sha256::digest(&bytes));
        export_checkpoint("before_checkpoint");
        // Checkpoint ownership before making either final filename visible.
        item.checksum = Some(checksum);
        item.sidecar_checksum = Some(sidecar_hash.clone());
        item.status = "pending".into();
        item.error = None;
        lock(&self.inner.db)?.update_export_item(plan, item)?;
        export_checkpoint("after_checkpoint");
        if !path.exists() {
            persist_copy(&output, path)?;
        }
        export_checkpoint("after_image");
        if !sidecar_path.exists() {
            let mut file =
                tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(error)?;
            file.write_all(&bytes).map_err(error)?;
            file.as_file().sync_all().map_err(error)?;
            file.persist_noclobber(&sidecar_path).map_err(error)?;
        } else if hash_file(&sidecar_path, cancelled)? != sidecar_hash {
            return Err(AppError::new(
                "OUTPUT_CONFLICT",
                "A sidecar occupies the output path.",
            ));
        }
        export_checkpoint("after_sidecar");
        item.status = "succeeded".into();
        lock(&self.inner.db)?.update_export_item(plan, item)?;
        export_checkpoint("after_item_checkpoint");
        Ok(true)
    }
}
// Only compiled into explicitly selected test builds. Release application builds do not enable it.
#[cfg(feature = "fault-injection")]
fn export_checkpoint(name: &str) {
    if std::env::var("FRAMEIQ_TEST_EXPORT_CRASH").as_deref() == Ok(name) {
        std::process::exit(86);
    }
}
#[cfg(not(feature = "fault-injection"))]
fn export_checkpoint(_: &str) {}
fn hash_file(path: &Path, cancel: impl Fn() -> bool) -> AppResult<String> {
    let mut file = File::open(path)
        .map_err(|e| AppError::new("SOURCE_MISSING", format!("Cannot read file: {e}")))?;
    let mut digest = Sha256::new();
    let mut bytes = vec![0u8; 1024 * 1024];
    loop {
        if cancel() {
            return Err(AppError::new("CANCELLED", "Task cancelled."));
        }
        let n = file.read(&mut bytes).map_err(error)?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn persist_copy(from: &Path, to: &Path) -> AppResult<()> {
    let mut temp = tempfile::NamedTempFile::new_in(
        to.parent()
            .ok_or_else(|| error("Missing output directory"))?,
    )
    .map_err(error)?;
    std::io::copy(&mut File::open(from).map_err(error)?, &mut temp).map_err(error)?;
    temp.as_file().sync_all().map_err(error)?;
    temp.persist_noclobber(to).map_err(error)?;
    Ok(())
}
