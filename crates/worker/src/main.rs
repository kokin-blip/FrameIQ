use frameiq_contracts::*;
use std::io::{BufRead, Write};
fn emit(message: WorkerMessage) -> AppResult<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &message)
        .map_err(|e| AppError::new("WORKER_PROTOCOL", e.to_string()))?;
    out.write_all(b"\n")
        .map_err(|e| AppError::new("WORKER_PROTOCOL", e.to_string()))?;
    out.flush()
        .map_err(|e| AppError::new("WORKER_PROTOCOL", e.to_string()))
}
fn run() -> AppResult<()> {
    let mut line = String::new();
    std::io::Read::take(std::io::stdin().lock(), 1024 * 1024 + 1)
        .read_line(&mut line)
        .map_err(|e| AppError::new("WORKER_PROTOCOL", e.to_string()))?;
    if line.len() > 1024 * 1024 {
        return Err(AppError::new("WORKER_PROTOCOL", "Request exceeds limit."));
    }
    let request: WorkerRequest =
        serde_json::from_str(&line).map_err(|e| AppError::new("WORKER_PROTOCOL", e.to_string()))?;
    match request {
        WorkerRequest::Index { path, stream } => frameiq_media::index(&path, stream, emit),
        WorkerRequest::AnalyzeBatch {
            path,
            stream,
            targets,
        } => frameiq_media::analyze_batch(&path, stream, &targets, emit),
        WorkerRequest::Decode {
            path,
            stream,
            ordinal,
            expected_pts,
            anchor,
            output,
            max_width,
            options,
        } => {
            let message = frameiq_media::decode(
                &path,
                stream,
                ordinal,
                expected_pts,
                anchor.as_ref(),
                std::path::Path::new(&output),
                max_width,
                &options,
            )?;
            emit(message)?;
            emit(WorkerMessage::Complete)
        }
    }
}
fn main() {
    if let Err(error) = run() {
        let _ = emit(WorkerMessage::Error { error });
        std::process::exit(1);
    }
}
