//! Produce assets for tools/verify-yunet.py; no external calls or weight downloads.
use frameiq_contracts::*;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    if a.len() != 5 {
        return Err("Usage: face-parity <manifest> <model> <image> <output-directory>".into());
    }
    let manifest: ModelManifestV1 = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    frameiq_inference::face::validate_template(&manifest)?;
    let (prepared, g) = frameiq_inference::face::prepare(Path::new(&a[3]))?;
    let directory = Path::new(&a[4]);
    std::fs::create_dir_all(directory)?;
    let image = directory.join("prepared.png");
    prepared.save(&image)?;
    let mut engine = frameiq_inference::Engine::default();
    let tensors = engine.run(&manifest, Path::new(&a[2]), Some(&image))?;
    let faces = frameiq_inference::face::decode(&tensors, &g)?;
    std::fs::write(
        directory.join("tensors.json"),
        serde_json::to_vec(&tensors)?,
    )?;
    std::fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "geometry": { "width": g.width, "height":g.height, "resizedWidth":g.resized_width, "resizedHeight":g.resized_height },
            "faces":faces, "modelSha256":manifest.sha256, "algorithm":frameiq_inference::face::ALGORITHM
        }))?,
    )?;
    println!(
        "Produced {} face candidates for reference verification.",
        faces.len()
    );
    Ok(())
}
