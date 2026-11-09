//! Named local refusals -- the plan's section 6 catalog as constants.
//!
//! A refusal is information, not an exception: every one carries a stable code
//! an independent reader can match, exactly like `cu-abi`'s vocabulary. Load
//! refusals print the file, the JSON path and the reason before the gate exits
//! non-zero; HTTP refusals carry the code in the error body.

/// The refusal code a reader matches on. String literals so a JSON response and
/// a stderr line spell the same word.
pub mod code {
    /// `model` is neither `gate/<agent>` nor `gate/lane/<id>`.
    pub const UNKNOWN_MODEL: &str = "unknown_model";
    /// An agent id does not exist.
    pub const UNKNOWN_AGENT: &str = "unknown_agent";
    /// A lane id does not exist.
    pub const UNKNOWN_LANE: &str = "unknown_lane";
    /// A lane references an endpoint that is not defined.
    pub const UNKNOWN_ENDPOINT: &str = "unknown_endpoint";
    /// A lane credential order references a credential that is not defined.
    pub const UNKNOWN_CREDENTIAL: &str = "unknown_credential";
    /// A lane egress order references an egress that is not defined.
    pub const UNKNOWN_EGRESS: &str = "unknown_egress";
    /// A workflow id does not exist.
    pub const UNKNOWN_WORKFLOW: &str = "unknown_workflow";
    /// A request or node override sets an operator-only field.
    pub const PROTECTED_FIELD: &str = "protected_field";
    /// The body is not one JSON object, or the request is chunked.
    pub const BAD_BODY: &str = "bad_body";
    /// The request body is over its bound.
    pub const BODY_TOO_LARGE: &str = "body_too_large";
    /// A lane queue is at its bound.
    pub const QUEUE_FULL: &str = "queue_full";
    /// `budget.maxAttempts` is exhausted.
    pub const ATTEMPT_BUDGET: &str = "attempt_budget";
    /// Every expanded candidate is cooling or queue-full.
    pub const NO_CANDIDATE_AVAILABLE: &str = "no_candidate_available";
    /// An env secret is unset or a file secret is unreadable (named, never shown).
    pub const SECRET_UNREADABLE: &str = "secret_unreadable";
    /// Transport failure on the last candidate.
    pub const UPSTREAM_UNREACHABLE: &str = "upstream_unreachable";
    /// The config revision changed mid-request.
    pub const STALE_CONFIG: &str = "stale_config";
    /// A shape `openai-chat` cannot express.
    pub const PROTOCOL_UNSUPPORTED: &str = "protocol_unsupported";
    /// `oauthFile` / `command` secrets, `harness` lanes, weighted select.
    pub const NOT_IN_THIS_SLICE: &str = "not_in_this_slice";
    /// Expansion exceeds the per-agent candidate bound.
    pub const EXPANSION_OVER_BOUND: &str = "expansion_over_bound";
    /// An egress host equals the gate's own listener.
    pub const EGRESS_SELF_LOOP: &str = "egress_self_loop";
    /// A credential's `endpoint` is not the endpoint its lane names.
    pub const CREDENTIAL_ENDPOINT_MISMATCH: &str = "credential_endpoint_mismatch";
    /// A `"client"` credential has no `allowedHosts`.
    pub const CLIENT_HOSTS_MISSING: &str = "client_hosts_missing";
    /// A numeric bound from `PRD_01_27` is exceeded.
    pub const OVER_BOUND: &str = "over_bound";
    /// Duplicate id under one axis or map.
    pub const DUPLICATE_ID: &str = "duplicate_id";
    /// The config is not one JSON object, or has an unknown field.
    pub const BAD_CONFIG: &str = "bad_config";
    /// The config file cannot be read.
    pub const CONFIG_UNREADABLE: &str = "config_unreadable";
    /// The bearer token the config names cannot be read.
    pub const GATE_TOKEN_UNREADABLE: &str = "gate_token_unreadable";
    /// `events.mirror` names a path whose parent directory does not exist.
    pub const MIRROR_PARENT_MISSING: &str = "mirror_parent_missing";
    /// A non-loopback `listen.bind` without `--allow-non-loopback`.
    pub const NON_LOOPBACK_BIND: &str = "non_loopback_bind";
}

/// One load-time refusal: which config location, which code, which reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    /// The JSON path inside the config (for example `$["lanes"]["oa-sol"]["egressOrder"][1]`).
    pub path: String,
    /// A `code::*` constant.
    pub code: &'static str,
    /// One sentence of reason, never containing a secret.
    pub reason: String,
}

impl Refusal {
    /// Builds one refusal.
    pub fn new(path: impl Into<String>, code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code,
            reason: reason.into(),
        }
    }

    /// The line printed to stderr.
    pub fn render(&self, file: &std::path::Path) -> String {
        format!(
            "{}: {} {}: {}: {}",
            super::PROGRAM,
            file.display(),
            self.path,
            self.code,
            self.reason
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_snake_case() {
        let all = [
            code::UNKNOWN_MODEL,
            code::UNKNOWN_AGENT,
            code::UNKNOWN_LANE,
            code::UNKNOWN_ENDPOINT,
            code::UNKNOWN_CREDENTIAL,
            code::UNKNOWN_EGRESS,
            code::UNKNOWN_WORKFLOW,
            code::PROTECTED_FIELD,
            code::BAD_BODY,
            code::BODY_TOO_LARGE,
            code::QUEUE_FULL,
            code::ATTEMPT_BUDGET,
            code::NO_CANDIDATE_AVAILABLE,
            code::SECRET_UNREADABLE,
            code::UPSTREAM_UNREACHABLE,
            code::STALE_CONFIG,
            code::PROTOCOL_UNSUPPORTED,
            code::NOT_IN_THIS_SLICE,
            code::EXPANSION_OVER_BOUND,
            code::EGRESS_SELF_LOOP,
            code::CREDENTIAL_ENDPOINT_MISMATCH,
            code::CLIENT_HOSTS_MISSING,
            code::OVER_BOUND,
            code::DUPLICATE_ID,
            code::BAD_CONFIG,
            code::CONFIG_UNREADABLE,
            code::GATE_TOKEN_UNREADABLE,
            code::MIRROR_PARENT_MISSING,
            code::NON_LOOPBACK_BIND,
        ];
        let mut sorted: Vec<&str> = all.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len(), "duplicate refusal code");
        for code in all {
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{code} is not snake_case"
            );
            assert!(!code.is_empty());
        }
    }

    #[test]
    fn refusal_renders_file_path_code_and_reason() {
        let refusal = Refusal::new("$[\"lanes\"][\"x\"][\"egress\"]", code::UNKNOWN_EGRESS, "no such egress");
        let line = refusal.render(std::path::Path::new("gate.jsonc"));
        assert!(line.contains("gate.jsonc"));
        assert!(line.contains("unknown_egress"));
        assert!(line.contains("minicon-gate:"));
    }
}
