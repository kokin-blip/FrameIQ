use super::*;
use std::process::{Child, ChildStdin};
pub(super) struct AiProcess {
    child: Child,
    stdin: ChildStdin,
    rx: mpsc::Receiver<AppResult<InferenceResponseV1>>,
}
impl Drop for AiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl AiProcess {
    fn start(worker: &Path) -> AppResult<Self> {
        let mut child = Command::new(worker)
            .env("FRAMEIQ_CPU_THREADS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AppError::new("AI_WORKER_MISSING", e.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| error("No inference stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| error("No inference stdout"))?;
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match (&mut reader)
                    .take(16 * 1024 * 1024 + 1)
                    .read_line(&mut line)
                {
                    Ok(0) => break,
                    Ok(n) => {
                        let parsed = if n > 16 * 1024 * 1024 {
                            Err(AppError::new(
                                "AI_PROTOCOL",
                                "Inference response exceeds limit.",
                            ))
                        } else {
                            serde_json::from_str(&line).map_err(error)
                        };
                        if tx.send(parsed).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(error(e)));
                        break;
                    }
                }
            }
        });
        Ok(Self { child, stdin, rx })
    }
    fn request(
        &mut self,
        request: &InferenceRequestV1,
        cancel: impl Fn() -> bool,
        mut progress: impl FnMut(u64, u64) -> AppResult<()>,
    ) -> AppResult<InferenceResponseV1> {
        serde_json::to_writer(&mut self.stdin, request).map_err(error)?;
        self.stdin.write_all(b"\n").map_err(error)?;
        self.stdin.flush().map_err(error)?;
        let start = Instant::now();
        let mut last_memory = Instant::now();
        let native_inference = matches!(
            request,
            InferenceRequestV1::Validate { .. }
                | InferenceRequestV1::Run { .. }
                | InferenceRequestV1::DetectFaces { .. }
                | InferenceRequestV1::DetectPersons { .. }
                | InferenceRequestV1::EstimatePose { .. }
                | InferenceRequestV1::Benchmark { .. }
        );
        let timeout = if matches!(request, InferenceRequestV1::Download { .. }) {
            Duration::from_secs(7200)
        } else {
            Duration::from_secs(120)
        };
        loop {
            if cancel() {
                return Err(AppError::new("CANCELLED", "Inference cancelled."));
            }
            if start.elapsed() > timeout {
                return Err(AppError::new(
                    "MODEL_TIMEOUT",
                    "Inference exceeded its time limit.",
                ));
            }
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(InferenceResponseV1::DownloadProgress { completed, total })) => {
                    progress(completed, total)?;
                }
                Ok(Ok(InferenceResponseV1::Error { error })) => return Err(error),
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(AppError::new(
                        "AI_WORKER_CRASH",
                        "Inference worker stopped.",
                    ))
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if last_memory.elapsed() < Duration::from_millis(250) {
                continue;
            }
            last_memory = Instant::now();
            let h = frameiq_inference::hardware();
            let total = h.total_memory_bytes.parse::<u64>().map_err(error)?;
            let available = h.available_memory_bytes.parse::<u64>().map_err(error)?;
            let budget = h.ai_budget_bytes.parse::<u64>().map_err(error)?;
            let pid = sysinfo::Pid::from_u32(self.child.id());
            let mut system = sysinfo::System::new();
            system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            let memory = system.process(pid).map(|p| p.memory()).unwrap_or(0);
            if (native_inference && available < total * 15 / 100)
                || memory > budget.max(256 * 1024 * 1024)
            {
                return Err(AppError::new(
                    "MEMORY_LIMIT",
                    "Inference stopped because its memory budget or system reserve was exceeded.",
                ));
            }
        }
    }
}
impl Application {
    pub(super) fn infer(
        &self,
        request: InferenceRequestV1,
        cancel: impl Fn() -> bool,
    ) -> AppResult<InferenceResponseV1> {
        self.infer_stream(request, cancel, |_, _| Ok(()))
    }
    pub(super) fn infer_stream(
        &self,
        request: InferenceRequestV1,
        cancel: impl Fn() -> bool,
        progress: impl FnMut(u64, u64) -> AppResult<()>,
    ) -> AppResult<InferenceResponseV1> {
        let mut ai = loop {
            match self.inner.ai.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    if cancel() {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Inference cancelled while queued.",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return Err(error("Inference supervisor interrupted")),
            }
        };
        let _cpu = if matches!(
            &request,
            InferenceRequestV1::Validate { .. }
                | InferenceRequestV1::Run { .. }
                | InferenceRequestV1::DetectFaces { .. }
                | InferenceRequestV1::Benchmark { .. }
        ) {
            Some(self.inner.cpu.acquire(false, &cancel)?)
        } else {
            None
        };
        if ai.is_none() {
            let worker = self.inner.worker.with_file_name(if cfg!(windows) {
                "frameiq-ai-worker.exe"
            } else {
                "frameiq-ai-worker"
            });
            *ai = Some(AiProcess::start(&worker)?);
        }
        let result = ai.as_mut().unwrap().request(&request, cancel, progress);
        if result.is_err() {
            *ai = None;
        }
        result
    }
    pub fn discover_runtime(
        &self,
        runtime: String,
        port: u16,
        token: Option<String>,
    ) -> AppResult<LocalRuntimeReportV1> {
        let (mut event, cancel) = self.job("runtime_discovery", None)?;
        event.status = JobStatus::Running;
        self.emit(&event, None)?;
        let result = self
            .infer(
                InferenceRequestV1::Discover {
                    runtime,
                    port,
                    token,
                },
                || cancel.load(Ordering::SeqCst),
            )
            .and_then(|r| {
                if let InferenceResponseV1::RuntimeReport { report } = r {
                    Ok(report)
                } else {
                    Err(error("Unexpected runtime report"))
                }
            });
        self.finish_job(
            &mut event,
            result.as_ref().map(|_| ()).map_err(Clone::clone),
            "RUNTIME_DISCOVERED",
        );
        result
    }
    pub fn probe_runtime(
        &self,
        runtime: String,
        port: u16,
        model: String,
        token: Option<String>,
    ) -> AppResult<f64> {
        let (mut event, cancel) = self.job("runtime_probe", None)?;
        event.status = JobStatus::Running;
        self.emit(&event, None)?;
        let result = self
            .infer(
                InferenceRequestV1::Probe {
                    runtime,
                    port,
                    model,
                    token,
                },
                || cancel.load(Ordering::SeqCst),
            )
            .and_then(|r| {
                if let InferenceResponseV1::RuntimeProbe { latency_ms } = r {
                    Ok(latency_ms)
                } else {
                    Err(error("Unexpected runtime probe"))
                }
            });
        self.finish_job(
            &mut event,
            result.as_ref().map(|_| ()).map_err(Clone::clone),
            "RUNTIME_PROBED",
        );
        result
    }
    pub fn face_feature(
        &self,
        source_id: &str,
        n: &str,
        model_id: &str,
    ) -> AppResult<Option<FaceFrameV1>> {
        let db = lock(&self.inner.db)?;
        let source = db.source(source_id)?;
        db.validate_source(&source)?;
        let model = db
            .model_installations()?
            .into_iter()
            .find(|m| m.id == model_id && m.status == "ready")
            .ok_or_else(|| {
                AppError::new("MODEL_MISSING", "Install a compatible face model first.")
            })?;
        frameiq_inference::face::validate_template(&model.manifest)?;
        db.face_feature(
            &source,
            n,
            &model,
            frameiq_inference::face::ALGORITHM,
            "reference_preview_sdr",
        )
    }
    /// Candidate regions only. This does not enable unvalidated automatic preset eligibility.
    pub fn detect_faces(&self, source_id: &str, n: &str, model_id: &str) -> AppResult<JobEvent> {
        let model = self
            .models()?
            .into_iter()
            .find(|m| m.id == model_id && m.status == "ready")
            .ok_or_else(|| {
                AppError::new("MODEL_MISSING", "Install a compatible face model first.")
            })?;
        frameiq_inference::face::validate_template(&model.manifest)?;
        let source = source_id.to_owned();
        let n = n.to_owned();
        // Validate the indexed frame before creating a durable job.
        {
            let db = lock(&self.inner.db)?;
            let s = db.source(&source)?;
            db.validate_source(&s)?;
            db.frame(&s.index_generation, &n)?;
        }
        let (mut event, cancel) = self.job("face_detection", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source))?;
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Face detection cancelled."));
                }
                if let Some(cached) = app.face_feature(&source, &n, &model.id)? {
                    event.result_ids = vec![cached.id];
                    return Ok(());
                }
                let view = app.render_task(&source, &n, &cancel)?;
                let response = app.infer(
                    InferenceRequestV1::DetectFaces {
                        manifest: model.manifest,
                        path: model.path,
                        image: view.image_path,
                    },
                    || cancel.load(Ordering::SeqCst),
                )?;
                let result = match response {
                    InferenceResponseV1::Faces { result } => result,
                    _ => return Err(error("Unexpected face detector response")),
                };
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Face detection cancelled."));
                }
                let feature = FaceFrameV1 {
                    id: Uuid::new_v4().to_string(),
                    model_id: model.id,
                    frame: Selection {
                        source_id: source.clone(),
                        frame_ref: view.frame_ref,
                        time_seconds: view.time_seconds,
                    },
                    color_policy: "reference_preview_sdr".into(),
                    result,
                };
                lock(&app.inner.db)?.save_face_feature(&feature)?;
                event.result_ids = vec![feature.id];
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, outcome, "FACE_CANDIDATES_READY");
        });
        Ok(first)
    }
    pub fn hardware(&self) -> HardwareProfileV1 {
        frameiq_inference::hardware()
    }
    pub fn models(&self) -> AppResult<Vec<ModelInstallationV1>> {
        lock(&self.inner.db)?.model_installations()
    }
    pub fn benchmarks(&self) -> AppResult<Vec<ModelBenchmarkV1>> {
        lock(&self.inner.db)?.benchmarks()
    }
    pub fn import_model(&self, path: &Path, manifest: ModelManifestV1) -> AppResult<JobEvent> {
        frameiq_inference::validate_manifest(&manifest)?;
        let source = path.canonicalize().map_err(error)?;
        let (mut event, cancel) = self.job("model_install", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let models = app.inner.cache.parent().unwrap().join("models");
                model_recovery::prepare_directory(&models)?;
                let staging = tempfile::Builder::new()
                    .prefix(&format!(
                        "{}{}-",
                        model_recovery::STAGING_PREFIX,
                        Uuid::new_v4()
                    ))
                    .tempdir_in(&models)
                    .map_err(error)?;
                let target = staging.path().join("model.onnx");
                // Copy in bounded chunks so cancellation and file changes cannot activate partial installs.
                let mut input = File::open(&source).map_err(error)?;
                let mut output = File::create(&target).map_err(error)?;
                let mut chunk = vec![0; 1024 * 1024];
                let mut bytes = 0;
                loop {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new("CANCELLED", "Model import cancelled."));
                    }
                    let n = input.read(&mut chunk).map_err(error)?;
                    if n == 0 {
                        break;
                    }
                    bytes += n as u64;
                    if bytes > 512 * 1024 * 1024 {
                        return Err(AppError::new(
                            "MODEL_SIZE",
                            "Model exceeds the import size limit.",
                        ));
                    }
                    output.write_all(&chunk[..n]).map_err(error)?;
                }
                output.sync_all().map_err(error)?;
                drop(output);
                app.infer(
                    InferenceRequestV1::Validate {
                        manifest: manifest.clone(),
                        path: target.to_string_lossy().into(),
                    },
                    || cancel.load(Ordering::SeqCst),
                )?;
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new(
                        "CANCELLED",
                        "Model import cancelled before activation.",
                    ));
                }
                let id = Uuid::new_v4().to_string();
                let destination = models.join(&id);
                model_recovery::checkpoint("before_activation");
                // Rename the validated directory in one operation; startup recovery
                // handles a crash between this rename and the database commit.
                fs::rename(staging.path(), &destination).map_err(error)?;
                model_recovery::checkpoint("after_activation");
                let installation = ModelInstallationV1 {
                    id: id.clone(),
                    manifest: ModelManifestV1 {
                        id: id.clone(),
                        ..manifest
                    },
                    status: "ready".into(),
                    path: destination.join("model.onnx").to_string_lossy().into(),
                    error: None,
                };
                if let Err(e) = lock(&app.inner.db).and_then(|db| db.save_model(&installation)) {
                    let _ = fs::remove_dir_all(&destination);
                    return Err(e);
                }
                model_recovery::checkpoint("after_commit");
                event.result_ids = vec![id];
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, result, "MODEL_READY");
        });
        Ok(first)
    }
    pub fn download_model(&self, manifest: ModelManifestV1, reviewed: bool) -> AppResult<JobEvent> {
        if !reviewed {
            return Err(AppError::new(
                "MODEL_REVIEW_REQUIRED",
                "Review the model source, size, license, and checksum before downloading.",
            ));
        }
        frameiq_inference::validate_manifest(&manifest)?;
        let (mut event, cancel) = self.job("model_download", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let directory = app.inner.cache.parent().unwrap().join("model-downloads");
                model_recovery::prepare_directory(&directory)?;
                let mut updated = Instant::now();
                let response = app.infer_stream(
                    InferenceRequestV1::Download {
                        manifest: manifest.clone(),
                        directory: directory.to_string_lossy().into(),
                    },
                    || cancel.load(Ordering::SeqCst),
                    |done, total| {
                        event.completed_units = done.to_string();
                        event.total_units = Some(total.to_string());
                        if updated.elapsed() > Duration::from_millis(150) {
                            app.emit(&event, None)?;
                            updated = Instant::now();
                        }
                        Ok(())
                    },
                )?;
                let path = if let InferenceResponseV1::Downloaded { path } = response {
                    PathBuf::from(path)
                } else {
                    return Err(error("Unexpected download response"));
                };
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Download cancelled."));
                }
                let validation = app.import_model(&path, manifest)?;
                event.result_ids = vec![validation.job_id];
                Ok(())
            })();
            app.finish_job(&mut event, result, "DOWNLOAD_COMPLETE_VALIDATING");
        });
        Ok(first)
    }
    pub fn benchmark_model(&self, id: &str) -> AppResult<JobEvent> {
        let model = self
            .models()?
            .into_iter()
            .find(|m| m.id == id && m.status == "ready")
            .ok_or_else(|| AppError::new("MODEL_MISSING", "Install a validated model first."))?;
        let (mut event, cancel) = self.job("model_benchmark", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let response = app.infer(
                    InferenceRequestV1::Benchmark {
                        manifest: model.manifest,
                        path: model.path,
                    },
                    || cancel.load(Ordering::SeqCst),
                )?;
                if let InferenceResponseV1::Benchmark { result } = response {
                    lock(&app.inner.db)?.save_benchmark(&result)?;
                    event.result_ids = vec![result.model_id];
                    event.completed_units = "1".into();
                    event.total_units = Some("1".into());
                    Ok(())
                } else {
                    Err(error("Unexpected benchmark response"))
                }
            })();
            app.finish_job(&mut event, result, "BENCHMARK_COMPLETE");
        });
        Ok(first)
    }
    pub fn remove_model(&self, id: &str) -> AppResult<()> {
        let _body = self.inner.body.try_lock().map_err(|_| {
            AppError::new(
                "MODEL_BUSY",
                "Cancel the active region operation before removing a model.",
            )
        })?;
        let model = self
            .models()?
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| AppError::new("MODEL_MISSING", "Model is not installed."))?;
        let mut ai = self.inner.ai.try_lock().map_err(|_| {
            AppError::new(
                "MODEL_BUSY",
                "Cancel the active model operation before removing an installation.",
            )
        })?;
        *ai = None;
        // Only delete paths created by the installation service, never the original imported file.
        let owned = self
            .inner
            .cache
            .parent()
            .unwrap()
            .join("models")
            .join(&model.id);
        if Path::new(&model.path) != owned.join("model.onnx") {
            return Err(error("Invalid managed model path"));
        }
        model_recovery::prepare_directory(owned.parent().unwrap())?;
        if owned.is_symlink() {
            return Err(AppError::new(
                "MODEL_STORAGE",
                "Model removal cannot follow a redirected installation.",
            ));
        }
        let db = lock(&self.inner.db)?;
        model_recovery::checkpoint("before_removal_commit");
        let masks = db.remove_model_transaction(id)?;
        model_recovery::checkpoint("after_removal_commit");
        for path in masks {
            self.remove_body_mask(&path);
        }
        model_recovery::remove_installation(&owned)?;
        model_recovery::checkpoint("after_removal_files");
        Ok(())
    }
}
