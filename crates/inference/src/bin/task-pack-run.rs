use frameiq_contracts::*;
use std::path::Path;
fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", e.code, e.message);
        std::process::exit(1)
    }
}
fn run() -> AppResult<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "--hardware") {
        println!(
            "{}",
            serde_json::to_string_pretty(&frameiq_inference::hardware()).unwrap()
        );
        return Ok(());
    }
    let manifest = args.first().ok_or_else(|| {
        AppError::new(
            "USAGE",
            "task-pack-run <manifest.json> [image] [mask-output]",
        )
    })?;
    let path = Path::new(manifest);
    let pack: TaskPackV2 = serde_json::from_slice(
        &std::fs::read(path).map_err(|e| AppError::new("TASK_PACK", e.to_string()))?,
    )
    .map_err(|e| AppError::new("TASK_PACK", e.to_string()))?;
    let mut engine = frameiq_inference::Engine::default();
    let result = engine.task_pack(
        &pack,
        path.parent().unwrap_or(Path::new(".")),
        args.get(1).map(String::as_str),
        None,
        args.get(2).map(String::as_str),
    )?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}
