//! Real loopback HTTP protocol checks with synthetic pixels; no external service.
use frameiq_contracts::FashionTaskV1;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
fn serve(responses: Vec<Value>) -> (u16, std::thread::JoinHandle<Vec<(String, Value)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Expected loopback request");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 1024];
            let end = loop {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                assert!(bytes.len() < 65536);
            };
            let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
            let size = head
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(size < 2 * 1024 * 1024);
            while bytes.len() < end + size {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let body = if size > 0 {
                serde_json::from_slice(&bytes[end..end + size]).unwrap()
            } else {
                Value::Null
            };
            requests.push((head.lines().next().unwrap().to_string(), body));
            let data = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",data.len(),data).unwrap();
        }
        requests
    });
    (port, handle)
}
#[test]
fn ollama_sends_the_explicit_review_image_and_returns_structured_attributes() {
    let text=json!({"items":[],"attributes":[{"name":"color","value":"blue","origin":"observed","confidence":null,"evidenceIds":[]}],"products":[]}).to_string();
    let (port, server) = serve(vec![
        json!({"models":[{"name":"local-vision"}]}),
        json!({"capabilities":["vision"]}),
        json!({"model":"local-vision:revision","response":text}),
    ]);
    let image = "reviewed-synthetic-base64".to_string();
    let result = frameiq_inference::local::analyze_images(
        "ollama",
        port,
        "local-vision",
        None,
        &[image.clone()],
        "User-reviewed source crop",
        &FashionTaskV1::Attributes,
    )
    .unwrap();
    assert_eq!(result.attributes[0].value, "blue");
    assert_eq!(result.model.as_deref(), Some("local-vision:revision"));
    let requests = server.join().unwrap();
    assert!(requests[2].0.contains("/api/generate"));
    assert_eq!(requests[2].1["images"][0], image);
    assert_eq!(requests[2].1["stream"], false);
}
#[test]
fn lm_studio_sends_image_content_after_capability_discovery() {
    let text = json!({"items":[],"attributes":[],"products":[]}).to_string();
    let (port, server) = serve(vec![
        json!({"models":[{"key":"vision","capabilities":{"vision":true}}]}),
        json!({"choices":[{"message":{"content":text}}]}),
    ]);
    frameiq_inference::local::analyze_images(
        "lm_studio",
        port,
        "vision",
        None,
        &["reviewed-pixel".into()],
        "",
        &FashionTaskV1::Detect,
    )
    .unwrap();
    let requests = server.join().unwrap();
    assert!(requests[0].0.contains("/api/v1/models"));
    assert!(requests[1].0.contains("/v1/chat/completions"));
    assert_eq!(
        requests[1].1["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,reviewed-pixel"
    );
}
#[test]
fn ollama_cloud_models_never_receive_reviewed_images() {
    let (port, server) = serve(vec![
        json!({"models":[{"name":"remote:cloud"}]}),
        json!({"capabilities":["vision"],"remote_host":"https://remote.example"}),
    ]);
    assert!(frameiq_inference::local::analyze_images(
        "ollama",
        port,
        "remote:cloud",
        None,
        &["reviewed-pixel".into()],
        "",
        &FashionTaskV1::Attributes
    )
    .is_err());
    assert_eq!(server.join().unwrap().len(), 2);
}
