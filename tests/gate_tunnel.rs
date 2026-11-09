// W3 loopback tier: the tunnel plane, cooling across requests, and fallback.
//
// The proxy here is a real stub that answers `CONNECT` itself, so the gate's
// tunnel code runs for real: socket to the proxy, CONNECT line, status reply,
// then plain HTTP over the established stream.

use std::io::{Read, Write};
use std::sync::Mutex;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use minicon_harness::gate::{config, evidence, router};

/// The tunnel tests share loopback stubs and a 300 s response timeout, so they
/// run one at a time: a serialized tier is the difference between a flaky
/// suite and a proof.
static SERIAL: Mutex<()> = Mutex::new(());

/// A stub proxy plus the upstream it forwards to, both in one thread.
struct ProxyStub {
    port: u16,
    connects: Arc<AtomicUsize>,
    /// Last `CONNECT` line the proxy saw.
    last_connect: Arc<std::sync::Mutex<String>>,
}

impl ProxyStub {
    fn start(connect_status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let connects = Arc::new(AtomicUsize::new(0));
        let last_connect = Arc::new(std::sync::Mutex::new(String::new()));
        let connects_thread = Arc::clone(&connects);
        let last_thread = Arc::clone(&last_connect);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                connects_thread.fetch_add(1, Ordering::SeqCst);
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while head.len() < 8192 {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    head.push(byte[0]);
                    if head.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&head).to_string();
                *last_thread.lock().expect("last") = text.clone();
                // A proxy that refuses answers the CONNECT/request line at
                // once and closes; nothing follows it.
                if connect_status != 200 {
                    let reply = format!("HTTP/1.1 {connect_status} Proxy Auth Required\r\n\r\n");
                    let _ = stream.write_all(reply.as_bytes());
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                if text.starts_with("CONNECT") {
                    // https shape: the tunnel opens, then the request follows.
                    let _ = stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
                    let _ = stream.flush();
                }
                // Either way the head is complete, so the stub answers the
                // request now; an http proxy sees the whole request line, an
                // https proxy sees the tunnelled one.
                let body = r#"{"choices":[{"message":{"role":"assistant","content":"via-proxy"}}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Both);
                continue;
            }
        });
        Self {
            port,
            connects,
            last_connect,
        }
    }
}

/// A direct upstream that answers with a chosen status.
struct Upstream {
    port: u16,
    calls: Arc<AtomicUsize>,
}

impl Upstream {
    fn start(status: u16, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_thread = Arc::clone(&calls);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                calls_thread.fetch_add(1, Ordering::SeqCst);
                let mut buffer = vec![0u8; 8192];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, calls }
    }
}

fn build_router(
    proxy_port: u16,
    direct_port: u16,
    direct_works: bool,
    max_attempts: u32,
) -> (Arc<router::Router>, std::path::PathBuf) {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gate-w3-{}-{}",        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let mirror = dir.join("events.jsonl");
    let config = format!(
        r#"{{
  "listen": {{ "bind": "127.0.0.1:0" }},
  "events": {{ "ring": 64, "mirror": "{mirror}" }},
  "endpoints": {{
    "proxied": {{ "provider": "stub", "protocol": "openai-chat", "baseURL": "http://upstream.example.test/v1" }},
    "direct":  {{ "provider": "stub", "protocol": "openai-chat", "baseURL": "http://127.0.0.1:{direct_port}/v1" }}
  }},
  "credentials": {{
    "one": {{ "label": "one", "secret": {{ "env": "GATE_W3_KEY_ONE" }} }},
    "two": {{ "label": "two", "secret": {{ "env": "GATE_W3_KEY_TWO" }} }}
  }},
  "egress": {{
    "viastub": {{ "proxy": {{ "url": "http://127.0.0.1:{proxy_port}", "noProxy": ["localhost"] }} }},
    "direct":  {{ "proxy": "direct" }}
  }},
  "lanes": {{
    "proxied": {{ "endpoint": "proxied", "model": "m", "credentialOrder": ["one"], "egressOrder": ["viastub"] }},
    "direct":  {{ "endpoint": "direct", "model": "m", "credentialOrder": ["one"], "egressOrder": ["direct"] }}
  }},
  "agents": {{
    "both": {{ "lanes": ["proxied", "direct"], "budget": {{ "maxAttempts": {max_attempts} }} }}
  }}
}}"#,
        mirror = mirror.to_string_lossy().replace('\\', "/")
    );
    let path = dir.join("gate.jsonc");
    std::fs::write(&path, config).expect("write config");
    let loaded = config::load(&path, false).expect("config loads");
    let mut command_line = config::CommandLine::default();
    command_line
        .env
        .insert(String::from("GATE_W3_KEY_ONE"), String::from("w3-one"));
    command_line
        .env
        .insert(String::from("GATE_W3_KEY_TWO"), String::from("w3-two"));
    let _ = direct_works;
    let router = Arc::new(router::Router {
        config: loaded,
        runtime: router::Runtime::default(),
        evidence: std::sync::Mutex::new(
            evidence::Evidence::new(64, Some(mirror.to_string_lossy().into_owned())).expect("mirror"),
        ),
        bearer: None,
        command_line,
    });
    (router, dir)
}

