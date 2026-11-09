//! The definition plane: config parsing, validation, revision, and the
//! Kronecker expansion from three axes to ordered candidates.
//!
//! Pure by construction -- nothing here opens a socket, reads a secret or
//! consults a clock. That is what lets an independent reader recompute every
//! routing decision from the revision alone (the reader contract in
//! `PRD_01_27` "Evidence").
//!
//! One deliberate deviation from `plan/v0.0.20.md` section 4.8, reported at W2:
//! the plan's fixture gives a credential an `endpoint` field, while
//! `PRD_01_27` says a credential carries a `scope` and is not bound to one
//! endpoint. The contract wins, so `endpoint` is accepted and checked against
//! the lane's endpoint (`credential_endpoint_mismatch`), and a `scope` object is
//! optional. See the W2 record in the plan.

use serde_json::Value;

use super::refusal::{code, Refusal};

/// Config file bound from `PRD_01_27` "Bounds".
pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;
/// Maximum endpoints.
pub const MAX_ENDPOINTS: usize = 256;
/// Maximum credentials.
pub const MAX_CREDENTIALS: usize = 256;
/// Maximum egress definitions.
pub const MAX_EGRESS: usize = 64;
/// Maximum lanes.
pub const MAX_LANES: usize = 512;
/// Maximum agents.
pub const MAX_AGENTS: usize = 256;
/// Maximum workflows.
pub const MAX_WORKFLOWS: usize = 64;
/// Maximum nodes in one workflow.
pub const MAX_WORKFLOW_NODES: usize = 64;
/// Maximum expanded candidates for one agent.
pub const MAX_CANDIDATES: usize = 32;

/// A secret reference. The value is read at egress time and never stored here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecretRef {
    /// `{ "env": "NAME" }`
    Env(String),
    /// `{ "file": "PATH" }`
    File(String),
    /// `{ "oauthFile": "PATH" }` -- parsed, refused in this slice.
    OauthFile(String),
    /// `{ "command": [argv...] }` -- parsed, refused in this slice.
    Command(Vec<String>),
}

impl SecretRef {
    fn parse(value: &Value, path: &str, out: &mut Vec<Refusal>) -> Option<Self> {
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                out.push(Refusal::new(path, code::BAD_CONFIG, "secret must be an object"));
                return None;
            }
        };
        let present: Vec<&str> = ["env", "file", "oauthFile", "command"]
            .into_iter()
            .filter(|key| object.contains_key(*key))
            .collect();
        match present.as_slice() {
            [only] => match *only {
                "env" => object["env"].as_str().map(|s| SecretRef::Env(s.to_owned())),
                "file" => object["file"].as_str().map(|s| SecretRef::File(s.to_owned())),
                "oauthFile" => object["oauthFile"].as_str().map(|s| SecretRef::OauthFile(s.to_owned())),
                "command" => object["command"].as_array().map(|list| {
                    SecretRef::Command(
                        list.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect(),
                    )
                }),
                _ => None,
            }
            .or_else(|| {
                out.push(Refusal::new(
                    path,
                    code::BAD_CONFIG,
                    format!("secret {} must be a string", only),
                ));
                None
            }),
            _ => {
                out.push(Refusal::new(
                    path,
                    code::BAD_CONFIG,
                    "secret must name exactly one of env/file/oauthFile/command",
                ));
                None
            }
        }
    }

    /// Whether 0.0.20 can resolve this reference. `oauthFile` and `command` are
    /// named in the schema and refused rather than silently unsupported.
    pub fn supported(&self) -> bool {
        matches!(self, SecretRef::Env(_) | SecretRef::File(_))
    }

    /// How the refusal names the shape without printing any secret.
    pub fn shape(&self) -> &'static str {
        match self {
            SecretRef::Env(_) => "env",
            SecretRef::File(_) => "file",
            SecretRef::OauthFile(_) => "oauthFile",
            SecretRef::Command(_) => "command",
        }
    }
}

