//! Development-only reference artifacts; no downloads or project mutations.
use frameiq_contracts::*;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("Usage: person-parity <manifest> <model> <image> <output-directory>".into());
    }
    let m: ModelManifestV1 = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    frameiq_inference::person::validate_template(&m)?;
    let mut reader = image::ImageReader::open(&args[3])?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let source = reader.decode()?.to_rgb8();
    let (data, g) = frameiq_inference::person::prepare(&source)?;
    let dir = Path::new(&args[4]);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("input.json"), serde_json::to_vec(&data)?)?;
    let t = frameiq_inference::Engine::default().run_prepared(&m, Path::new(&args[2]), data)?;
    let persons = frameiq_inference::person::decode(&t, &g)?;
    let rois: Vec<_> = persons
        .iter()
        .map(|p| {
            frameiq_inference::pose::Roi::from_person(p, g.width, g.height)
                .ok()
                .map(|r| {
                    let points = [[0., 0.], [128., 128.], [256., 256.]]
                        .map(|point| r.tensor_to_source(point));
                    serde_json::json!({"geometry":r,"sourcePoints":points})
                })
        })
        .collect();
    std::fs::write(dir.join("tensors.json"), serde_json::to_vec(&t)?)?;
    std::fs::write(
        dir.join("result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"geometry":g,"persons":persons,"poseRois":rois,"modelSha256":m.sha256,"algorithm":frameiq_inference::person::ALGORITHM}),
        )?,
    )?;
    println!("Produced {} person candidates", persons.len());
    Ok(())
}
