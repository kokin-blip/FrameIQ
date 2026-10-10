//! Opt-in pose reference artifacts. Weights and sources remain outside project storage.
use frameiq_contracts::*;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 6 {
        return Err("Usage: pose-parity manifest model source person-artifacts output".into());
    }
    let m: ModelManifestV1 = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    frameiq_inference::pose::validate_template(&m)?;
    let source = frameiq_inference::person::read_image(Path::new(&args[3]))?;
    let results: serde_json::Value =
        serde_json::from_slice(&std::fs::read(Path::new(&args[4]).join("result.json"))?)?;
    let persons: Vec<PersonDetectionV1> = serde_json::from_value(results["persons"].clone())?;
    let mut engine = frameiq_inference::Engine::default();
    for (i, p) in persons.iter().enumerate() {
        let roi = frameiq_inference::pose::Roi::from_person(p, source.width(), source.height())?;
        let data = frameiq_inference::pose::prepare(&source, &roi)?;
        let dir = Path::new(&args[5]).join(i.to_string());
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("input.json"), serde_json::to_vec(&data)?)?;
        let tensors = engine.run_prepared(&m, Path::new(&args[2]), data)?;
        let pose = frameiq_inference::pose::decode(&tensors, &roi)?;
        frameiq_inference::pose::mask(&tensors, &roi)?.save(dir.join("mask.png"))?;
        std::fs::write(dir.join("tensors.json"), serde_json::to_vec(&tensors)?)?;
        std::fs::write(
            dir.join("result.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"roi":roi,"pose":pose,"person":p,"modelHash":m.sha256,"algorithm":frameiq_inference::pose::ALGORITHM}),
            )?,
        )?;
        println!("Processed pose region {i}");
    }
    Ok(())
}
