//! Opt-in real CPU model stress; no downloads and no predicted labels as ground truth.
use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
#[path = "support/resources.rs"]
mod resources;
fn write_report(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    if path.is_symlink() {
        return Err(std::io::Error::other(
            "Report destination must not be a symbolic link",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(data.as_ref())?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn wait(app: &Application, job: &str) -> Result<JobEvent, Box<dyn std::error::Error>> {
    let start = Instant::now();
    loop {
        let e = app.get_job(job)?;
        if matches!(
            e.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return Ok(e);
        }
        if start.elapsed() > Duration::from_secs(300) {
            app.cancel(job)?;
            return Err("Stage timeout".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn succeeded(app: &Application, e: JobEvent) -> Result<JobEvent, Box<dyn std::error::Error>> {
    let e = wait(app, &e.job_id)?;
    if !matches!(e.status, JobStatus::Succeeded) {
        return Err(format!("Stage {}: {:?}", e.stage, e.error).into());
    }
    Ok(e)
}
fn bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| {
            let Ok(m) = e.metadata() else { return 0 };
            if m.is_dir() && !e.path().is_symlink() {
                bytes(&e.path())
            } else if m.is_file() {
                m.len()
            } else {
                0
            }
        })
        .sum()
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7 {
        return Err("Usage: inference-stress <video> <worker> <person.onnx> <pose.onnx> <report.json> <seconds:1..7200>".into());
    }
    let seconds = args[6].parse::<u64>()?;
    if !(1..=7200).contains(&seconds) {
        return Err("Seconds must be 1..7200".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = tempfile::tempdir()?;
    let worker = PathBuf::from(&args[2]).canonicalize()?;
    let mut sampler = resources::MemorySampler::start();
    let mut app = Application::new(data.path(), worker.clone(), Arc::new(|_| {}))?;
    let install =
        |app: &Application, file: &str, path: &str| -> Result<String, Box<dyn std::error::Error>> {
            let manifest =
                serde_json::from_slice(&std::fs::read(root.join("models/manifests").join(file))?)?;
            Ok(succeeded(app, app.import_model(Path::new(path), manifest)?)?.result_ids[0].clone())
        };
    let person = install(&app, "person_detection_mediapipe-2023mar.json", &args[3])?;
    let pose = install(&app, "pose_estimation_mediapipe-2023mar.json", &args[4])?;
    let project = app.create_project("Inference stress fixture")?;
    succeeded(
        &app,
        app.import_video(&project.id, Path::new(&args[1]), None)?,
    )?;
    let source = app.sources(&project.id)?[0].clone();
    succeeded(&app, app.detect_persons(&source.id, "0", &person)?)?;
    let people = app
        .person_feature(&source.id, "0", &person)?
        .ok_or("Missing people")?;
    if people.result.persons.is_empty() {
        return Err("Fixture needs a detected subject on frame zero".into());
    }
    app.choose_subject(&people.id, 0)?;
    succeeded(&app, app.estimate_pose(&people.id, 0, &pose)?)?;
    let pose_feature = app
        .pose_feature(&people.id, 0, &pose)?
        .ok_or("Missing pose")?;
    if pose_feature.result.pose.is_none() {
        return Err("Fixture needs a usable pose".into());
    }
    let preset = app
        .presets()?
        .into_iter()
        .find(|p| p.id == "outfit")
        .unwrap();
    let mut request = PresetExecutionV1 {
        source_id: source.id.clone(),
        preset,
        anchor_ordinal: "0".into(),
        person_model_id: person.clone(),
        person_index: 0,
        face_model_id: None,
        custom_rectangle: None,
        experimental: true,
    };
    let job = succeeded(&app, app.execute_preset(request.clone())?)?;
    let result = app
        .preset_result(&job.job_id)?
        .ok_or("Missing preset result")?;
    if result.track.observations.is_empty() {
        return Err("No tracked measurements".into());
    }
    let initial_cache = bytes(&data.path().join("cache"));
    let start = Instant::now();
    let mut cycles = 0;
    let mut timings = vec![];
    let mut cancelled = 0;
    let mut completed_races = 0;
    let mut reopenings = 0;
    let mut checkpoints = vec![];
    while cycles < 10 || start.elapsed() < Duration::from_secs(seconds) {
        let stage = Instant::now();
        // Uncached execution and real session replacement; each benchmark executes several tensor passes.
        succeeded(&app, app.benchmark_model(&person)?)?;
        succeeded(&app, app.benchmark_model(&pose)?)?;
        let pending = app.benchmark_model(&pose)?;
        let dispatched = Instant::now();
        while matches!(app.get_job(&pending.job_id)?.status, JobStatus::Queued)
            && dispatched.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(20));
        let ack = Instant::now();
        if let Err(e) = app.cancel(&pending.job_id) {
            if e.code != "JOB_NOT_RUNNING" {
                return Err(e.into());
            }
        }
        let ack_ms = ack.elapsed().as_secs_f64() * 1000.;
        let terminal = wait(&app, &pending.job_id)?;
        if ack.elapsed() > Duration::from_secs(2) {
            return Err("Cancellation exceeded two-second target".into());
        }
        match terminal.status {
            JobStatus::Cancelled => cancelled += 1,
            JobStatus::Succeeded => completed_races += 1,
            _ => return Err(format!("Cancel failed: {:?}", terminal.error).into()),
        }
        // A cancelled request kills its worker; the following real CPU operation must recover.
        succeeded(&app, app.benchmark_model(&person)?)?;
        let current = app.preview(&source.id, "0")?;
        if current.frame_ref.presentation_ordinal != "0" {
            return Err("Frame identity changed".into());
        }
        let cached = succeeded(&app, app.estimate_pose(&people.id, 0, &pose)?)?;
        if cached.result_ids != [pose_feature.id.clone()] {
            return Err("Pose cache was not reused".into());
        }
        request.preset.weights = [10., 90., 0., 0., 0.];
        let rerank = succeeded(&app, app.execute_preset(request.clone())?)?;
        let ranked = app
            .preset_result(&rerank.job_id)?
            .ok_or("Missing reranked results")?;
        if ranked.track.id != result.track.id
            || ranked.track.observations.len() != result.track.observations.len()
        {
            return Err("Weight change repeated tracking".into());
        }
        cycles += 1;
        write_report(
            &args[5],
            serde_json::to_vec_pretty(
                &serde_json::json!({"schemaVersion":1,"passed":null,"status":"running","completedCycles":cycles,"elapsedSeconds":start.elapsed().as_secs_f64(),"cancelled":cancelled,"completedCancellationRaces":completed_races,"cacheBytes":bytes(&data.path().join("cache")),"maximumObservedDescendants":sampler.maximum_descendants.load(Ordering::Relaxed)}),
            )?,
        )?;
        timings.push(stage.elapsed().as_secs_f64() * 1000.);
        if cycles % 5 == 0 {
            app.select(&source.id, "0", true)?;
            let output = data.path().join("exports");
            std::fs::create_dir_all(&output)?;
            succeeded(
                &app,
                app.export(&project.id, &output, ExportOptions::default())?,
            )?;
            drop(app);
            app = Application::new(data.path(), worker.clone(), Arc::new(|_| {}))?;
            if app.preset_result(&rerank.job_id)?.is_none() {
                return Err("Preset result lost on reopen".into());
            }
            reopenings += 1;
            checkpoints.push(serde_json::json!({"cycle":cycles,"elapsedMs":start.elapsed().as_millis(),"cacheBytes":bytes(&data.path().join("cache")),"cancelAcknowledgementMs":ack_ms}));
        }
    }
    let final_cache = bytes(&data.path().join("cache"));
    if final_cache > 5 * 1024 * 1024 * 1024 {
        return Err("Cache exceeded the five-GiB budget".into());
    }
    app.remove_model(&pose)?;
    app.remove_model(&person)?;
    if app.preset_result(&job.job_id)?.is_some() {
        return Err("Removed model left stale preset cache".into());
    }
    drop(app);
    std::thread::sleep(Duration::from_millis(200));
    let mut samples = sampler.finish();
    let mut processes = sysinfo::System::new_all();
    processes.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let survivors: Vec<_> = sampler
        .observed_pids
        .lock()
        .unwrap()
        .iter()
        .filter_map(|id| processes.process(*id))
        .filter(|p| p.name().to_string_lossy().starts_with("frameiq-"))
        .map(|p| p.pid().as_u32())
        .collect();
    if !survivors.is_empty() {
        return Err(format!("Orphan workers after shutdown: {survivors:?}").into());
    }
    samples.sort_unstable();
    let peak = samples.last().copied().unwrap_or(0);
    if peak == 0 {
        return Err("RSS sampler unavailable".into());
    }
    timings.sort_by(f64::total_cmp);
    let report = serde_json::json!({"schemaVersion":1,"passed":true,"scope":"Real CPU person/pose model stress; not accuracy, GPU or UI acceptance", "hardware":frameiq_inference::hardware(),"elapsedSeconds":start.elapsed().as_secs_f64(),"cycles":cycles,"minimumSeconds":seconds,"uncachedModelBenchmarks":cycles*3,"cancelled":cancelled,"completedCancellationRaces":completed_races,"reopenings":reopenings,"orphanWorkersAfterShutdown":0,"cachedTrackingReused":true,"trackedObservations":result.track.observations.len(),"trackStopReason":result.track.stop_reason,"presetValidation":result.validation_status,"sampledPeakProcessTreeRssBytes":peak,"maximumObservedDescendants":sampler.maximum_descendants.load(Ordering::Relaxed),"resourceCheckpoints":sampler.checkpoints.lock().unwrap().clone(),"workflowCheckpoints":checkpoints,"initialCacheBytes":initial_cache,"finalCacheBytes":final_cache,"cycleP95Ms":timings[(timings.len()*95/100).min(timings.len()-1)],"limitations":["Single-image inference; no batching claim","Cancellation may finish before dispatch and is reported separately","RSS sampling may miss short-lived peaks; shared pages can be counted twice","Real handle counts, five-minute idle unload and GPU acceptance remain separate gates","One fixture cannot validate subject tracking or preset quality"]});
    write_report(&args[5], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Completed {cycles} real inference stress cycles; report {}",
        args[5]
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7 {
        return Err("Usage: inference-stress <video> <worker> <person.onnx> <pose.onnx> <report.json> <seconds:1..7200>".into());
    }
    if let Ok(report) = Path::new(&args[5]).canonicalize() {
        for input in &args[1..5] {
            if Path::new(input).canonicalize().is_ok_and(|p| p == report) {
                return Err(
                    "Report must be distinct from input media, workers and original models".into(),
                );
            }
        }
    }
    if let Some(path) = std::env::args().nth(5) {
        write_report(
            path,
            serde_json::to_vec_pretty(
                &serde_json::json!({"schemaVersion":1,"passed":null,"status":"starting"}),
            )?,
        )?;
    }
    let result = run();
    if let Err(e) = &result {
        if let Some(path) = std::env::args().nth(5) {
            let progress = std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
            let _=write_report(path,serde_json::to_vec_pretty(&serde_json::json!({"schemaVersion":1,"passed":false,"error":e.to_string(),"partialProgress":progress,"hardware":frameiq_inference::hardware(),"scope":"Real CPU inference stress stopped; this is not passing acceptance evidence"})).unwrap());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_replacement_does_not_modify_hardlinked_original() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let report = root.path().join("report.json");
        std::fs::write(&original, b"original media").unwrap();
        std::fs::hard_link(&original, &report).unwrap();
        write_report(&report, b"{\"passed\":false}").unwrap();
        assert_eq!(std::fs::read(&original).unwrap(), b"original media");
    }
}
