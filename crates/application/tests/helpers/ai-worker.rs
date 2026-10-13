//! Portable protocol double, compiled only in explicit fault-injection builds.
use std::io::{BufRead, Write};
fn main() {
    let exe = std::env::current_exe().unwrap();
    let directory = exe.parent().unwrap();
    std::fs::write(
        directory.join("ai-worker.pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let mode =
            std::fs::read_to_string(directory.join("ai-worker-mode.txt")).unwrap_or_default();
        let request: serde_json::Value = serde_json::from_str(&line).unwrap();
        if request["kind"] == "download" {
            let folder = std::path::Path::new(request["directory"].as_str().unwrap());
            std::fs::create_dir_all(folder).unwrap();
            let path = folder.join(format!(
                "{}.partial",
                request["manifest"]["sha256"].as_str().unwrap()
            ));
            std::fs::copy(directory.join("original.onnx"), &path).unwrap();
            println!(
                "{}",
                serde_json::json!({"kind":"downloaded", "path":path.to_string_lossy()})
            );
            std::io::stdout().flush().unwrap();
            continue;
        }
        if mode == "tracking" {
            let request: serde_json::Value = serde_json::from_str(&line).unwrap();
            match request["kind"].as_str().unwrap() {
                "validate" => println!("{{\"kind\":\"validated\"}}"),
                "detect_persons" => {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(directory.join("ai-worker.calls"))
                        .unwrap()
                        .write_all(b"inference\n")
                        .unwrap();
                    let (w, h) =
                        image::image_dimensions(request["image"].as_str().unwrap()).unwrap();
                    println!(
                        "{}",
                        serde_json::json!({"kind":"persons","result":{"width":w,"height":h,"algorithm":"mediapipe-person-224-v1","modelHash":request["manifest"]["sha256"],"persons":[{"detectorBox":[0.,0.,w as f64,h as f64],"landmarks":[[w as f64/2.,h as f64/2.],[w as f64/2.,0.],[w as f64/2.,h as f64/4.],[w as f64/2.,0.]],"score":0.9}]}})
                    );
                }
                _ => panic!("Unsupported protocol-double request"),
            }
            std::io::stdout().flush().unwrap();
            continue;
        }
        match mode.as_str() {
            "crashed" => std::process::exit(1),
            "cancelled" => {
                std::fs::write(directory.join("frameiq-ai-worker.started"), b"started").unwrap();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            "memory" => println!("{{\"kind\":\"error\",\"error\":{{\"code\":\"MEMORY_LIMIT\",\"message\":\"Injected memory reserve blockage\"}}}}"),
            "rejected" => println!("{{\"kind\":\"error\",\"error\":{{\"code\":\"MODEL_TEMPLATE\",\"message\":\"Injected validation failure\"}}}}"),
            _ => println!("{{\"kind\":\"validated\"}}"),
        }
        std::io::stdout().flush().unwrap();
    }
}
