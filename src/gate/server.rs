//! The ingress: a bounded, hand-written HTTP/1.1 server on `std::net::TcpListener`.
//!
//! Deliberately tiny, exactly like the MCP adapter's hand-written stdio loop is
//! deliberately tiny on stdio:
//!
//! - request line + headers + `Content-Length` body, nothing else;
//! - a chunked request is refused (`bad_body`), because buffering an unbounded
//!   body is precisely what the bounds exist to prevent;
//! - header count, header line length, URL length and body size each have a
//!   `PRD_01_27` bound and a named refusal when they exceed it;
//! - the `Host` header must name a loopback authority (DNS-rebinding check),
//!   and no CORS header is ever emitted.
//!
//! The server owns one accept loop and hands each connection to a fixed worker
//! pool. It knows nothing about routing; it parses a request and calls the
//! handler, which is where every HTTP refusal is decided.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::refusal::code;

/// Largest request line plus headers the parser will buffer, from
/// `PRD_01_27` "Bounds" (64 headers, 8 KiB per line).
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
/// Maximum header count.
pub const MAX_HEADERS: usize = 64;
/// Maximum single header line.
pub const MAX_HEADER_LINE: usize = 8 * 1024;
/// Maximum URL length.
pub const MAX_URL: usize = 2 * 1024;
/// Largest request body the gate will buffer.
pub const MAX_BODY: usize = 8 * 1024 * 1024;
/// Read timeout for one request, so a silent client cannot pin a worker.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// One parsed request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// Upper-cased method.
    pub method: String,
    /// Path only (no query string).
    pub path: String,
    /// The raw query string, when present.
    pub query: String,
    /// Headers as received, in order, names lower-cased.
    pub headers: Vec<(String, String)>,
    /// The body bytes.
    pub body: Vec<u8>,
}

impl Request {
    /// The first header with `name` (case-insensitive), or `None`.
    pub fn header(&self, name: &str) -> Option<&str> {
        let wanted = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == wanted)
            .map(|(_, value)| value.as_str())
    }
}

/// One response the handler produced.
///
/// A buffered response is `Body::Bytes`; an SSE pass-through is
/// `Body::Stream`, relayed chunk by chunk under an event cap. The two share
/// one renderer so a caller cannot tell from the status line which it got.
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Headers, emitted in order.
    pub headers: Vec<(String, String)>,
    /// The body, buffered or streaming.
    pub body: Body,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Response")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body", &match &self.body {
                Body::Bytes(bytes) => format!("<{} bytes>", bytes.len()),
                Body::Stream(_) => String::from("<stream>"),
            })
            .finish()
    }
}

/// A response body.
pub enum Body {
    /// Fully buffered.
    Bytes(Vec<u8>),
    /// A live upstream stream, relayed in `chunk`-sized reads, one event at a
    /// time, under `max_event` bytes per chunk.
    Stream(Box<dyn std::io::Read + Send>),
}

impl Response {
    /// A buffered response.
    pub fn buffered(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body: Body::Bytes(body),
        }
    }

    /// A JSON body with `application/json`.
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self::buffered(
            status,
            vec![(
                String::from("Content-Type"),
                String::from("application/json"),
            )],
            value.to_string().into_bytes(),
        )
    }

    /// A plain text body.
    pub fn text(status: u16, text: &str) -> Self {
        Self::buffered(
            status,
            vec![(
                String::from("Content-Type"),
                String::from("text/plain; charset=utf-8"),
            )],
            text.as_bytes().to_vec(),
        )
    }

    /// An SSE pass-through.
    pub fn stream(status: u16, headers: Vec<(String, String)>, body: Box<dyn std::io::Read + Send>) -> Self {
        Self {
            status,
            headers,
            body: Body::Stream(body),
        }
    }

    /// The head of the wire form: status line and headers.
    fn render_head(&self, content_length: Option<usize>) -> Vec<u8> {
        let reason = reason_phrase(self.status);
        let mut head = format!("HTTP/1.1 {} {}\r\n", self.status, reason);
        match content_length {
            Some(length) => head.push_str(&format!("Content-Length: {length}\r\n")),
            // A stream has no length up front. The connection closes at the end
            // of the body, which is the HTTP/1.0 completion rule and needs no
            // chunk framing: the bytes the upstream sent are the bytes the
            // client reads.
            None => head.push_str("Connection: close\r\n"),
        }
        if content_length.is_some() {
            head.push_str("Connection: close\r\n");
        }
        for (name, value) in &self.headers {
            if content_length.is_none() && name.eq_ignore_ascii_case("connection") {
                continue;
            }
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        head.into_bytes()
    }

    /// The rendered wire bytes of a buffered response.
    pub fn render(&self) -> Vec<u8> {
        match &self.body {
            Body::Bytes(bytes) => {
                let mut wire = self.render_head(Some(bytes.len()));
                wire.extend_from_slice(bytes);
                wire
            }
            // A stream is not renderable into one buffer by design; the
            // connection writer relays it.
            Body::Stream(_) => self.render_head(None),
        }
    }
}

