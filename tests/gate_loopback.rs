// Loopback tier: start the real `minicon-gate`, point it at an in-process
// stub upstream, and drive the actual wire.
//
// This is the tier that proves M1 (axis round-trip), M2/M3 (precedence and
// protected fields), M8 (verdict mapping), M12 (refusals), M14 (listener
// hardening) and M15 (expansion) on a socket rather than on a struct.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use minicon_harness::gate::{config, evidence, router, server};

const TOKEN: &str = "loopback-token";

/// A stub upstream that records what it received and answers a canned script.
struct Stub {
    port: u16,
    seen: Arc<std::sync::Mutex<Vec<String>>>,
    calls: Arc<AtomicUsize>,
}

impl Stub {
    fn start(status: u16, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("stub bind");
        let port = listener.local_addr().expect("addr").port();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let seen_thread = Arc::clone(&seen);
        let calls_thread = Arc::clone(&calls);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                calls_thread.fetch_add(1, Ordering::SeqCst);
                let request = read_one(&mut stream);
                if let Some(headers) = request {
                    seen_thread.lock().expect("seen").push(headers);
                }
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, seen, calls }
    }

    fn last(&self) -> String {
        self.seen.lock().expect("seen").last().cloned().unwrap_or_default()
    }
}

fn read_one(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&buffer).to_string();
        if let Some(position) = text.find("\r\n\r\n") {
            let declared = text
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if buffer.len() >= position + 4 + declared {
                return Some(text);
            }
        }
    }
    Some(String::from_utf8_lossy(&buffer).to_string())
}

fn gate_config(stub_port: u16, dir: &std::path::Path) -> String {
    format!(
        r#"{{
  "listen": {{ "bind": "127.0.0.1:0", "auth": {{ "secret": {{ "env": "GATE_LOOPBACK_TOKEN" }} }} }},
  "events": {{ "ring": 64, "mirror": "{mirror}" }},
  "endpoints": {{ "stub": {{ "provider": "stub", "protocol": "openai-chat", "baseURL": "http://127.0.0.1:{stub_port}/v1" }} }},
  "credentials": {{
    "one": {{ "label": "one", "secret": {{ "env": "GATE_LOOPBACK_KEY_ONE" }} }},
    "two": {{ "label": "two", "secret": {{ "env": "GATE_LOOPBACK_KEY_TWO" }} }}
  }},
  "egress": {{ "direct": {{ "proxy": "direct" }} }},
  "lanes": {{ "slow": {{ "endpoint": "stub", "model": "stub-model", "credentialOrder": ["one", "two"], "egressOrder": ["direct"] }} }},
  "agents": {{ "planner": {{ "lanes": ["slow"], "budget": {{ "maxAttempts": 2 }} }} }}
}}"#,
        mirror = dir.join("events.jsonl").to_string_lossy().replace('\\', "/")
    )
}

fn start_gate(stub: &Stub) -> (server::Server, Arc<router::Router>, std::path::PathBuf) {
    // Each test gets its own directory: cargo runs tests in parallel, and a
    // shared path would have one test deleting another's config mid-run.
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "gate-loopback-{}-{serial}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("gate.jsonc");
    std::fs::write(&path, gate_config(stub.port, &dir)).expect("write config");
    let loaded = config::load(&path, false).expect("config loads");
    let mut command_line = config::CommandLine::default();
    command_line
        .env
        .insert(String::from("GATE_LOOPBACK_TOKEN"), String::from(TOKEN));
    command_line
        .env
        .insert(String::from("GATE_LOOPBACK_KEY_ONE"), String::from("secret-one"));
    command_line
        .env
        .insert(String::from("GATE_LOOPBACK_KEY_TWO"), String::from("secret-two"));
    let bearer = loaded
        .config
        .listen
        .auth
        .as_ref()
        .map(|secret| command_line.read_secret(secret, false, None).expect("token").expose().to_owned());
    let router = Arc::new(router::Router {
        config: loaded,
        runtime: router::Runtime::default(),
        evidence: std::sync::Mutex::new(
            evidence::Evidence::new(64, Some(dir.join("events.jsonl").to_string_lossy().into_owned()))
                .expect("mirror"),
        ),
        bearer,
        command_line,
    });
    let server = config::bind(Arc::clone(&router), 2, false).expect("bind");
    (server, router, path)
}

