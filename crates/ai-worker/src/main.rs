use frameiq_contracts::*;
use std::io::{BufRead, BufReader, Read, Write};
fn run() -> AppResult<()> {
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut engine = frameiq_inference::Engine::default();
    loop {
        let mut line = String::new();
        let n = (&mut reader)
            .take(1024 * 1024 + 1)
            .read_line(&mut line)
            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
        if n == 0 {
            break;
        }
        if n > 1024 * 1024 {
            return Err(AppError::new(
                "AI_PROTOCOL",
                "Inference request exceeds limit.",
            ));
        }
        let response = match serde_json::from_str::<InferenceRequestV1>(&line) {
            Ok(InferenceRequestV1::Download {
                manifest,
                directory,
            }) => {
                engine.handle(InferenceRequestV1::Unload);
                match frameiq_inference::download::download(
                    &manifest,
                    std::path::Path::new(&directory),
                    || false,
                    |completed, total| {
                        let mut out = std::io::stdout().lock();
                        serde_json::to_writer(
                            &mut out,
                            &InferenceResponseV1::DownloadProgress { completed, total },
                        )
                        .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
                        out.write_all(b"\n")
                            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
                        out.flush()
                            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))
                    },
                ) {
                    Ok(path) => InferenceResponseV1::Downloaded {
                        path: path.to_string_lossy().into(),
                    },
                    Err(error) => InferenceResponseV1::Error { error },
                }
            }
            Ok(InferenceRequestV1::Discover {
                runtime,
                port,
                token,
            }) => {
                engine.handle(InferenceRequestV1::Unload);
                match frameiq_inference::local::discover(&runtime, port, token.as_deref()) {
                    Ok(report) => InferenceResponseV1::RuntimeReport { report },
                    Err(error) => InferenceResponseV1::Error { error },
                }
            }
            Ok(InferenceRequestV1::Probe {
                runtime,
                port,
                model,
                token,
            }) => {
                engine.handle(InferenceRequestV1::Unload);
                match frameiq_inference::local::probe(&runtime, port, &model, token.as_deref()) {
                    Ok(latency_ms) => InferenceResponseV1::RuntimeProbe { latency_ms },
                    Err(error) => InferenceResponseV1::Error { error },
                }
            }
            Ok(request) => engine.handle(request),
            Err(e) => InferenceResponseV1::Error {
                error: AppError::new("AI_PROTOCOL", e.to_string()),
            },
        };
        let mut out = std::io::stdout().lock();
        serde_json::to_writer(&mut out, &response)
            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
        out.write_all(b"\n")
            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
        out.flush()
            .map_err(|e| AppError::new("AI_PROTOCOL", e.to_string()))?;
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e.code);
        std::process::exit(1);
    }
}
