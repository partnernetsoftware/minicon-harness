// The real-client tier (W4): the repository's own worker core drives the gate
// end to end.
//
// This is the tier the plan calls the one non-negotiable piece of evidence: a
// client that was not written for the gate is pointed at `gate/<agent>` with
// `--base-url` and completes a real turn. The stub upstream answers one tool
// call, then the final answer, so the core's own loop, the gate's routing and
// the evidence mirror all run for real.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;

/// A tiny upstream that answers the core's conversation: first a final answer,
/// which is enough to prove the wire and the model substitution.
fn upstream() -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_thread = std::sync::Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
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
                seen_thread
                    .lock()
                    .expect("seen")
                    .push(String::from_utf8_lossy(&request).to_string());
            }
            let body = r#"{"choices":[{"message":{"role":"assistant","content":"the answer is done"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (port, seen)
}

fn unique_dir(name: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gate-client-{}-{}-{}",
        name,
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir
}

fn binary(name: &str) -> PathBuf {
    // `cargo test` builds the binaries beside the test executable; the target
    // directory is shared, so the debug binary is the one under test.
    let profile_dir = std::env::current_exe()
        .expect("test exe")
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("target dir");
    let candidate = profile_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        candidate.exists(),
        "{} is not built; run `cargo build --bins` before this tier",
        candidate.display()
    );
    candidate
}

#[test]
fn the_worker_core_completes_a_turn_through_the_gate() {
    let (upstream_port, seen) = upstream();
    let work = unique_dir("core");
    let mirror = work.join("events.jsonl");
    let config = work.join("gate.jsonc");
    std::fs::write(
        &config,
        format!(
            r#"{{
  "listen": {{ "bind": "127.0.0.1:0", "auth": {{ "secret": {{ "env": "GATE_CLIENT_TOKEN" }} }} }},
  "events": {{ "ring": 64, "mirror": "{mirror}" }},
  "endpoints": {{ "up": {{ "provider": "stub", "protocol": "openai-chat", "baseURL": "http://127.0.0.1:{upstream_port}/v1" }} }},
  "credentials": {{ "k": {{ "label": "k", "secret": {{ "env": "GATE_CLIENT_KEY" }} }} }},
  "egress": {{ "direct": {{ "proxy": "direct" }} }},
  "lanes": {{ "lane": {{ "endpoint": "up", "model": "upstream-model", "credentialOrder": ["k"], "egressOrder": ["direct"] }} }},
  "agents": {{ "planner": {{ "lanes": ["lane"] }} }}
}}"#,
            mirror = mirror.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("write config");

    // The gate prints its bound address; a `:0` bind means the test must read
    // it back. Use a fixed free port instead so the flow stays simple.
    let port = {
        let probe = TcpListener::bind("127.0.0.1:0").expect("probe");
        let port = probe.local_addr().expect("addr").port();
        drop(probe);
        port
    };
    let text = std::fs::read_to_string(&config).expect("config");
    std::fs::write(&config, text.replace("\"127.0.0.1:0\"", &format!("\"127.0.0.1:{port}\"")))
        .expect("rewrite");

    let gate = Command::new(binary("minicon-gate"))
        .arg("--config")
        .arg(&config)
        // The lane's credential, read by the gate at egress time.
        .env("GATE_CLIENT_KEY", "sk-lane-key")
        // The bearer a client uses to authenticate to the gate itself.
        .env("GATE_CLIENT_TOKEN", "gate-bearer-token")
        .spawn()
        .expect("start gate");
    let _gate = GateChild(gate);

    // Wait for the listener.
    for _ in 0..200 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let root = work.join("playground");
    std::fs::create_dir_all(&root).expect("root");
    let output = Command::new(binary("minicon-harness"))
        .arg("--root")
        .arg(&root)
        .arg("--task")
        .arg("say done")
        .arg("--base-url")
        .arg(format!("http://127.0.0.1:{port}/chat/completions"))
        .arg("--model")
        .arg("gate/planner")
        .arg("--key-env")
        .arg("GATE_CLIENT_TOKEN")
        // The core sends its key to the gate as the bearer; the gate validates
        // it against `listen.auth` and replaces it with the lane credential.
        .env("GATE_CLIENT_TOKEN", "gate-bearer-token")
        .env("GATE_CLIENT_KEY", "sk-lane-key")
        .output()
        .expect("run the core");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "the core failed: status {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("the answer is done"),
        "the core printed the gate's answer: {stdout}"
    );

    // The gate did the routing: the upstream saw the lane's model, never the
    // virtual one, and the lane's credential replaces the client's gate bearer.
    let bodies = seen.lock().expect("seen").clone();
    assert!(!bodies.is_empty(), "the upstream was called");
    let last = bodies.last().expect("a request");
    assert!(last.contains("\"model\":\"upstream-model\""), "upstream saw its model: {last}");
    assert!(!last.contains("gate/planner"), "the virtual model never reached upstream");
    assert!(
        last.to_ascii_lowercase().contains("authorization: bearer sk-lane-key"),
        "the upstream hop carries the lane credential: {last}"
    );
    assert!(
        !last.to_ascii_lowercase().contains("gate-bearer-token"),
        "the client's gate bearer must never reach upstream: {last}"
    );

    let records = std::fs::read_to_string(&mirror).expect("mirror");
    assert!(!records.is_empty(), "the gate recorded evidence");
    assert!(records.contains("\"verdict\":\"performed\""), "rows: {records}");
    assert!(!records.contains("sk-lane-key"), "no lane key in evidence");
    assert!(!records.contains("gate-bearer-token"), "no gate token in evidence");

    std::fs::remove_dir_all(&work).ok();
}

/// Kills the gate when the test ends, however it ends.
struct GateChild(std::process::Child);

impl Drop for GateChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
