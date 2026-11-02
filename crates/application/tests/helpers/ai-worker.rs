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
                "detect_faces" => {
                    let (w, h) =
                        image::image_dimensions(request["image"].as_str().unwrap()).unwrap();
                    let landmarks = (0..5)
                        .map(|_| serde_json::json!({"x":0.5,"y":0.2}))
                        .collect::<Vec<_>>();
                    println!(
                        "{}",
                        serde_json::json!({"kind":"faces","result":{"width":w,"height":h,"algorithm":"yunet-2023-letterbox-v1","modelHash":request["manifest"]["sha256"],"faces":[{"rectangle":{"x":0.35,"y":0.1,"width":0.3,"height":0.25},"detectionScore":0.95,"landmarks":landmarks,"inFrameFraction":1.,"allLandmarksInFrame":true}]}})
                    );
                }
                "estimate_pose" => {
                    use sha2::{Digest, Sha256};
                    let (w, h) =
                        image::image_dimensions(request["image"].as_str().unwrap()).unwrap();
                    let path = request["mask_output"].as_str().unwrap();
                    image::GrayImage::from_pixel(w, h, image::Luma([255]))
                        .save(path)
                        .unwrap();
                    let hash = format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()));
                    let points=(0..39).map(|_|serde_json::json!({"x":w as f64/2.,"y":h as f64/2.,"z":0.,"visibility":0.95,"presence":0.95})).collect::<Vec<_>>();
                    let world = (0..39)
                        .map(|i| {
                            if i == 11 {
                                [-0.2, 0., 0.]
                            } else if i == 12 {
                                [0.2, 0., 0.]
                            } else {
                                [0., 0., 0.]
                            }
                        })
                        .collect::<Vec<_>>();
                    println!(
                        "{}",
                        serde_json::json!({"kind":"pose","result":{"width":w,"height":h,"algorithm":"mediapipe-pose-roi-v1","modelHash":request["manifest"]["sha256"],"pose":{"score":0.95,"boxXyxy":[0.,0.,w as f64,h as f64],"landmarks":points,"worldLandmarks":world},"maskPath":path,"maskSha256":hash}})
                    );
                }
                "run_task_pack" => {
                    use sha2::{Digest, Sha256};
                    let p = &request["pack"];
                    let (w, h) =
                        image::image_dimensions(request["image"].as_str().unwrap()).unwrap();
                    let mask = if p["task"] == "hair" {
                        let path = request["mask_output"].as_str().unwrap();
                        image::GrayImage::from_fn(w, h, |x, y| {
                            image::Luma([
                                if x > w / 4 && x < w * 3 / 4 && y > h / 12 && y < h / 3 {
                                    255
                                } else {
                                    0
                                },
                            ])
                        })
                        .save(path)
                        .unwrap();
                        serde_json::json!({"path":path,"sha256":format!("{:x}",Sha256::digest(std::fs::read(path).unwrap())),"width":w,"height":h})
                    } else {
                        serde_json::Value::Null
                    };
                    let detections = if p["task"] == "detection" {
                        serde_json::json!([{"category":"bag","rectangle":{"x":0.6,"y":0.4,"width":0.2,"height":0.25},"score":0.9},{"category":"footwear","rectangle":{"x":0.35,"y":0.75,"width":0.3,"height":0.2},"score":0.9}])
                    } else {
                        serde_json::json!([])
                    };
                    println!(
                        "{}",
                        serde_json::json!({"kind":"task_result","result":{"packId":p["id"],"modelHash":p["artifacts"][0]["sha256"],"detections":detections,"mask":mask,"embedding":[]}})
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