/// One upstream: protocol plus where it lives. `provider` is a label, never a
/// metadata table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// String label (openai, deepseek, ...), used in evidence and scope checks.
    pub provider: String,
    /// `openai-chat` in 0.0.20.
    pub protocol: String,
    /// Upstream root; the path prefix is preserved.
    pub base_url: String,
    /// Static upstream headers, string values only.
    pub headers: Vec<(String, String)>,
}

/// One credential: identity plus a secret reference. Never the secret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credential {
    /// Identifier the lane's credential order names.
    pub id: String,
    /// The endpoint this credential belongs to, when the config says so.
    /// Checked at load time against every lane that names the credential.
    pub endpoint: Option<String>,
    /// Human-facing name.
    pub label: String,
    /// Where the secret comes from.
    pub secret: SecretRef,
    /// Optional `scope` object (for example `{ "modelArgs": { "store": false } }`),
    /// merged at the lowest layer of precedence above the gate defaults.
    pub scope: Option<Value>,
    /// `"client": true` -- the caller supplies the upstream credential.
    pub client: bool,
    /// Upstream hosts a `client` credential may reach (mandatory for `client`).
    pub allowed_hosts: Vec<String>,
}

/// One pipe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Proxy {
    /// Never proxied, even when the gate process has `HTTP(S)_PROXY`.
    Direct,
    /// Inherit the process variables.
    Env,
    /// An explicit proxy URL plus the mandatory bypass list.
    Url {
        /// `http://host:port`, credentials allowed.
        url: String,
        /// Hosts that bypass the proxy; never empty.
        no_proxy: Vec<String>,
    },
}

/// One egress axis entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Egress {
    /// Identifier the lane's egress order names.
    pub id: String,
    /// How a request leaves the gate.
    pub proxy: Proxy,
    /// `headerMs` and `chunkMs` transport timeouts, when the axis sets them.
    pub timeouts: Option<Timeouts>,
}

/// Transport timeouts for one attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timeouts {
    /// Max wait for response headers.
    pub header_ms: u64,
    /// Max gap between streamed chunks.
    pub chunk_ms: u64,
}

/// One lane: an endpoint pairing with ordered credentials and egress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lane {
    /// Lane id.
    pub id: String,
    /// The endpoint axis this lane composes with.
    pub endpoint: String,
    /// Upstream model id actually sent.
    pub model: String,
    /// Credentials in declared order (outer loop of the product).
    pub credential_order: Vec<String>,
    /// Egress in declared order (inner loop of the product).
    pub egress_order: Vec<String>,
    /// Request-body overrides; merged over the credential scope.
    pub model_args: Value,
    /// Per-lane limits.
    pub limits: Limits,
}

/// Per-lane limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Concurrent attempts on one lane.
    pub concurrency: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { concurrency: 1 }
    }
}

/// One agent: an ordered lane list plus overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Agent {
    /// Agent id.
    pub id: String,
    /// Ordered lane ids.
    pub lanes: Vec<String>,
    /// Selection strategy; `ordered` only in 0.0.20.
    pub select: String,
    /// Agent-level request-body overrides.
    pub model_args: Value,
    /// Transport bounds.
    pub budget: Budget,
}

/// Transport bounds for one request through an agent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    /// Attempt chain length.
    pub max_attempts: u32,
    /// Optional output-token cap.
    pub max_output_tokens: Option<u64>,
}

/// One workflow node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    /// The agent this node binds.
    pub agent: String,
    /// Node-level `egress` override; replaces the lane's order when present.
    pub egress: Option<String>,
    /// Node-level request-body overrides.
    pub model_args: Value,
}

/// One workflow edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    /// Source node.
    pub from: String,
    /// Target node.
    pub to: String,
}

/// One workflow: a declaration, never something the gate advances.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    /// Workflow id.
    pub id: String,
    /// Entry node.
    pub entry: String,
    /// Nodes in declared order.
    pub nodes: Vec<(String, Node)>,
    /// Edges in declared order.
    pub edges: Vec<Edge>,
}