/// The canonical reason phrase for a status the gate emits.
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// A parse failure: the status to answer and the named refusal code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseFailure {
    /// Status the gate answers with.
    pub status: u16,
    /// A `code::*` constant.
    pub code: &'static str,
    /// One sentence of reason, never containing a secret.
    pub reason: String,
}

impl ParseFailure {
    fn new(status: u16, code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            status,
            code,
            reason: reason.into(),
        }
    }
}

/// Whether an authority names a loopback host.
pub fn is_loopback_authority(authority: &str) -> bool {
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        match authority.rsplit_once(':') {
            Some((host, _port)) => host,
            None => authority,
        }
    };
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

/// Parses one request from `stream`; `limit` bounds the whole exchange.
pub fn parse_request(stream: &mut impl Read) -> Result<Request, ParseFailure> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    // Read until the end of the header block, bounded.
    let header_end = loop {
        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(ParseFailure::new(
                413,
                code::BODY_TOO_LARGE,
                format!("the request head is over the {MAX_HEADER_BYTES} byte bound"),
            ));
        }
        let read = stream
            .read(&mut chunk)
            .map_err(|error| ParseFailure::new(400, code::BAD_BODY, format!("the request could not be read: {error}")))?;
        if read == 0 {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                "the connection closed before the request head was complete",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| ParseFailure::new(400, code::BAD_BODY, "the request head is not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| ParseFailure::new(400, code::BAD_BODY, "the request line is missing"))?;
    let parts: Vec<&str> = request_line.split(' ').collect();
    if parts.len() != 3 {
        return Err(ParseFailure::new(
            400,
            code::BAD_BODY,
            "the request line must be `METHOD TARGET HTTP/1.1`",
        ));
    }
    let method = parts[0].to_ascii_uppercase();
    let target = parts[1];
    if !parts[2].starts_with("HTTP/1.") {
        return Err(ParseFailure::new(400, code::BAD_BODY, "only HTTP/1.x is served"));
    }
    if target.len() > MAX_URL {
        return Err(ParseFailure::new(
            400,
            code::BAD_BODY,
            format!("the URL is over the {MAX_URL} byte bound"),
        ));
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_owned(), query.to_owned()),
        None => (target.to_owned(), String::new()),
    };
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                format!("the request has over {MAX_HEADERS} headers"),
            ));
        }
        if line.len() > MAX_HEADER_LINE {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                format!("a header line is over the {MAX_HEADER_LINE} byte bound"),
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ParseFailure::new(400, code::BAD_BODY, "a header has no colon"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    // DNS-rebinding check: a browser can be pointed at a loopback listener by a
    // hostile page, so the Host must itself be loopback.
    match headers.iter().find(|(name, _)| name == "host") {
        Some((_, authority)) if is_loopback_authority(authority) => {}
        Some((_, authority)) => {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                format!("the Host header `{authority}` is not a loopback authority"),
            ));
        }
        None => {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                "the Host header is required",
            ));
        }
    }
    if headers
        .iter()
        .any(|(name, value)| name == "transfer-encoding" && value.to_ascii_lowercase().contains("chunked"))
    {
        return Err(ParseFailure::new(
            400,
            code::BAD_BODY,
            "chunked request bodies are refused; send Content-Length",
        ));
    }
    let declared = match headers.iter().find(|(name, _)| name == "content-length") {
        None => 0,
        Some((_, value)) => value
            .parse::<usize>()
            .map_err(|_| ParseFailure::new(400, code::BAD_BODY, "Content-Length is not a number"))?,
    };
    if declared > MAX_BODY {
        return Err(ParseFailure::new(
            413,
            code::BODY_TOO_LARGE,
            format!("the body declares {declared} bytes, over the {MAX_BODY} bound"),
        ));
    }
    let body_start = header_end + 4;
    while buffer.len() < body_start + declared {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| ParseFailure::new(400, code::BAD_BODY, format!("the body could not be read: {error}")))?;
        if read == 0 {
            return Err(ParseFailure::new(
                400,
                code::BAD_BODY,
                "the connection closed before the declared body arrived",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer[body_start..body_start + declared].to_vec();
    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// The handler one request is given.
pub type Handler = dyn Fn(&Request) -> Response + Send + Sync;

/// A running server: the bound address and a shutdown flag.
pub struct Server {
    /// The address the listener actually bound (resolves port 0).
    pub local: std::net::SocketAddr,
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Binds a loopback address and starts accepting in the background.
    pub fn start(
        bind: &str,
        workers: usize,
        handler: Arc<Handler>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(bind).map_err(|error| {
            format!("{}: {bind}: bind failed: {error}", super::PROGRAM)
        })?;
        listener
            .set_nonblocking(false)
            .map_err(|error| format!("{}: {bind}: {error}", super::PROGRAM))?;
        let local = listener
            .local_addr()
            .map_err(|error| format!("{}: {bind}: {error}", super::PROGRAM))?;
        if !local.ip().is_loopback() {
            return Err(format!(
                "{}: {local}: {}: the listener must be loopback",
                super::PROGRAM,
                code::NON_LOOPBACK_BIND
            ));
        }
        let running = Arc::new(AtomicBool::new(true));
        let workers = workers.clamp(1, 16);
        let shared = Arc::new(Shared {
            listener,
            running: Arc::clone(&running),
            handler,
            workers,
        });
        let handle = std::thread::spawn(move || shared.accept_loop());
        Ok(Self {
            local,
            running,
            handle: Some(handle),
        })
    }
    /// Stops accepting and waits for the accept thread.
    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Unblock the accept() that is waiting for a connection.
        let _ = TcpStream::connect(self.local);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect(self.local);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct Shared {
    listener: TcpListener,
    running: Arc<AtomicBool>,
    handler: Arc<Handler>,
    workers: usize,
}

impl Shared {
    fn accept_loop(self: Arc<Self>) {
        // A fixed worker pool: one accept thread hands each connection over,
        // and no request can spawn unbounded threads.
        let (sender, receiver) = std::sync::mpsc::sync_channel::<TcpStream>(self.workers * 2);
        let receiver = Arc::new(std::sync::Mutex::new(receiver));
        let mut workers = Vec::with_capacity(self.workers);
        for _ in 0..self.workers {
            let receiver = Arc::clone(&receiver);
            let handler = Arc::clone(&self.handler);
            workers.push(std::thread::spawn(move || loop {
                let stream = {
                    let guard = receiver.lock().expect("worker receiver");
                    guard.recv()
                };
                let Ok(stream) = stream else { return };
                serve_connection(stream, &*handler);
            }));
        }
        for stream in self.listener.incoming() {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }
            match stream {
                Ok(stream) => {
                    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
                    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
                    if sender.send(stream).is_err() {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
        drop(sender);
        for worker in workers {
            let _ = worker.join();
        }
    }
}

fn serve_connection(mut stream: TcpStream, handler: &Handler) {
    let response = match parse_request(&mut stream) {
        Ok(request) => handler(&request),
        Err(failure) => refusal_response(failure.status, failure.code, &failure.reason),
    };
    // The head is rendered before the body moves, so the same renderer serves a
    // buffered answer and a live SSE relay.
    let head = response.render_head(match &response.body {
        Body::Bytes(bytes) => Some(bytes.len()),
        Body::Stream(_) => None,
    });
    match response.body {
        Body::Bytes(bytes) => {
            let mut wire = head;
            wire.extend_from_slice(&bytes);
            let _ = stream.write_all(&wire);
            let _ = stream.flush();
        }
        Body::Stream(mut body) => {
            // SSE pass-through: the head goes out first, then each read is
            // relayed as it arrives. The gate never buffers the whole stream,
            // never re-frames an event and never invents a terminator.
            if stream.write_all(&head).is_err() {
                return;
            }
            let _ = stream.flush();
            let mut chunk = vec![0u8; SSE_RELAY_CHUNK];
            loop {
                match body.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => {
                        if stream.write_all(&chunk[..read]).is_err() {
                            break;
                        }
                        let _ = stream.flush();
                    }
                    // A transport failure after the first byte closes the
                    // stream rather than fabricating a `[DONE]`.
                    Err(_) => break,
                }
            }
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// The relay read size. An SSE event is bounded by the contract; this is the
/// socket read, so a large `data:` line still streams without buffering it
/// whole.
pub const SSE_RELAY_CHUNK: usize = 16 * 1024;

/// The `PRD_01_27` local refusal body.
pub fn refusal_response(status: u16, code: &str, reason: &str) -> Response {
    Response::json(
        status,
        &serde_json::json!({
            "error": {
                "message": format!("gate: {reason} ({code})"),
                "type": "gate_refusal",
                "code": code,
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<Request, ParseFailure> {
        parse_request(&mut std::io::Cursor::new(raw.as_bytes().to_vec()))
    }

    #[test]
    fn a_well_formed_request_parses() {
        let request = parse(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:8787\r\nContent-Length: 2\r\n\r\n{}",
        )
        .expect("parses");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/chat/completions");
        assert_eq!(request.body, b"{}");
        assert_eq!(request.header("host"), Some("127.0.0.1:8787"));
    }

    #[test]
    fn body_is_read_exactly_as_declared() {
        let request = parse(
            "PUT /x HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhelloTrailing",
        )
        .expect("parses");
        assert_eq!(request.body, b"hello");
    }

    #[test]
    fn query_is_split_from_path() {
        let request = parse("GET /a/b?c=d HTTP/1.1\r\nHost: localhost\r\n\r\n").expect("parses");
        assert_eq!(request.path, "/a/b");
        assert_eq!(request.query, "c=d");
    }

    #[test]
    fn a_non_loopback_host_is_refused() {
        let failure = parse("GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n").expect_err("must refuse");
        assert_eq!(failure.code, code::BAD_BODY);
        assert!(failure.reason.contains("loopback"));
    }

    #[test]
    fn bracket_ipv6_loopback_host_is_accepted() {
        let request = parse("GET / HTTP/1.1\r\nHost: [::1]:8787\r\n\r\n").expect("parses");
        assert_eq!(request.path, "/");
    }

    #[test]
    fn chunked_is_refused() {
        let failure =
            parse("POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
                .expect_err("must refuse");
        assert!(failure.reason.contains("chunked"));
    }

    #[test]
    fn an_oversized_declared_body_is_413() {
        let failure = parse(&format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        ))
        .expect_err("must refuse");
        assert_eq!(failure.status, 413);
        assert_eq!(failure.code, code::BODY_TOO_LARGE);
    }

    #[test]
    fn too_many_headers_are_refused() {
        let mut raw = String::from("GET / HTTP/1.1\r\nHost: localhost\r\n");
        for index in 0..=MAX_HEADERS {
            raw.push_str(&format!("X-H{index}: v\r\n"));
        }
        raw.push_str("\r\n");
        let failure = parse(&raw).expect_err("must refuse");
        assert!(failure.reason.contains("headers"));
    }

    #[test]
    fn a_long_url_is_refused() {
        let target = format!("/{}", "a".repeat(MAX_URL));
        let failure = parse(&format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n"))
            .expect_err("must refuse");
        assert!(failure.reason.contains("URL"));
    }

    #[test]
    fn a_missing_request_line_part_is_refused() {
        let failure = parse("GET /\r\n\r\n").expect_err("must refuse");
        assert_eq!(failure.code, code::BAD_BODY);
    }

    #[test]
    fn refusal_body_has_the_contract_shape() {
        let response = refusal_response(400, code::BAD_BODY, "nope");
        let crate::gate::server::Body::Bytes(bytes) = &response.body else {
            panic!("a refusal is buffered");
        };
        let value: serde_json::Value = serde_json::from_slice(bytes).expect("json");
        assert_eq!(value["error"]["type"], "gate_refusal");
        assert_eq!(value["error"]["code"], "bad_body");
        assert!(value["error"]["message"].as_str().unwrap_or_default().contains("bad_body"));
    }

    #[test]
    fn response_renders_content_length_and_connection_close() {
        let response = Response::text(200, "ok");
        let wire = String::from_utf8(response.render()).expect("utf8");
        assert!(wire.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(wire.contains("Content-Length: 2\r\n"));
        assert!(wire.contains("Connection: close\r\n"));
        assert!(wire.ends_with("\r\n\r\nok"));
    }

    #[test]
    fn loopback_authority_recognises_the_three_names() {
        assert!(is_loopback_authority("localhost"));
        assert!(is_loopback_authority("localhost:8787"));
        assert!(is_loopback_authority("127.0.0.1:1"));
        assert!(is_loopback_authority("[::1]:8787"));
        assert!(!is_loopback_authority("example.com"));
        assert!(!is_loopback_authority("10.0.0.1:80"));
    }
}