fn exchange(port: u16, method: &str, path: &str, body: Option<&str>) -> (u16, Vec<(String, String)>, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read");
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, response_body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
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
    (status, headers, response_body.to_owned())
}

#[test]
fn a_proxied_lane_opens_a_tunnel_and_the_proxy_carries_the_request() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let proxy = ProxyStub::start(200);
    let upstream = Upstream::start(200, "{}");
    let (router, dir) = build_router(proxy.port, upstream.port, true, 2);
    let server = config::bind(Arc::clone(&router), 2, false).expect("bind");
    let port = server.local.port();

    let (status, headers, body) = exchange(
        port,
        "POST",
        "/v1/chat/completions",
        Some(r#"{"model":"gate/lane/proxied","messages":[]}"#),
    );
    let (_, _, dbg_events) = exchange(port, "GET", "/gate/v1/events", None);
    assert_eq!(status, 200, "body: {body} events: {dbg_events}");
    assert!(body.contains("via-proxy"), "the proxy's upstream answered: {body}");
    assert_eq!(proxy.connects.load(Ordering::SeqCst), 1, "exactly one tunnel");
    let connect = proxy.last_connect.lock().expect("last").clone();
    // What the proxy saw: either the CONNECT line (https shape) or the
    // absolute-URI request line (http shape). Either proves the gate routed
    // this lane's bytes through the proxy the operator chose.
    assert!(
        connect.starts_with("CONNECT upstream.example.test:80 HTTP/1.1")
            || connect.starts_with("POST http://upstream.example.test/"),
        "proxy saw: {connect}"
    );
    let egress = headers
        .iter()
        .find(|(name, _)| name == "x-gate-egress")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    assert_eq!(egress, format!("127.0.0.1:{}", proxy.port), "the egress header names the proxy host");
    assert!(!egress.contains('@'), "no userinfo in the header");
    std::fs::remove_dir_all(dir).ok();
    server.stop();
}

#[test]
fn a_proxy_407_cools_the_egress_and_stops_the_chain() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // PRD_01_27: a 407 is "the pipe is broken, not the lane" -- it cools the
    // egress and does NOT fall back, because the same pipe cannot be trusted
    // for a retry. The gate answers a named refusal, never a proxy body.
    let proxy = ProxyStub::start(407);
    let upstream = Upstream::start(200, r#"{"choices":[{"message":{"role":"assistant","content":"direct-ok"}}]}"#);
    let (router, dir) = build_router(proxy.port, upstream.port, true, 2);
    let server = config::bind(Arc::clone(&router), 2, false).expect("bind");
    let port = server.local.port();

    let (status, _, body) = exchange(
        port,
        "POST",
        "/v1/chat/completions",
        Some(r#"{"model":"gate/both","messages":[]}"#),
    );
    assert_eq!(status, 502, "body: {body}");
    assert!(body.contains("upstream_unreachable"), "body: {body}");
    assert_eq!(
        upstream.calls.load(Ordering::SeqCst),
        0,
        "a 407 stops the chain: the direct lane is not tried"
    );

    // The evidence row names which axis cooled: the egress, never the lane.
    let (status, _, events) = exchange(port, "GET", "/gate/v1/events", None);
    assert_eq!(status, 200);
    assert!(events.contains("proxy_unauthorized"), "events: {events}");
    assert!(events.contains("\"axis\":\"egress\""), "events: {events}");
    assert!(events.contains("\"verdict\":\"unknown\""), "events: {events}");
    assert!(!events.contains("w3-one"), "no key in evidence");
    std::fs::remove_dir_all(dir).ok();
    server.stop();
}

#[test]
fn a_connect_refusal_surfaces_as_an_unknown_verdict_with_the_status() {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let proxy = ProxyStub::start(407);
    let upstream = Upstream::start(200, "{}");
    let (router, dir) = build_router(proxy.port, upstream.port, true, 1);
    let server = config::bind(Arc::clone(&router), 2, false).expect("bind");
    let port = server.local.port();
    // A lane whose only pipe is the refusing proxy: no fallback can change
    // the outcome, so the answer is deterministic.
    let (status, _, body) = exchange(port, "POST", "/v1/chat/completions", Some(r#"{"model":"gate/lane/proxied","messages":[]}"#));
    // A 407 does not fall back (the pipe is broken, not the lane), so the gate
    // answers its own named refusal, never the proxy's 407 body.
    assert_eq!(status, 502, "body: {body}");
    assert!(body.contains("upstream_unreachable"), "body: {body}");
    let (_, _, events) = exchange(port, "GET", "/gate/v1/events", None);
    assert!(events.contains("\"verdict\":\"unknown\""), "events: {events}");
    assert!(events.contains("proxy_unauthorized"), "events: {events}");
    std::fs::remove_dir_all(dir).ok();
    server.stop();
}