/// The listen block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listen {
    /// `host:port`.
    pub bind: String,
    /// Optional local bearer for the client-to-gate hop.
    pub auth: Option<SecretRef>,
}

/// Evidence sink settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Events {
    /// Ring capacity.
    pub ring: usize,
    /// Optional JSONL mirror path; its parent directory must exist.
    pub mirror: Option<String>,
}

/// The whole definition plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateConfig {
    /// Listener.
    pub listen: Listen,
    /// Gate-wide defaults.
    pub defaults: Defaults,
    /// Evidence sink.
    pub events: Events,
    /// Endpoint axis.
    pub endpoints: Vec<(String, Endpoint)>,
    /// Credential axis.
    pub credentials: Vec<(String, Credential)>,
    /// Egress axis.
    pub egress: Vec<(String, Egress)>,
    /// Lanes.
    pub lanes: Vec<(String, Lane)>,
    /// Agents.
    pub agents: Vec<(String, Agent)>,
    /// Workflows.
    pub workflows: Vec<(String, Workflow)>,
}

/// Gate-wide defaults; the lowest precedence layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Defaults {
    /// Default transport timeouts.
    pub timeouts: Timeouts,
    /// Default request-body overrides.
    pub model_args: Value,
}

/// A config that loaded: its path, the parsed definition and the revision.
#[derive(Clone, Debug)]
pub struct LoadedConfig {
    /// The file this config came from.
    pub path: std::path::PathBuf,
    /// `sha256` over the canonicalised config bytes.
    pub revision: String,
    /// The parsed definition.
    pub config: GateConfig,
}

/// One resolved routing candidate: the product of the three axes plus the
/// lane's own settings. Deterministic given the config, so the reader
/// recomputes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Lane this candidate came from.
    pub lane: String,
    /// Endpoint axis id.
    pub endpoint: String,
    /// Credential axis id.
    pub credential: String,
    /// Egress axis id.
    pub egress: String,
    /// Upstream model id.
    pub model: String,
    /// Merged request-body overrides: defaults < credential scope < lane < agent.
    pub model_args: Value,
}

/// Reads, parses, validates and revision-hashes one config file.
pub fn load(path: &std::path::Path, allow_non_loopback: bool) -> Result<LoadedConfig, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "{}: {}: {}: {}",
            super::PROGRAM,
            path.display(),
            code::CONFIG_UNREADABLE,
            error
        )
    })?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(format!(
            "{}: {}: {}: the config is {} bytes, over the {MAX_CONFIG_BYTES} bound",
            super::PROGRAM,
            path.display(),
            code::OVER_BOUND,
            bytes.len()
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        format!(
            "{}: {}: {}: not UTF-8: {}",
            super::PROGRAM,
            path.display(),
            code::BAD_CONFIG,
            error
        )
    })?;
    let revision = revision_of(text);
    match parse(text, allow_non_loopback) {
        Ok(config) => Ok(LoadedConfig {
            path: path.to_path_buf(),
            revision,
            config,
        }),
        Err(refusals) => {
            let mut message = String::new();
            for refusal in refusals {
                message.push_str(&refusal.render(path));
                message.push('\n');
            }
            Err(message.trim_end().to_owned())
        }
    }
}

/// Formats a load or refusal message exactly once: a message that already
/// starts with the program name is printed as-is, anything else gets it.
pub fn program_message(message: &str) -> String {
    if message.starts_with(super::PROGRAM) {
        message.to_owned()
    } else {
        format!("{}: {message}", super::PROGRAM)
    }
}

/// `sha256` over the canonicalised config text (CRLF normalised to LF, so a
/// checkout round trip does not change the revision).
pub fn revision_of(text: &str) -> String {
    // A small, dependency-free SHA-256: the gate owns its revision, so it must
    // not depend on a crate to compute one.
    sha256_hex(text.replace("\r\n", "\n").as_bytes())
}

/// Pure-`std` SHA-256, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    let mut w = [0u32; 64];
    for chunk in message.chunks_exact(64) {
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

include!("config_parse.rs");
include!("config_expand.rs");
include!("config_tests.rs");
