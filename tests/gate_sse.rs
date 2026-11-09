// SSE pass-through (M11): the gate relays the upstream's bytes, in order, and
// never re-frames or invents a terminator.
//
// The stub answers `text/event-stream` in several writes so the relay has more
// than one chunk to forward, and the test compares the whole body byte for
// byte.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use minicon_harness::gate::{config, evidence, router};

/// The two tests bind loopback stubs; running them one at a time keeps the
/// suite deterministic without weakening either assertion.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const EVENTS: [&str; 4] = [
    "data: {\"choices\":[{\"delta\":{\"content\":\"he\"}}]}\n\n",
    "data: {\"choices\":[{\"delta\":{\"content\":\"llo\"}}]}\n\n",
    "data: {\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n",
    "data: [DONE]\n\n",
];

fn sse_stub() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buffer = vec![0u8; 8192];
            let _ = stream.read(&mut buffer);
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
            if stream.write_all(head.as_bytes()).is_err() {
                continue;
            }
            let _ = stream.flush();
            for event in EVENTS {
                // A deliberate pause between writes: the relay must not need
                // the whole body to start forwarding.
                std::thread::sleep(std::time::Duration::from_millis(2));
                if stream.write_all(event.as_bytes()).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
        }
    });
    port
}

fn router_over(upstream_port: u16) -> (Arc<router::Router>, std::path::PathBuf) {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static SEQUENCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // A per-run sequence keeps two `cargo test` invocations (or two test
    // binaries under one job) from sharing a directory and deleting each
    // other's config.
    let run = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "gate-sse-{}-{serial}",
        std::process::id()
    ))
    .join(format!("run-{run}"));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let config = format!(
        r#"{{
  "listen": {{ "bind": "127.0.0.1:0" }},
  "endpoints": {{ "stub": {{ "provider": "stub", "protocol": "openai-chat", "baseURL": "http://127.0.0.1:{upstream_port}/v1" }} }},
  "credentials": {{ "k": {{ "label": "k", "secret": {{ "env": "GATE_SSE_KEY" }} }} }},
  "egress": {{ "direct": {{ "proxy": "direct" }} }},
  "lanes": {{ "s": {{ "endpoint": "stub", "model": "m", "credentialOrder": ["k"], "egressOrder": ["direct"] }} }},
  "agents": {{ "a": {{ "lanes": ["s"] }} }}
}}"#
    );
    let path = dir.join("gate.jsonc");
    std::fs::write(&path, config).expect("write config");
    let loaded = config::load(&path, false).expect("config loads");
    let mut command_line = config::CommandLine::default();
    command_line
        .env
        .insert(String::from("GATE_SSE_KEY"), String::from("sk-sse"));
    let router = Arc::new(router::Router {
        config: loaded,
        runtime: router::Runtime::default(),
        evidence: std::sync::Mutex::new(evidence::Evidence::new(16, None).expect("sink")),
        bearer: None,
        command_line,
    });
    (router, dir)
}

fn exchange(port: u16, body: &str) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read");
    let separator = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("head terminator");
    let head = String::from_utf8_lossy(&raw[..separator]).to_string();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        .collect();
    (status, headers, raw[separator + 4..].to_vec())
}

#[test]
fn an_sse_answer_is_relayed_byte_for_byte() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let stub = sse_stub();
    let (router, dir) = router_over(stub);
    let server = config::bind(Arc::clone(&router), 1, false).expect("bind");
    let (status, headers, body) = exchange(
        server.local.port(),
        r#"{"model":"gate/a","messages":[{"role":"user","content":"hi"}],"stream":true}"#,
    );
    assert_eq!(status, 200);
    let content_type = headers
        .iter()
        .find(|(name, _)| name == "content-type")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    assert!(content_type.contains("text/event-stream"), "content-type: {content_type}");
    assert!(
        headers.iter().any(|(name, _)| name == "x-gate-lane"),
        "evidence rides on the stream head"
    );
    // Byte-for-byte: no re-framing, no injected terminator, order preserved.
    assert_eq!(body, EVENTS.concat().into_bytes());
    // A stream has no declared length up front and closes at its end.
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "connection" && value.eq_ignore_ascii_case("close")),
        "the stream closes the connection"
    );
    let _ = std::fs::remove_dir_all(&dir);
    server.stop();
}

#[test]
fn a_non_stream_answer_still_carries_a_content_length() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        // Serve every connection until the process ends: the gate may open more
        // than one while the test runs, and a stub that accepts once races it.
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            // Read until the request head is complete, then exactly the declared
            // body: one `read` can return before the gate has written the whole
            // request, and answering early closes the socket mid-send.
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            let body_start = loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break None;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some(position) = text.find("\r\n\r\n") {
                    break Some(position + 4);
                }
            };
            if let Some(start) = body_start {
                let head = String::from_utf8_lossy(&request[..start]).to_string();
                let declared = head
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while request.len() < start + declared {
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
            }
            let body = r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    let (router, dir) = router_over(port);
    let server = config::bind(Arc::clone(&router), 1, false).expect("bind");
    let (status, headers, body) = exchange(server.local.port(), r#"{"model":"gate/a","messages":[]}"#);
    assert_eq!(status, 200);
    assert!(
        headers.iter().any(|(name, _)| name == "content-length"),
        "a buffered answer declares its length"
    );
    assert!(body.starts_with(b"{\"choices\""));
    let _ = std::fs::remove_dir_all(&dir);
    server.stop();
}
