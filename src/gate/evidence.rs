//! The transport plane's attempted work: one evidence row per attempt, a
//! bounded in-memory ring, and the optional JSONL mirror.
//!
//! The row shape is `PRD_01_27` "Evidence" verbatim. Three rules hold here and
//! nowhere else, so a reviewer has one place to check:
//!
//! 1. a row never contains a credential byte -- the fields are ids and a host;
//! 2. `triggered_by` records the chain, never adjacency;
//! 3. the mirror is required whenever the config names a path, because the ring
//!    cannot support offline recomputation after the process exits.

use serde_json::{json, Value};

use super::config::Candidate;
use super::refusal::code;

/// The six verdicts of the repo's vocabulary, mapped not reinvented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Upstream answered 2xx.
    Performed,
    /// Upstream refused, or a local refusal stopped the attempt.
    Refused,
    /// The attempt was skipped before dispatch.
    NotPerformed,
    /// Transport failure, timeout, unreadable body.
    Unknown,
    /// The lane or protocol cannot express the request.
    Unsupported,
    /// The config revision changed mid-request.
    Stale,
}

impl Verdict {
    /// The lower-case word a row carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Performed => "performed",
            Verdict::Refused => "refused",
            Verdict::NotPerformed => "not_performed",
            Verdict::Unknown => "unknown",
            Verdict::Unsupported => "unsupported",
            Verdict::Stale => "stale",
        }
    }

    /// Parses the word back; used by the reader tests.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "performed" => Some(Verdict::Performed),
            "refused" => Some(Verdict::Refused),
            "not_performed" => Some(Verdict::NotPerformed),
            "unknown" => Some(Verdict::Unknown),
            "unsupported" => Some(Verdict::Unsupported),
            "stale" => Some(Verdict::Stale),
            _ => None,
        }
    }
}

/// Which axis a cooling decision lands on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    /// The credential (429, 401/403, an unreadable secret).
    Credential,
    /// The egress (5xx, connect failure, proxy 407).
    Egress,
    /// The lane itself (an over-bound body).
    Lane,
}

impl Axis {
    /// The word a row carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Axis::Credential => "credential",
            Axis::Egress => "egress",
            Axis::Lane => "lane",
        }
    }
}

/// One attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attempt {
    /// Milliseconds since the Unix epoch.
    pub ts_ms: u128,
    /// The request this attempt belongs to.
    pub request_id: String,
    /// Position in the chain, starting at 1.
    pub attempt: u32,
    /// The attempt whose outcome started this one.
    pub triggered_by: Option<u32>,
    /// Why the previous attempt did not settle the request.
    pub trigger_reason: Option<&'static str>,
    /// Correlation ids, when the client supplied them.
    pub workflow_id: Option<String>,
    /// Correlation id.
    pub step_id: Option<String>,
    /// Correlation id.
    pub task_id: Option<String>,
    /// The agent or lane virtual model that was asked for.
    pub agent_id: String,
    /// The `model` field the client sent.
    pub virtual_model: String,
    /// The candidate this attempt used.
    pub candidate: Candidate,
    /// Egress host and port, never a URL: a proxy URL may carry credentials.
    pub egress_host: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Upstream status when there was one.
    pub status: Option<u16>,
    /// Named failure class when the attempt failed.
    pub error_class: Option<&'static str>,
    /// Which axis was cooled, and to what end, as decided by the mapping.
    pub cooled_axis: Option<(Axis, u64)>,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Token usage when the upstream reported it.
    pub usage: Option<(u64, u64)>,
    /// The config revision this attempt resolved under.
    pub config_revision: String,
    /// The lane definition revision.
    pub lane_revision: String,
}

impl Attempt {
    /// Renders the row. Field order is fixed so a mirror diff is readable.
    pub fn to_value(&self) -> Value {
        json!({
            "ts": self.ts_ms,
            "request_id": self.request_id,
            "attempt": self.attempt,
            "triggered_by": self.triggered_by,
            "trigger_reason": self.trigger_reason,
            "workflow_id": self.workflow_id,
            "step_id": self.step_id,
            "task_id": self.task_id,
            "agent_id": self.agent_id,
            "virtual_model": self.virtual_model,
            "endpoint_id": self.candidate.endpoint,
            "credential_id": self.candidate.credential,
            "egress_id": self.candidate.egress,
            "lane_id": self.candidate.lane,
            "model": self.candidate.model,
            "egress_host": self.egress_host,
            "verdict": self.verdict.as_str(),
            "status": self.status,
            "error_class": self.error_class,
            "cooled_axis": self.cooled_axis.map(|(axis, ms)| json!({ "axis": axis.as_str(), "cool_ms": ms })),
            "latency_ms": self.latency_ms,
            "usage": self.usage.map(|(input, output)| json!({ "input": input, "output": output })),
            "config_revision": self.config_revision,
            "lane_revision": self.lane_revision,
        })
    }
}

/// A bounded ring plus an optional JSONL mirror.
#[derive(Debug)]
pub struct Evidence {
    capacity: usize,
    rows: std::collections::VecDeque<Value>,
    mirror: Option<std::fs::File>,
    /// The path the mirror writes to, for diagnostics (never a secret).
    mirror_path: Option<String>,
}

