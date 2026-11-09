//! The transport: one model call, out through one candidate's egress.
//!
//! 0.0.20 ships `direct` egress and non-stream `openai-chat`. The tunnel plane
//! (explicit `CONNECT`, `env`, proxy credentials) is W3; this module's shape is
//! already the one that plane needs -- an `Outcome` that carries a status, the
//! headers, the body and nothing else -- so W3 changes `connect` and nothing
//! above it.

use std::time::Duration;

use serde_json::Value;

use super::config::{Credential, Egress, Endpoint, Lane, Proxy};
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
}

impl Outcome {
    /// The disposition this outcome maps to.
    pub fn disposition(&self) -> Disposition {
        match self {
            Outcome::Answered { status, .. } => map::classify_status(*status),
            Outcome::Failed { .. } => map::CONNECT_FAILURE,
        }
    }

    /// The upstream status, when there was one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Outcome::Answered { status, .. } => Some(*status),
            Outcome::Failed { .. } => None,
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
    match &target.egress.proxy {
        Proxy::Direct => {}
        Proxy::Env | Proxy::Url { .. } => {
            return Outcome::Failed {
                reason: format!(
                    "egress `{}` needs the tunnel plane, which is W3 ({})",
                    target.egress.id,
                    super::refusal::code::NOT_IN_THIS_SLICE
                ),
            };
        }
    }
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
    let mut headers: Vec<(&str, &str)> = vec![
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
    ];
    for (name, value) in &target.endpoint.headers {
        headers.push((name.as_str(), value.as_str()));
    }
    let bearer = target.secret.bearer();
    let response = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(timeout)
        // A redirected POST replays the bearer to a host the operator never
        // named: the same rule the core's transport follows.
        .redirects(0)
        .build()
        .post(&url)
        .set("Authorization", &bearer)
        .send_string(&body);
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
            // A non-2xx is still an answer: the body states why, and it passes
            // through verbatim.
            let mut bytes = Vec::new();
            let _ = response.into_reader().take(MAX_RESPONSE_BYTES as u64).read_to_end(&mut bytes);
            Outcome::Answered {
                status,
                headers: Vec::new(),
                body: bytes,
            }
        }
        Err(ureq::Error::Transport(error)) => Outcome::Failed {
            reason: format!("the upstream request failed: {error}"),
        },
    }
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;
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
    fn a_proxy_egress_is_not_in_this_slice_yet() {
        let target = Target {
            endpoint: &endpoint(),
            egress: &egress(Proxy::Url {
                url: String::from("http://127.0.0.1:8891"),
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
        match post_chat(&target) {
            Outcome::Failed { reason } => {
                assert!(reason.contains("not_in_this_slice"));
                assert!(!reason.contains("k"), "the secret must not appear");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
