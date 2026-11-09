//! The transport: one model call, out through one candidate's egress.
//!
//! 0.0.20 ships `direct` egress and non-stream `openai-chat`. The tunnel plane
//! (explicit `CONNECT`, `env`, proxy credentials) is W3; this module's shape is
//! already the one that plane needs -- an `Outcome` that carries a status, the
//! headers, the body and nothing else -- so W3 changes `connect` and nothing
//! above it.

use std::time::Duration;

use serde_json::Value;

use super::config::{Credential, Egress, Endpoint, Lane};
use super::map::{self, Disposition};
use super::config::CommandLine;
use super::secret::Secret;

/// Largest non-stream response body the gate will accept, from `PRD_01_27`.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// Connect timeout for one attempt.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// One attempt's target: the four axes plus the request.
pub struct Target<'a> {
    /// The endpoint.
    pub endpoint: &'a Endpoint,
    /// The pipe.
    pub egress: &'a Egress,
    /// Who pays.
    pub credential: &'a Credential,
    /// The lane (for timeouts and limits).
    pub lane: &'a Lane,
    /// The resolved secret.
    pub secret: Secret,
    /// The upstream model id.
    pub model: &'a str,
    /// Merged request-body overrides.
    pub model_args: &'a Value,
    /// The client's original body.
    pub body: &'a Value,
    /// The injected process facts.
    pub command_line: &'a CommandLine,
}

/// What one attempt produced.
#[derive(Debug)]
pub enum Outcome {
    /// The upstream answered; the status and body pass through verbatim.
    Answered {
        /// Upstream status.
        status: u16,
        /// Response headers as received, lower-cased names.
        headers: Vec<(String, String)>,
        /// Body bytes.
        body: Vec<u8>,
    },
    /// Nothing came back: connect, TLS, timeout, or an unreadable body.
    Failed {
        /// One sentence, never containing a secret.
        reason: String,
    },
    /// The proxy, not the upstream, answered — a broken pipe, never an
    /// upstream refusal.
    ProxyRejected {
        /// The proxy's CONNECT status.
        status: u16,
    },
}

impl Outcome {
    /// The disposition this outcome maps to.
    pub fn disposition(&self) -> Disposition {
        match self {
            Outcome::Answered { status, .. } => map::classify_status(*status),
            Outcome::Failed { .. } => map::CONNECT_FAILURE,
            Outcome::ProxyRejected { status } if *status == 407 => map::PROXY_407,
            Outcome::ProxyRejected { .. } => map::CONNECT_FAILURE,
        }
    }

    /// The upstream status, when there was one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Outcome::Answered { status, .. } => Some(*status),
            Outcome::Failed { .. } => None,
            Outcome::ProxyRejected { status } => Some(*status),
        }
    }
}

/// The request body actually sent upstream: the client's body with the lane's
/// and the agent's `modelArgs` merged over it, and the protected fields the
/// gate owns removed.
pub fn upstream_body(target: &Target<'_>) -> Result<String, String> {
    let mut body = target.body.clone();
    let Some(object) = body.as_object_mut() else {
        return Err(String::from("the request body is not an object"));
    };
    // The gate owns `model`: the client's virtual model never reaches upstream.
    object.insert(String::from("model"), Value::String(target.model.to_owned()));
    if let Some(args) = target.model_args.as_object() {
        for (key, value) in args {
            // `model` in modelArgs would be an operator-owned field sneaking in
            // through the override layer; the lane's `model` is authoritative.
            if key == "model" {
                continue;
            }
            object.insert(key.clone(), value.clone());
        }
    }
    // A caller's own gate-only field never crosses the boundary.
    object.remove("gate_overrides");
    Ok(body.to_string())
}

/// The URL one attempt posts to.
pub fn url_for(target: &Target<'_>) -> String {
    let base = target.endpoint.base_url.trim_end_matches('/');
    format!("{base}/chat/completions")
}