impl Evidence {
    /// Builds the sink; `mirror` is a path whose parent the loader already
    /// checked.
    pub fn new(capacity: usize, mirror: Option<String>) -> Result<Self, String> {
        let file = match &mirror {
            None => None,
            Some(path) => Some(std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(|error| {
                format!(
                    "{}: {path}: {}: {error}",
                    super::PROGRAM,
                    code::MIRROR_PARENT_MISSING
                )
            })?),
        };
        Ok(Self {
            capacity: capacity.max(1),
            rows: std::collections::VecDeque::with_capacity(capacity.max(1)),
            mirror: file,
            mirror_path: mirror,
        })
    }

    /// Appends one row to the ring and, when configured, to the mirror.
    pub fn push(&mut self, attempt: &Attempt) -> Result<Value, String> {
        let row = attempt.to_value();
        if let Some(file) = self.mirror.as_mut() {
            use std::io::Write as _;
            writeln!(file, "{row}").map_err(|error| {
                format!(
                    "{}: {}: {}: {error}",
                    super::PROGRAM,
                    self.mirror_path.as_deref().unwrap_or("<mirror>"),
                    code::BAD_CONFIG
                )
            })?;
        }
        if self.rows.len() == self.capacity {
            self.rows.pop_front();
        }
        self.rows.push_back(row.clone());
        Ok(row)
    }

    /// The retained rows, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &Value> {
        self.rows.iter()
    }

    /// Retained row count.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The mirror path, when configured.
    pub fn mirror_path(&self) -> Option<&str> {
        self.mirror_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn candidate() -> Candidate {
        Candidate {
            lane: String::from("oa-sol"),
            endpoint: String::from("oa"),
            credential: String::from("oa-work"),
            egress: String::from("proxy8891"),
            model: String::from("gpt-6.1-sol"),
            model_args: json!({}),
        }
    }

    fn attempt() -> Attempt {
        Attempt {
            ts_ms: 1,
            request_id: String::from("r1"),
            attempt: 1,
            triggered_by: None,
            trigger_reason: None,
            workflow_id: None,
            step_id: None,
            task_id: None,
            agent_id: String::from("planner"),
            virtual_model: String::from("gate/planner"),
            candidate: candidate(),
            egress_host: String::from("127.0.0.1:8891"),
            verdict: Verdict::Performed,
            status: Some(200),
            error_class: None,
            cooled_axis: None,
            latency_ms: 12,
            usage: Some((10, 20)),
            config_revision: String::from("sha256:abc"),
            lane_revision: String::from("sha256:def"),
        }
    }

    #[test]
    fn verdict_words_round_trip() {
        for verdict in [
            Verdict::Performed,
            Verdict::Refused,
            Verdict::NotPerformed,
            Verdict::Unknown,
            Verdict::Unsupported,
            Verdict::Stale,
        ] {
            assert_eq!(Verdict::parse(verdict.as_str()), Some(verdict));
        }
        assert_eq!(Verdict::parse("performed "), None);
    }

    #[test]
    fn row_carries_axis_ids_and_a_host_not_a_url() {
        let row = attempt().to_value();
        assert_eq!(row["endpoint_id"], json!("oa"));
        assert_eq!(row["credential_id"], json!("oa-work"));
        assert_eq!(row["egress_id"], json!("proxy8891"));
        assert_eq!(row["egress_host"], json!("127.0.0.1:8891"));
        let text = row.to_string();
        assert!(!text.contains("user:pass"));
        assert!(!text.contains("http://"));
    }

    #[test]
    fn cooling_is_recorded_as_axis_and_duration() {
        let mut positive = attempt();
        positive.cooled_axis = Some((Axis::Egress, 30_000));
        let row = positive.to_value();
        assert_eq!(row["cooled_axis"], json!({ "axis": "egress", "cool_ms": 30000 }));
    }

    #[test]
    fn ring_drops_the_oldest_row_at_capacity() {
        let mut evidence = Evidence::new(2, None).expect("ring");
        let mut first = attempt();
        first.request_id = String::from("a");
        evidence.push(&first).expect("push");
        let mut second = attempt();
        second.request_id = String::from("b");
        evidence.push(&second).expect("push");
        let mut third = attempt();
        third.request_id = String::from("c");
        evidence.push(&third).expect("push");
        assert_eq!(evidence.len(), 2);
        let ids: Vec<String> = evidence
            .rows()
            .map(|row| row["request_id"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(ids, vec![String::from("b"), String::from("c")]);
    }

    #[test]
    fn mirror_writes_one_json_line_per_row() {
        let dir = std::env::temp_dir().join(format!("gate-mirror-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("events.jsonl");
        let path_text = path.to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        {
            let mut evidence = Evidence::new(10, Some(path_text.clone())).expect("mirror");
            evidence.push(&attempt()).expect("push");
            evidence.push(&attempt()).expect("push");
        }
        let text = std::fs::read_to_string(&path).expect("mirror file");
        assert_eq!(text.lines().count(), 2);
        for line in text.lines() {
            let row: Value = serde_json::from_str(line).expect("each mirror line is one JSON object");
            assert_eq!(row["verdict"], json!("performed"));
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
