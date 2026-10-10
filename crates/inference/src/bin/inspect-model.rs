use frameiq_contracts::{InferenceRequestV1, InferenceResponseV1, ModelManifestV1};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: inspect-model <manifest.json> <model.onnx>".into());
    }
    let manifest: ModelManifestV1 = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let mut engine = frameiq_inference::Engine::default();
    for request in [
        InferenceRequestV1::Validate {
            manifest: manifest.clone(),
            path: args[2].clone(),
        },
        InferenceRequestV1::Benchmark {
            manifest,
            path: args[2].clone(),
        },
    ] {
        let result = engine.handle(request);
        if let InferenceResponseV1::Error { error } = &result {
            return Err(error.to_string().into());
        }
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}
