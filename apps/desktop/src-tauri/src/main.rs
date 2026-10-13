mod cloud;
use frameiq_application::Application;
use frameiq_contracts::*;
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};
#[tauri::command]
fn projects(app: State<Application>) -> AppResult<Vec<Project>> {
    app.projects()
}
#[tauri::command]
fn rename_project(app: State<Application>, id: String, name: String) -> AppResult<()> {
    app.rename_project(&id, &name)
}
#[tauri::command]
fn rename_source(app: State<Application>, id: String, name: String) -> AppResult<()> {
    app.rename_source(&id, &name)
}
#[tauri::command]
fn mark_project_opened(app: State<Application>, id: String) -> AppResult<()> {
    app.mark_project_opened(&id)
}
#[tauri::command]
fn project_activity(
    app: State<Application>,
) -> AppResult<std::collections::HashMap<String, String>> {
    app.project_activity()
}
#[tauri::command]
fn model_catalog(app: State<Application>) -> AppResult<Vec<ModelManifestV1>> {
    app.model_catalog()
}
#[tauri::command]
fn create_project(app: State<Application>, name: String) -> AppResult<Project> {
    app.create_project(&name)
}
#[tauri::command]
fn sources(app: State<Application>, project: String) -> AppResult<Vec<SourceSummary>> {
    app.sources(&project)
}
#[tauri::command]
fn frames(
    app: State<Application>,
    source: String,
    start: String,
    limit: u32,
) -> AppResult<Vec<IndexedFrame>> {
    app.frames(
        &source,
        start
            .parse()
            .map_err(|_| AppError::new("INVALID_FRAME", "Invalid frame ordinal."))?,
        limit,
    )
}
#[tauri::command]
fn resolve_frame(app: State<Application>, source: String, time: f64) -> AppResult<IndexedFrame> {
    app.resolve(&source, time)
}
#[tauri::command]
async fn selections(app: State<'_, Application>, project: String) -> AppResult<Vec<Selection>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.selections(&project))
        .await
        .map_err(|_| AppError::new("SELECTION_FAILED", "Selection loading interrupted."))?
}
#[tauri::command]
fn select_frame(
    app: State<Application>,
    source: String,
    ordinal: String,
    selected: bool,
) -> AppResult<()> {
    app.select(&source, &ordinal, selected)
}
#[tauri::command]
async fn select_all_frames(app: State<'_, Application>, source: String) -> AppResult<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.select_all(&source))
        .await
        .map_err(|_| AppError::new("SELECTION_FAILED", "Selection operation interrupted."))?
}
#[tauri::command]
async fn clear_selections(app: State<'_, Application>, project: String) -> AppResult<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.clear_selections(&project))
        .await
        .map_err(|_| AppError::new("SELECTION_FAILED", "Selection operation interrupted."))?
}
#[tauri::command]
async fn add_selections(
    app: State<'_, Application>,
    project: String,
    frames: Vec<Selection>,
) -> AppResult<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.add_selections(&project, &frames))
        .await
        .map_err(|_| AppError::new("SELECTION_FAILED", "Selection operation interrupted."))?
}
#[tauri::command]
fn list_jobs(app: State<Application>) -> AppResult<Vec<JobEvent>> {
    app.list_jobs()
}
#[tauri::command]
fn get_job(app: State<Application>, id: String) -> AppResult<JobEvent> {
    app.get_job(&id)
}
#[tauri::command]
fn cancel_job(app: State<Application>, id: String) -> AppResult<()> {
    app.cancel(&id)
}
#[tauri::command]
fn import_video(
    app: State<Application>,
    project: String,
    path: PathBuf,
    stream: Option<u32>,
) -> AppResult<JobEvent> {
    app.import_video(&project, &path, stream)
}
#[tauri::command]
async fn preview_frame(
    app: State<'_, Application>,
    source: String,
    ordinal: String,
) -> AppResult<FrameView> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.preview(&source, &ordinal))
        .await
        .map_err(|e| AppError::new("INTERNAL_ERROR", e.to_string()))?
}
#[tauri::command]
async fn thumbnail(
    app: State<'_, Application>,
    source: String,
    ordinal: String,
) -> AppResult<FrameView> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.thumbnail(&source, &ordinal))
        .await
        .map_err(|e| AppError::new("INTERNAL_ERROR", e.to_string()))?
}
#[tauri::command]
fn export_frames(
    app: State<Application>,
    project: String,
    destination: PathBuf,
    options: ExportOptions,
) -> AppResult<JobEvent> {
    app.export(&project, &destination, options)
}
#[tauri::command]
fn relocate_source(app: State<Application>, source: String, path: PathBuf) -> AppResult<JobEvent> {
    app.relocate_source(&source, &path)
}
#[tauri::command]
fn export_plans(app: State<Application>, project: String) -> AppResult<Vec<ExportPlanV1>> {
    app.export_plans(&project)
}
#[tauri::command]
fn export_attempt_summary(
    app: State<Application>,
    job: String,
) -> AppResult<ExportAttemptSummaryV1> {
    app.export_attempt_summary(&job)
}
#[tauri::command]
fn resume_export(app: State<Application>, plan: String) -> AppResult<JobEvent> {
    app.resume_export(&plan)
}
#[tauri::command]
fn dismiss_export(app: State<Application>, plan: String) -> AppResult<()> {
    app.dismiss_export(&plan)
}
#[tauri::command]
fn analyze_source(app: State<Application>, source: String) -> AppResult<JobEvent> {
    app.analyze(&source)
}
#[tauri::command]
async fn recommendations(
    app: State<'_, Application>,
    source: String,
    config: RankingConfigV1,
) -> AppResult<Option<RecommendationSetV1>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.recommendations(&source, config))
        .await
        .map_err(|e| AppError::new("INTERNAL_ERROR", e.to_string()))?
}
#[tauri::command]
async fn read_model_manifest(path: PathBuf) -> AppResult<ModelManifestV1> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(path)
            .map_err(|e| AppError::new("MODEL_MANIFEST", e.to_string()))?;
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::new("MODEL_MANIFEST", e.to_string()))?;
        if bytes.len() > 65536 {
            return Err(AppError::new("MODEL_MANIFEST", "Manifest exceeds 64 KiB."));
        }
        let manifest = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::new("MODEL_MANIFEST", e.to_string()))?;
        frameiq_inference::validate_manifest(&manifest)?;
        Ok(manifest)
    })
    .await
    .map_err(|e| AppError::new("MODEL_MANIFEST", e.to_string()))?
}
#[tauri::command]
fn presets(app: State<Application>) -> AppResult<Vec<PresetV1>> {
    app.presets()
}
#[tauri::command]
fn save_preset(app: State<Application>, preset: PresetV1) -> AppResult<PresetV1> {
    app.save_preset(preset)
}
#[tauri::command]
fn regions(app: State<Application>, source: String) -> AppResult<Vec<GuidedRegionV1>> {
    app.regions(&source)
}
#[tauri::command]
fn remove_region(app: State<Application>, id: String) -> AppResult<()> {
    app.remove_region(&id)
}
#[tauri::command]
fn measure_region(
    app: State<Application>,
    source: String,
    ordinal: String,
    preset: String,
    rectangle: RegionRectV1,
) -> AppResult<JobEvent> {
    app.measure_region(&source, &ordinal, &preset, rectangle)
}
#[tauri::command]
fn execute_preset(app: State<Application>, request: PresetExecutionV1) -> AppResult<JobEvent> {
    app.execute_preset(request)
}
#[tauri::command]
fn select_preset_frames(
    app: State<Application>,
    request: PresetExecutionV1,
) -> AppResult<JobEvent> {
    app.select_preset_frames(request)
}
#[tauri::command]
fn preset_result(app: State<Application>, job: String) -> AppResult<Option<PresetResultV1>> {
    app.preset_result(&job)
}
#[tauri::command]
fn detect_faces(
    app: State<Application>,
    source: String,
    ordinal: String,
    model: String,
) -> AppResult<JobEvent> {
    app.detect_faces(&source, &ordinal, &model)
}
#[tauri::command]
fn face_feature(
    app: State<Application>,
    source: String,
    ordinal: String,
    model: String,
) -> AppResult<Option<FaceFrameV1>> {
    app.face_feature(&source, &ordinal, &model)
}
#[tauri::command]
fn choose_subject(
    app: State<Application>,
    person: String,
    index: u32,
) -> AppResult<SubjectChoiceV1> {
    app.choose_subject(&person, index)
}
#[tauri::command]
fn subject_choice(
    app: State<Application>,
    source: String,
    ordinal: String,
) -> AppResult<Option<SubjectChoiceV1>> {
    app.subject_choice(&source, &ordinal)
}
#[tauri::command]
fn clear_subject(app: State<Application>, source: String, ordinal: String) -> AppResult<()> {
    app.clear_subject(&source, &ordinal)
}
#[tauri::command]
fn detect_persons(
    app: State<Application>,
    source: String,
    ordinal: String,
    model: String,
) -> AppResult<JobEvent> {
    app.detect_persons(&source, &ordinal, &model)
}
#[tauri::command]
fn person_feature(
    app: State<Application>,
    source: String,
    ordinal: String,
    model: String,
) -> AppResult<Option<PersonFrameV1>> {
    app.person_feature(&source, &ordinal, &model)
}
#[tauri::command]
fn estimate_pose(
    app: State<Application>,
    person: String,
    index: u32,
    model: String,
) -> AppResult<JobEvent> {
    app.estimate_pose(&person, index, &model)
}
#[tauri::command]
fn pose_feature(
    app: State<Application>,
    person: String,
    index: u32,
    model: String,
) -> AppResult<Option<PoseFrameV1>> {
    app.pose_feature(&person, index, &model)
}
#[tauri::command]
fn hardware(app: State<Application>) -> HardwareProfileV1 {
    app.hardware()
}
#[tauri::command]
fn model_setup_status(app: State<Application>) -> AppResult<Vec<ModelSetupStatusV1>> {
    app.model_setup_status()
}
#[tauri::command]
fn retry_model_validation(
    app: State<Application>,
    manifest: ModelManifestV1,
) -> AppResult<JobEvent> {
    app.retry_model_validation(manifest)
}
#[tauri::command]
fn models(app: State<Application>) -> AppResult<Vec<ModelInstallationV1>> {
    app.models()
}
#[tauri::command]
fn model_benchmarks(app: State<Application>) -> AppResult<Vec<ModelBenchmarkV1>> {
    app.benchmarks()
}
#[tauri::command]
fn import_model(
    app: State<Application>,
    path: PathBuf,
    manifest: ModelManifestV1,
) -> AppResult<JobEvent> {
    app.import_model(&path, manifest)
}
#[tauri::command]
fn download_model(
    app: State<Application>,
    manifest: ModelManifestV1,
    reviewed: bool,
) -> AppResult<JobEvent> {
    app.download_model(manifest, reviewed)
}
#[tauri::command]
fn benchmark_model(app: State<Application>, id: String) -> AppResult<JobEvent> {
    app.benchmark_model(&id)
}
#[tauri::command]
fn remove_model(app: State<Application>, id: String) -> AppResult<()> {
    app.remove_model(&id)
}
fn runtime_entry(runtime: &str) -> AppResult<keyring::Entry> {
    if !["ollama", "lm_studio"].contains(&runtime) {
        return Err(AppError::new("LOCAL_RUNTIME", "Unsupported runtime."));
    }
    keyring::Entry::new("com.frameiq.runtime", runtime)
        .map_err(|_| AppError::new("CREDENTIAL_STORE", "Credential storage is unavailable."))
}
fn runtime_token(runtime: &str) -> AppResult<Option<String>> {
    match runtime_entry(runtime)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(AppError::new(
            "CREDENTIAL_STORE",
            "Unlock OS credential storage to use this runtime.",
        )),
    }
}
#[tauri::command]
async fn save_runtime_token(runtime: String, token: String) -> AppResult<()> {
    if token.len() > 8192 {
        return Err(AppError::new(
            "CREDENTIAL_STORE",
            "Token exceeds the supported size.",
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let entry = runtime_entry(&runtime)?;
        let result = if token.is_empty() {
            entry.delete_credential()
        } else {
            entry.set_password(&token)
        };
        match result {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::new(
                "CREDENTIAL_STORE",
                "The operating system could not save this credential.",
            )),
        }
    })
    .await
    .map_err(|_| AppError::new("CREDENTIAL_STORE", "Credential operation interrupted."))?
}
#[tauri::command]
async fn discover_runtime(
    app: State<'_, Application>,
    runtime: String,
    port: u16,
) -> AppResult<LocalRuntimeReportV1> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let token = runtime_token(&runtime)?;
        app.discover_runtime(runtime, port, token)
    })
    .await
    .map_err(|_| AppError::new("LOCAL_RUNTIME", "Runtime discovery interrupted."))?
}
#[tauri::command]
async fn probe_runtime(
    app: State<'_, Application>,
    runtime: String,
    port: u16,
    model: String,
) -> AppResult<f64> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let token = runtime_token(&runtime)?;
        app.probe_runtime(runtime, port, model, token)
    })
    .await
    .map_err(|_| AppError::new("LOCAL_RUNTIME", "Runtime probe interrupted."))?
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|handle| {
            let data = handle.path().app_data_dir()?;
            let worker = if let Some(path) = std::env::var_os("FRAMEIQ_WORKER") {
                PathBuf::from(path)
            } else {
                std::env::current_exe()?
                    .parent()
                    .ok_or("Executable has no directory")?
                    .join(if cfg!(windows) {
                        "frameiq-worker.exe"
                    } else {
                        "frameiq-worker"
                    })
            };
            let emitter = handle.handle().clone();
            let app = Application::new(
                &data,
                worker,
                std::sync::Arc::new(move |e| {
                    let _ = emitter.emit("frameiq://job", e);
                }),
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
            handle.manage(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            projects,
            create_project,
            rename_project,
            rename_source,
            mark_project_opened,
            project_activity,
            model_catalog,
            model_setup_status,
            retry_model_validation,
            sources,
            frames,
            resolve_frame,
            selections,
            select_frame,
            select_all_frames,
            clear_selections,
            add_selections,
            list_jobs,
            get_job,
            cancel_job,
            import_video,
            preview_frame,
            thumbnail,
            export_frames,
            relocate_source,
            export_plans,
            export_attempt_summary,
            resume_export,
            dismiss_export,
            analyze_source,
            recommendations,
            presets,
            save_preset,
            regions,
            remove_region,
            measure_region,
            execute_preset,
            select_preset_frames,
            preset_result,
            choose_subject,
            subject_choice,
            clear_subject,
            detect_persons,
            person_feature,
            estimate_pose,
            pose_feature,
            detect_faces,
            face_feature,
            read_model_manifest,
            hardware,
            models,
            model_benchmarks,
            import_model,
            download_model,
            benchmark_model,
            remove_model,
            cloud::prepare_cloud_review,
            cloud::cloud_review,
            cloud::discard_cloud_review,
            cloud::analyze_cloud_review,
            cloud::cloud_result,
            cloud::cloud_key_status,
            cloud::save_cloud_key,
            cloud::remove_cloud_key,
            cloud::test_cloud_connection,
            save_runtime_token,
            discover_runtime,
            probe_runtime
        ])
        .run(tauri::generate_context!())
        .expect("FrameIQ could not start");
}