/// One raw HTTP exchange against the gate.
fn exchange(port: u16, method: &str, path: &str, host: &str, headers: &[(&str, &str)], body: Option<&str>) -> (u16, Vec<(String, String)>, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let body = body.unwrap_or("");
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
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

fn auth() -> [(&'static str, &'static str); 2] {
    [("Authorization", "Bearer loopback-token"), ("Content-Type", "application/json")]
}

const OK_BODY: &str = r#"{"id":"x","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;

#[test]
fn a_chat_request_reaches_the_stub_with_the_lane_attributes() {
    let stub = Stub::start(200, OK_BODY);
    let (gate, _router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let (status, headers, body) = exchange(
        gate.local.port(),
        "POST",
        "/v1/chat/completions",
        &host,
        &auth(),
        Some(r#"{"model":"gate/planner","messages":[{"role":"user","content":"hi"}]}"#),
    );
    assert_eq!(status, 200, "body: {body}");
    assert_eq!(body, OK_BODY, "the upstream body passes through verbatim");
    let lane = headers.iter().find(|(name, _)| name == "x-gate-lane").map(|(_, value)| value.clone());
    assert_eq!(lane.as_deref(), Some("slow"));
    // M1: the upstream saw the lane model, the first credential and the client body.
    let seen = stub.last();
    assert!(seen.contains("\"model\":\"stub-model\""), "seen: {seen}");
    assert!(seen.contains("Bearer secret-one"), "the first credential is used: {seen}");
    assert!(!seen.contains("gate/planner"), "the virtual model never reaches upstream");
    assert!(!seen.contains(TOKEN), "the gate bearer is never forwarded");
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}

#[test]
fn a_missing_bearer_is_refused_before_dispatch() {
    let stub = Stub::start(200, OK_BODY);
    let (gate, _router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let (status, _, body) = exchange(gate.local.port(), "POST", "/v1/chat/completions", &host, &[], Some("{}"));
    assert_eq!(status, 401, "body: {body}");
    assert!(body.contains("unauthorized"));
    assert_eq!(stub.calls.load(Ordering::SeqCst), 0, "nothing was dispatched");
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}

#[test]
fn protected_and_unknown_models_are_named_refusals() {
    let stub = Stub::start(200, OK_BODY);
    let (gate, _router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let (status, _, body) = exchange(
        gate.local.port(),
        "POST",
        "/v1/chat/completions",
        &host,
        &auth(),
        Some(r#"{"model":"gate/planner","messages":[],"gate_overrides":{"proxy":"http://x"}}"#),
    );
    assert_eq!(status, 400);
    assert!(body.contains("protected_field"), "body: {body}");
    let (status, _, body) = exchange(
        gate.local.port(),
        "POST",
        "/v1/chat/completions",
        &host,
        &auth(),
        Some(r#"{"model":"gpt-4","messages":[]}"#),
    );
    assert_eq!(status, 400);
    assert!(body.contains("unknown_model"), "body: {body}");
    assert_eq!(stub.calls.load(Ordering::SeqCst), 0);
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}

#[test]
fn an_upstream_status_passes_through_with_evidence_headers() {
    let stub = Stub::start(429, r#"{"error":{"message":"slow down"}}"#);
    let (gate, _router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let (status, headers, body) = exchange(
        gate.local.port(),
        "POST",
        "/v1/chat/completions",
        &host,
        &auth(),
        Some(r#"{"model":"gate/planner","messages":[]}"#),
    );
    assert_eq!(status, 429, "body: {body}");
    assert!(body.contains("slow down"));
    assert!(headers.iter().any(|(name, _)| name == "x-gate-attempt"));
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}

#[test]
fn the_inspect_surface_is_read_only_and_listener_hardened() {
    let stub = Stub::start(200, OK_BODY);
    let (gate, _router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let (status, _, body) = exchange(gate.local.port(), "GET", "/gate/v1/health", &host, &[], None);
    assert_eq!(status, 200, "body: {body}");
    let (status, _, _) = exchange(gate.local.port(), "DELETE", "/gate/v1/pool", &host, &[], None);
    assert_eq!(status, 405);
    let (status, _, _) = exchange(gate.local.port(), "GET", "/no/such/path", &host, &[], None);
    assert_eq!(status, 404);
    // DNS rebinding: a non-loopback Host is refused.
    let (status, _, _) = exchange(gate.local.port(), "GET", "/gate/v1/health", "evil.example", &[], None);
    assert_eq!(status, 400);
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}

#[test]
fn evidence_and_the_mirror_agree_and_carry_no_secret() {
    let stub = Stub::start(200, OK_BODY);
    let (gate, router, dir) = start_gate(&stub);
    let host = gate.local.ip().to_string();
    let _ = exchange(
        gate.local.port(),
        "POST",
        "/v1/chat/completions",
        &host,
        &auth(),
        Some(r#"{"model":"gate/planner","messages":[]}"#),
    );
    let (status, _, body) = exchange(gate.local.port(), "GET", "/gate/v1/events", &host, &[], None);
    assert_eq!(status, 200);
    assert!(!body.contains("secret-one") && !body.contains("secret-two"), "keys leaked: {body}");
    assert!(!body.contains(TOKEN));
    assert!(body.contains("\"credential_id\""));
    // The mirror path is the one the config names, not one this test guesses:
    // the config writes portable forward slashes.
    let mirror_path = router
        .config
        .config
        .events
        .mirror
        .clone()
        .expect("the config names a mirror");
    let mirror = std::fs::read_to_string(&mirror_path).expect("mirror");
    let ring_rows = router.evidence.lock().expect("evidence").len();
    assert!(mirror.lines().count() >= ring_rows);
    assert!(!mirror.contains("secret-one"));
    std::fs::remove_dir_all(dir).ok();
    gate.stop();
}
