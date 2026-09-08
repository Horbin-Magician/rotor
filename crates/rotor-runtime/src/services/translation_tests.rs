use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::Instant,
};

struct PendingRequest {
    headers: String,
    response: mpsc::SyncSender<(&'static str, &'static str)>,
}

struct FixtureServer {
    url: String,
    requests: mpsc::Receiver<PendingRequest>,
    worker: thread::JoinHandle<()>,
}

impl FixtureServer {
    fn start(count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/?text={{text}}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let (sender, requests) = mpsc::sync_channel(count);
        let worker = thread::spawn(move || {
            let mut handlers = Vec::new();
            for _ in 0..count {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut connection = loop {
                    match listener.accept() {
                        Ok((connection, _)) => break connection,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accepting fixture request: {error}"),
                    }
                };
                // Windows sockets can inherit nonblocking mode from accept.
                connection.set_nonblocking(false).unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                connection
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let sender = sender.clone();
                handlers.push(thread::spawn(move || {
                    let mut headers = Vec::new();
                    while !headers.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let mut buffer = [0; 1024];
                        let count = connection.read(&mut buffer).unwrap();
                        assert!(count > 0 && headers.len() + count <= 8192);
                        headers.extend_from_slice(&buffer[..count]);
                    }
                    let (response, receive) = mpsc::sync_channel(1);
                    sender.send(PendingRequest { headers: String::from_utf8(headers).unwrap(), response }).unwrap();
                    let (status, body) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
                    // A cancelled HTTP request is allowed to close its socket.
                    let _ = write!(connection, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                }));
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        Self {
            url,
            requests,
            worker,
        }
    }

    fn request(&self, text: &str) -> PendingRequest {
        let request = self.requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(request.headers.contains(&format!("text={text}")));
        request
    }
}

fn isolated_services(url: &str) -> (tempfile::TempDir, Services, Receiver<RuntimeEvent>) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = ConfigService::load_from(directory.path()).unwrap();
    config
        .set_many([
            ("translator_engine".into(), "custom".into()),
            ("translator_custom_url".into(), url.into()),
        ])
        .unwrap();
    let (services, events) = Services::new(
        Arc::new(Mutex::new(config)),
        None,
        ServiceOptions { index_files: false },
    )
    .unwrap();
    (directory, services, events)
}

fn receive(services: &Services, events: &Receiver<RuntimeEvent>) -> RuntimeEvent {
    services.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap()
    })
}

#[test]
fn superseding_and_cancelling_http_requests_preserves_the_newest_identity() {
    let server = FixtureServer::start(3);
    let (_directory, services, events) = isolated_services(&server.url);
    let old = services.translate("older".into()).unwrap();
    let first = server.request("older");
    let new = services.translate("newer".into()).unwrap();
    assert_ne!(old, new);
    let second = server.request("newer");
    services.cancel_translation_request(old);
    first
        .response
        .send(("200 OK", r#"{"translated":"obsolete"}"#))
        .unwrap();
    second
        .response
        .send(("200 OK", r#"{"translated":"最新结果"}"#))
        .unwrap();
    match receive(&services, &events) {
        RuntimeEvent::TranslationFinished { id, result } => {
            assert_eq!(id, new);
            assert_eq!(result.unwrap().translated, "最新结果");
        }
        _ => panic!("expected the newest completed translation"),
    }
    let cancelled = services.translate("cancelled".into()).unwrap();
    let third = server.request("cancelled");
    services.cancel_translation_request(cancelled);
    third
        .response
        .send(("200 OK", r#"{"translated":"must not be delivered"}"#))
        .unwrap();
    server.worker.join().unwrap();
    assert!(services
        .runtime()
        .block_on(async { tokio::time::timeout(Duration::from_millis(200), events.recv()).await })
        .is_err());
}

#[test]
fn http_failure_is_delivered_as_error_with_the_active_request_id() {
    let server = FixtureServer::start(1);
    let (_directory, services, events) = isolated_services(&server.url);
    let expected = services.translate("failure".into()).unwrap();
    server
        .request("failure")
        .response
        .send(("503 Service Unavailable", r#"{"error":"fixture only"}"#))
        .unwrap();
    match receive(&services, &events) {
        RuntimeEvent::TranslationFinished { id, result } => {
            assert_eq!(id, expected);
            assert!(result.unwrap_err().contains("503"));
        }
        _ => panic!("expected an identified translation failure"),
    }
    server.worker.join().unwrap();
}
