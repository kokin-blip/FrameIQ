//! Portable protocol double, compiled only in explicit fault-injection builds.
use std::io::{BufRead, Write};
fn main() {
    let exe = std::env::current_exe().unwrap();
    let directory = exe.parent().unwrap();
    for line in std::io::stdin().lock().lines() {
        if line.is_err() {
            break;
        }
        let mode =
            std::fs::read_to_string(directory.join("ai-worker-mode.txt")).unwrap_or_default();
        match mode.as_str() {
            "crashed" => std::process::exit(1),
            "cancelled" => {
                std::fs::write(directory.join("frameiq-ai-worker.started"), b"started").unwrap();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            "rejected" => println!("{{\"kind\":\"error\",\"error\":{{\"code\":\"MODEL_TEMPLATE\",\"message\":\"Injected validation failure\"}}}}"),
            _ => println!("{{\"kind\":\"validated\"}}"),
        }
        std::io::stdout().flush().unwrap();
    }
}