/// Performs one attempt over `direct` egress.
///
/// A proxy, `env` or explicit, is refused here in W2 with
/// `not_in_this_slice`: the tunnel plane lands in W3, and a half-proxy that
/// silently used `direct` would be the exact failure the gate exists to fix.
pub fn post_chat(target: &Target<'_>) -> Outcome {
    let body = match upstream_body(target) {
        Ok(body) => body,
        Err(reason) => return Outcome::Failed { reason },
    };
    let timeout = target
        .egress
        .timeouts
        .map(|timeouts| Duration::from_millis(timeouts.header_ms))
        .unwrap_or(Duration::from_secs(300));
    let url = url_for(target);
    let endpoint_host = target
        .endpoint
        .base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&target.endpoint.base_url)
        .split('/')
        .next()
        .unwrap_or("")
        .to_owned();
    // The egress decision is data: a proxy is chosen per lane, never by an
    // HTTP client's global setting.
    let env_proxy = target
        .command_line
        .env
        .get("HTTPS_PROXY")
        .or_else(|| target.command_line.env.get("https_proxy"))
        .or_else(|| target.command_line.env.get("HTTP_PROXY"))
        .or_else(|| target.command_line.env.get("http_proxy"));
    let env_no_proxy: Vec<String> = target
        .command_line
        .env
        .get("NO_PROXY")
        .or_else(|| target.command_line.env.get("no_proxy"))
        .map(|value| value.split(',').map(|entry| entry.trim().to_owned()).collect())
        .unwrap_or_default();
    let route = match super::tunnel::route_for(&target.egress.proxy, &endpoint_host, env_proxy.map(String::as_str), &env_no_proxy) {
        Ok(route) => route,
        Err(reason) => return Outcome::Failed { reason },
    };
    // The tunnel is the unit of the per-lane decision, so the agent is built
    // per attempt: one agent carries exactly one egress, and no request can
    // inherit a proxy the operator did not pool for its lane.
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(timeout)
        // A redirected POST replays the bearer to a host the operator never
        // named: the same rule the core's transport follows.
        .redirects(0);
    if let super::tunnel::Route::Proxied { authority, credentials } = &route {
        // `ureq` opens the CONNECT itself for both http and https once it knows
        // the proxy; userinfo is parsed into `Proxy-Authorization` and never
        // printed.
        let url = match credentials {
            Some((user, password)) => {
                format!("http://{}:{}@{authority}", encode_userinfo(user), encode_userinfo(password))
            }
            None => format!("http://{authority}"),
        };
        match ureq::Proxy::new(url) {
            Ok(proxy) => builder = builder.proxy(proxy),
            Err(error) => {
                return Outcome::Failed {
                    reason: format!("the egress proxy URL is not usable: {error}"),
                };
            }
        }
    }
    let mut headers: Vec<(&str, &str)> = vec![
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
    ];
    for (name, value) in &target.endpoint.headers {
        headers.push((name.as_str(), value.as_str()));
    }
    let bearer = target.secret.bearer();
    let mut request = builder.build().post(&url).set("Authorization", &bearer);
    for (name, value) in headers {
        request = request.set(name, value);
    }
    let response = request.send_string(&body);
    match response {
        Ok(response) => {
            let status = response.status();
            let mut received = Vec::new();
            for name in response.headers_names() {
                if let Some(value) = response.header(&name) {
                    received.push((name.to_ascii_lowercase(), value.to_owned()));
                }
            }
            let mut bytes = Vec::new();
            match response.into_reader().take((MAX_RESPONSE_BYTES + 1) as u64).read_to_end(&mut bytes) {
                Ok(_) => {}
                Err(error) => {
                    return Outcome::Failed {
                        reason: format!("the upstream body could not be read: {error}"),
                    }
                }
            }
            if bytes.len() > MAX_RESPONSE_BYTES {
                return Outcome::Failed {
                    reason: format!(
                        "the upstream body is cut off at {MAX_RESPONSE_BYTES} bytes ({})",
                        super::refusal::code::BODY_TOO_LARGE
                    ),
                };
            }
            Outcome::Answered {
                status,
                headers: received,
                body: bytes,
            }
        }
        Err(ureq::Error::Status(status, response)) => {
            match &route {
                // The proxy, not the upstream, answered: `ureq` returns the
                // CONNECT reply's status as a status error. That is a broken
                // pipe, never an upstream refusal, so it must not be passed to
                // the client as if the model endpoint had said it.
                super::tunnel::Route::Proxied { .. } => {
                    let mut bytes = Vec::new();
                    let _ = response.into_reader().take(4096).read_to_end(&mut bytes);
                    Outcome::ProxyRejected { status }
                }
                // A non-2xx from the upstream is still an answer: its body
                // states why, and it passes through verbatim.
                super::tunnel::Route::Direct => {
                    let mut bytes = Vec::new();
                    let _ = response.into_reader().take(MAX_RESPONSE_BYTES as u64).read_to_end(&mut bytes);
                    Outcome::Answered {
                        status,
                        headers: Vec::new(),
                        body: bytes,
                    }
                }
            }
        }
        Err(ureq::Error::Transport(error)) => Outcome::Failed {
            reason: format!("the upstream request failed: {error}"),
        },
    }
}

