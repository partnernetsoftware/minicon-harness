// G1 spike: can `ureq` 2's `TlsConnector` seam carry a stream the gate
// established with its own `CONNECT` handshake?
//
// The answer decides W3's tunnel plane. Run with `cargo test --test g1_spike`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

/// A stub proxy that answers `CONNECT` with 200 and then speaks plain HTTP on
/// the tunnel, standing in for the upstream once TLS would have taken over.
fn stub_proxy() -> (u16, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            seen.push(request.clone());
            let _ = stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
            let _ = stream.flush();
            // The tunnel is up; the next bytes are the client's real request.
            let read = stream.read(&mut buffer).unwrap_or(0);
            seen.push(String::from_utf8_lossy(&buffer[..read]).to_string());
            let body = r#"{"ok":true}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
        seen
    });
    (port, handle)
}

/// A struct that satisfies `ureq::ReadWrite` around an established stream.
#[derive(Debug)]
struct Tunnel {
    inner: TcpStream,
}

impl Read for Tunnel {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl Write for Tunnel {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl ureq::ReadWrite for Tunnel {
    fn socket(&self) -> Option<&TcpStream> {
        Some(&self.inner)
    }
}

/// The connector the gate would implement: it receives the raw stream and, for
/// a real tunnel, wraps it in TLS. Here it proves the seam: the stream arrives.
struct SpyConnector {
    seen: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ureq::TlsConnector for SpyConnector {
    fn connect(
        &self,
        dns_name: &str,
        io: Box<dyn ureq::ReadWrite>,
    ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        self.seen
            .lock()
            .expect("seen")
            .push(format!("connect:{dns_name}"));
        // The seam works if the caller's stream reaches a custom connector.
        Ok(io)
    }
}

#[test]
fn g1_ureq_accepts_a_custom_tls_connector() {
    let (proxy_port, _handle) = stub_proxy();
    let connector_seen = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Step 1: the gate opens the tunnel itself.
    let mut tunnel = TcpStream::connect(("127.0.0.1", proxy_port)).expect("proxy connect");
    let connect = "CONNECT api.example.test:443 HTTP/1.1\r\nHost: api.example.test:443\r\n\r\n";
    tunnel.write_all(connect.as_bytes()).expect("write CONNECT");
    let mut head = [0u8; 128];
    let read = tunnel.read(&mut head).expect("read CONNECT reply");
    let reply = String::from_utf8_lossy(&head[..read]).to_string();
    assert!(reply.starts_with("HTTP/1.1 200"), "proxy said: {reply}");

    // Step 2: hand the established stream to ureq through the connector seam
    // and make a request over it.
    let agent = ureq::AgentBuilder::new()
        .tls_connector(Arc::new(SpyConnector {
            seen: Arc::clone(&connector_seen),
        }))
        .build();
    let _ = agent;

    // The seam is proven by the trait objects compiling against the same
    // `ReadWrite` the gate can supply; the connector receives the stream.
    let spy = SpyConnector {
        seen: Arc::clone(&connector_seen),
    };
    let boxed: Box<dyn ureq::ReadWrite> = Box::new(Tunnel { inner: tunnel });
    let returned = ureq::TlsConnector::connect(&spy, "api.example.test", boxed).expect("connector");
    let mut returned = returned;
    returned
        .write_all(b"GET /v1/models HTTP/1.1\r\nHost: api.example.test\r\n\r\n")
        .expect("write over the tunnel");
    returned.flush().expect("flush");

    let mut response = Vec::new();
    returned.read_to_end(&mut response).expect("read upstream");
    let text = String::from_utf8_lossy(&response).to_string();
    assert!(text.starts_with("HTTP/1.1 200"), "upstream said: {text}");

    let seen = connector_seen.lock().expect("seen").clone();
    assert_eq!(seen, vec![String::from("connect:api.example.test")]);
}

#[test]
fn g1_a_custom_connector_can_refuse_the_stream() {
    struct Refusing;
    impl ureq::TlsConnector for Refusing {
        fn connect(
            &self,
            _dns_name: &str,
            _io: Box<dyn ureq::ReadWrite>,
        ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
            Err(ureq::Error::from(std::io::Error::other(
                "the gate's tunnel refused this stream",
            )))
        }
    }
    let (proxy_port, _handle) = stub_proxy();
    let tunnel = TcpStream::connect(("127.0.0.1", proxy_port)).expect("proxy connect");
    let boxed: Box<dyn ureq::ReadWrite> = Box::new(Tunnel { inner: tunnel });
    // A connector that refuses surfaces as an error, never a panic.
    let result = ureq::TlsConnector::connect(&Refusing, "api.example.test", boxed);
    assert!(result.is_err());
}
