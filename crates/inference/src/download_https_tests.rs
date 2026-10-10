//! Real TLS sockets; private transport injection never exists in application IPC.
use super::*;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};
const CERT: &[u8] = include_bytes!("../tests/fixtures/localhost-test-cert.pem");
const KEY: &[u8] = include_bytes!("../tests/fixtures/localhost-test-key.pem");
struct Reply {
    headers: String,
    pieces: Vec<Vec<u8>>,
    delay: Duration,
}
impl Reply {
    fn full(bytes: &[u8], tag: &str) -> Self {
        Self {
            headers: format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: {tag}\r\nConnection: close\r\n\r\n",
                bytes.len()
            ),
            pieces: vec![bytes.to_vec()],
            delay: Duration::ZERO,
        }
    }
    fn partial(bytes: &[u8], offset: usize, tag: &str) -> Self {
        Self { headers: format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {offset}-{}/{}\r\nETag: {tag}\r\nConnection: close\r\n\r\n", bytes.len()-offset, bytes.len()-1, bytes.len()), pieces: vec![bytes[offset..].to_vec()], delay: Duration::ZERO }
    }
}
struct Server {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "https://localhost:{}/model",
            listener.local_addr().unwrap().port()
        );
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (log, stopped) = (requests.clone(), stop.clone());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(CERT).unwrap()],
            PrivateKeyDer::from_pem_slice(KEY).unwrap(),
        )
        .unwrap();
        let thread = thread::spawn(move || {
            for reply in replies {
                let socket = loop {
                    if stopped.load(Ordering::SeqCst) {
                        return;
                    }
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => return,
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let connection = rustls::ServerConnection::new(Arc::new(config.clone())).unwrap();
                let mut stream = rustls::StreamOwned::new(connection, socket);
                let mut request = Vec::new();
                loop {
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    request.push(byte[0]);
                    if request.ends_with(b"\r\n\r\n") || request.len() > 8192 {
                        break;
                    }
                }
                if !request.ends_with(b"\r\n\r\n") {
                    continue;
                }
                log.lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap().to_lowercase());
                if stream
                    .write_all(reply.headers.as_bytes())
                    .and_then(|_| stream.flush())
                    .is_err()
                {
                    continue;
                }
                for piece in reply.pieces {
                    if stopped.load(Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(reply.delay);
                    if stream
                        .write_all(&piece)
                        .and_then(|_| stream.flush())
                        .is_err()
                    {
                        break;
                    }
                }
                stream.conn.send_close_notify();
                let _ = stream.flush();
                let _ = stream.sock.shutdown(std::net::Shutdown::Write);
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }
    fn transport(&self, trusted: bool, timeout: Duration) -> Transport {
        let origin = self.url.clone();
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(1))
            .read_timeout(timeout)
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() > 5
                    || attempt.url().origin() != reqwest::Url::parse(&origin).unwrap().origin()
                {
                    attempt.error("Test redirect left its TLS origin")
                } else {
                    attempt.follow()
                }
            }));
        if trusted {
            builder = builder.add_root_certificate(reqwest::Certificate::from_pem(CERT).unwrap());
        }
        Transport {
            client: builder.build().unwrap(),
            permitted: |url| url.scheme() == "https" && url.host_str() == Some("localhost"),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
// A small self-contained standard-operator graph accepted by the graph safety
// validator. Tensor/runtime compatibility remains the later activation gate.
fn bytes() -> Vec<u8> {
    b"\x3a\x0c\x0a\x0a\x22\x08Identity".to_vec()
}
fn manifest(url: &str, bytes: &[u8]) -> ModelManifestV1 {
    let mut m: ModelManifestV1 =
        serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json")).unwrap();
    m.source_url = url.into();
    m.byte_size = bytes.len().to_string();
    m.sha256 = format!("{:x}", Sha256::digest(bytes));
    m
}
fn retained(dir: &Path, m: &ModelManifestV1, bytes: &[u8], tag: &str) {
    let path = dir.join(format!("{}.partial", m.sha256));
    std::fs::write(&path, bytes).unwrap();
    std::fs::write(
        path.with_extension("json"),
        serde_json::to_vec(&serde_json::json!({"url":m.source_url,"etag":tag})).unwrap(),
    )
    .unwrap();
}
fn fetch(s: &Server, m: &ModelManifestV1, dir: &Path) -> AppResult<std::path::PathBuf> {
    download_with(
        m,
        dir,
        s.transport(true, Duration::from_secs(1)),
        || false,
        |_, _| Ok(()),
    )
}

#[test]
fn interrupted_https_body_resumes_exact_bytes_and_headers() {
    let bytes = bytes();
    let mut cut = Reply::full(&bytes, "\"v1\"");
    cut.pieces = vec![bytes[..5].to_vec()];
    let s = Server::new(vec![cut, Reply::partial(&bytes, 5, "\"v1\"")]);
    let d = tempfile::tempdir().unwrap();
    let m = manifest(&s.url, &bytes);
    assert!(fetch(&s, &m, d.path()).is_err());
    assert_eq!(
        std::fs::read(d.path().join(format!("{}.partial", m.sha256))).unwrap(),
        bytes[..5]
    );
    let path = fetch(&s, &m, d.path()).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let log = s.requests.lock().unwrap();
    assert!(!log[0].contains("range:"));
    assert!(log[1].contains("range: bytes=5-\r\n"));
    assert!(log[1].contains("if-range: \"v1\"\r\n"));
}

#[test]
fn ignored_range_and_weak_validator_restart_without_mixing_bytes() {
    for tag in ["\"old\"", "W/\"old\"", ""] {
        let b = bytes();
        let s = Server::new(vec![Reply::full(&b, "\"new\"")]);
        let d = tempfile::tempdir().unwrap();
        let m = manifest(&s.url, &b);
        retained(d.path(), &m, b"wrong", tag);
        assert_eq!(std::fs::read(fetch(&s, &m, d.path()).unwrap()).unwrap(), b);
        assert_eq!(
            s.requests.lock().unwrap()[0].contains("range: bytes=5-"),
            strong_etag(tag)
        );
    }
}

#[test]
fn malformed_ranges_changed_etags_and_http_failures_preserve_old_partial() {
    let b = bytes();
    for header in [
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 5-garbage/14\r\nContent-Length: 9\r\n\r\n",
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 5-13/14\r\nContent-Length: 9\r\nETag: \"changed\"\r\n\r\n",
        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/private\r\nContent-Length: 0\r\n\r\n",
    ] {
        let s=Server::new(vec![Reply{headers:header.into(),pieces:vec![],delay:Duration::ZERO}]);
        let d=tempfile::tempdir().unwrap();let m=manifest(&s.url,&b);retained(d.path(),&m,&b[..5],"\"v1\"");
        assert!(fetch(&s,&m,d.path()).is_err());
        assert_eq!(std::fs::read(d.path().join(format!("{}.partial",m.sha256))).unwrap(),b[..5]);
    }
}

#[test]
fn cancellation_during_https_body_retains_download_for_resume() {
    let b = bytes();
    let mut reply = Reply::full(&b, "\"v1\"");
    reply.pieces = vec![b[..5].to_vec(), b[5..].to_vec()];
    reply.delay = Duration::from_millis(80);
    let s = Server::new(vec![reply, Reply::partial(&b, 5, "\"v1\"")]);
    let d = tempfile::tempdir().unwrap();
    let m = manifest(&s.url, &b);
    let cancel = AtomicBool::new(false);
    let result = download_with(
        &m,
        d.path(),
        s.transport(true, Duration::from_secs(1)),
        || cancel.load(Ordering::SeqCst),
        |_, _| {
            cancel.store(true, Ordering::SeqCst);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err().code, "CANCELLED");
    assert_eq!(std::fs::read(fetch(&s, &m, d.path()).unwrap()).unwrap(), b);
}

#[test]
fn tls_trust_and_stalled_transfer_are_enforced() {
    let b = bytes();
    let s = Server::new(vec![Reply::full(&b, "\"v1\"")]);
    let d = tempfile::tempdir().unwrap();
    let m = manifest(&s.url, &b);
    assert!(download_with(
        &m,
        d.path(),
        s.transport(false, Duration::from_millis(100)),
        || false,
        |_, _| Ok(())
    )
    .is_err());
    assert!(!d.path().join(format!("{}.partial", m.sha256)).exists());
    let mut reply = Reply::full(&b, "\"v1\"");
    reply.delay = Duration::from_millis(200);
    let s = Server::new(vec![reply]);
    let m = manifest(&s.url, &b);
    assert!(download_with(
        &m,
        d.path(),
        s.transport(true, Duration::from_millis(50)),
        || false,
        |_, _| Ok(())
    )
    .is_err());
}

#[test]
fn progressing_body_can_outlast_the_idle_timeout() {
    let b = bytes();
    let mut reply = Reply::full(&b, "\"v1\"");
    reply.pieces = b.iter().map(|byte| vec![*byte]).collect();
    reply.delay = Duration::from_millis(50);
    let s = Server::new(vec![reply]);
    let d = tempfile::tempdir().unwrap();
    let m = manifest(&s.url, &b);
    let start = std::time::Instant::now();
    let path = download_with(
        &m,
        d.path(),
        s.transport(true, Duration::from_millis(200)),
        || false,
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(start.elapsed() > Duration::from_millis(600));
    assert_eq!(std::fs::read(path).unwrap(), b);
}

#[test]
fn oversize_checksum_and_graph_failures_cannot_activate_downloads() {
    let good = bytes();
    let mut too_large = good.clone();
    too_large.push(0);
    let chunk = format!("{:x}\r\n", too_large.len()).into_bytes();
    let reply = Reply { headers: "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n".into(), pieces: vec![[chunk, too_large, b"\r\n0\r\n\r\n".to_vec()].concat()], delay: Duration::ZERO };
    let s = Server::new(vec![reply]);
    let d = tempfile::tempdir().unwrap();
    let m = manifest(&s.url, &good);
    assert_eq!(fetch(&s, &m, d.path()).unwrap_err().code, "MODEL_DOWNLOAD");
    for (body, expected, code) in [
        (vec![0; good.len()], good, "MODEL_CHECKSUM"),
        (
            b"invalid graph".to_vec(),
            b"invalid graph".to_vec(),
            "MODEL_UNSAFE",
        ),
    ] {
        let s = Server::new(vec![Reply::full(&body, "\"v1\"")]);
        let d = tempfile::tempdir().unwrap();
        let m = manifest(&s.url, &expected);
        assert_eq!(fetch(&s, &m, d.path()).unwrap_err().code, code);
        assert!(!d.path().join(format!("{}.partial", m.sha256)).exists());
        assert!(!d.path().join(format!("{}.json", m.sha256)).exists());
    }
}