/// Percent-encodes the bytes RFC 3986 forbids in userinfo, so a proxy password
/// with `@` or `:` survives the round trip through a URL.
fn encode_userinfo(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::config::Proxy;
    use serde_json::json;

    fn endpoint() -> Endpoint {
        Endpoint {
            provider: String::from("openai"),
            protocol: String::from("openai-chat"),
            base_url: String::from("https://api.openai.com/v1/"),
            headers: Vec::new(),
        }
    }

    fn egress(proxy: Proxy) -> Egress {
        Egress {
            id: String::from("e"),
            proxy,
            timeouts: None,
        }
    }

    fn credential() -> Credential {
        Credential {
            id: String::from("c"),
            endpoint: None,
            label: String::from("c"),
            secret: super::super::config::SecretRef::Env(String::from("K")),
            scope: None,
            client: false,
            allowed_hosts: Vec::new(),
        }
    }

    fn lane() -> Lane {
        Lane {
            id: String::from("l"),
            endpoint: String::from("oa"),
            model: String::from("m"),
            credential_order: vec![String::from("c")],
            egress_order: vec![String::from("e")],
            model_args: json!({}),
            limits: super::super::config::Limits::default(),
        }
    }

    fn body() -> Value {
        json!({ "model": "gate/planner", "messages": [{ "role": "user", "content": "hi" }] })
    }

    #[test]
    fn the_url_keeps_the_base_path_prefix() {
        let target = Target {
            endpoint: &endpoint(),
            egress: &egress(Proxy::Direct),
            credential: &credential(),
            lane: &lane(),
            secret: Secret::debug_value("k"),
            model: "m",
            model_args: &json!({}),
            body: &body(),
            command_line: &CommandLine::default(),
        };
        assert_eq!(url_for(&target), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn upstream_body_replaces_the_virtual_model_and_merges_args() {
        let target = Target {
            endpoint: &endpoint(),
            egress: &egress(Proxy::Direct),
            credential: &credential(),
            lane: &lane(),
            secret: Secret::debug_value("k"),
            model: "gpt-6.1-sol",
            model_args: &json!({ "temperature": 0.2, "model": "ignored" }),
            body: &body(),
            command_line: &CommandLine::default(),
        };
        let body: Value = serde_json::from_str(&upstream_body(&target).expect("body")).expect("json");
        assert_eq!(body["model"], json!("gpt-6.1-sol"));
        assert_eq!(body["temperature"], json!(0.2));
        assert_eq!(body["messages"][0]["content"], json!("hi"));
    }

    #[test]
    fn a_gate_only_field_never_crosses_the_boundary() {
        let mut request = body();
        request["gate_overrides"] = json!({ "x": 1 });
        let target = Target {
            endpoint: &endpoint(),
            egress: &egress(Proxy::Direct),
            credential: &credential(),
            lane: &lane(),
            secret: Secret::debug_value("k"),
            model: "m",
            model_args: &json!({}),
            body: &request,
            command_line: &CommandLine::default(),
        };
        let body: Value = serde_json::from_str(&upstream_body(&target).expect("body")).expect("json");
        assert!(body.get("gate_overrides").is_none());
    }

    #[test]
    fn a_proxied_egress_that_cannot_reach_its_proxy_fails_with_a_named_reason() {
        // Port 1 on loopback: the tunnel plane runs and reports an honest
        // transport failure rather than silently using `direct`.
        let target = Target {
            endpoint: &endpoint(),
            egress: &egress(Proxy::Url {
                url: String::from("http://127.0.0.1:1"),
                no_proxy: vec![String::from("localhost")],
            }),
            credential: &credential(),
            lane: &lane(),
            secret: Secret::debug_value("secret-value"),
            model: "m",
            model_args: &json!({}),
            body: &body(),
            command_line: &CommandLine::default(),
        };
        match post_chat(&target) {
            Outcome::Failed { reason } => {
                assert!(!reason.contains("secret-value"), "the secret must not appear: {reason}");
                assert!(!reason.contains("127.0.0.1:1@"), "no userinfo in a reason");
            }
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_loopback_endpoint_on_the_bypass_list_never_touches_the_proxy() {
        // A loopback endpoint on `noProxy` must route direct, so a dead proxy
        // cannot break the gate's own client hop.
        let loopback_endpoint = Endpoint {
            provider: String::from("stub"),
            protocol: String::from("openai-chat"),
            base_url: String::from("http://127.0.0.1:9/v1"),
            headers: Vec::new(),
        };
        let target = Target {
            endpoint: &loopback_endpoint,
            egress: &egress(Proxy::Url {
                url: String::from("http://127.0.0.1:1"),
                no_proxy: vec![String::from("127.0.0.1")],
            }),
            credential: &credential(),
            lane: &lane(),
            secret: Secret::debug_value("k"),
            model: "m",
            model_args: &json!({}),
            body: &body(),
            command_line: &CommandLine::default(),
        };
        let host = target
            .endpoint
            .base_url
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        let route = super::super::tunnel::route_for(&target.egress.proxy, host, None, &[]).expect("route");
        assert_eq!(route, super::super::tunnel::Route::Direct);
    }
}
