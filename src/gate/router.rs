//! The router: turn one OpenAI-compatible request into one ordered attempt
//! chain, and settle it into evidence.
//!
//! Everything the gate *decides* lives here; everything it *transmits* lives in
//! `transport`. The split is the contract's three planes: resolution is pure,
//! transport is bounded, and this module is the seam that keeps the first from
//! touching a socket.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::config::{self, Agent, Candidate, CommandLine, GateConfig};
use super::evidence::{Attempt, Axis, Evidence, Verdict};
use super::map::{self, Disposition};
use super::refusal::code;
use super::server::{refusal_response, Request, Response};
use super::transport;

/// How long a cooling entry lasts; `None` means "until the config changes".
struct Cooling {
    until_ms: Option<u128>,
    reason: &'static str,
}

/// The live state the definition plane does not hold: cooling per axis.
#[derive(Default)]
pub struct Runtime {
    cooling: Mutex<HashMap<(Axis, String), Cooling>>,
}

impl Runtime {
    /// Whether `(axis, id)` is cooling at `now_ms`.
    pub fn is_cooling(&self, axis: Axis, id: &str, now_ms: u128) -> Option<&'static str> {
        let map = self.cooling.lock().expect("cooling map");
        match map.get(&(axis, id.to_owned())) {
            None => None,
            Some(entry) => match entry.until_ms {
                Some(until) if until <= now_ms => None,
                Some(_) => Some(entry.reason),
                None => Some(entry.reason),
            },
        }
    }

    /// Records one cooling decision.
    pub fn cool(&self, axis: Axis, id: &str, cool_ms: u64, reason: &'static str, now_ms: u128) {
        let mut map = self.cooling.lock().expect("cooling map");
        map.insert(
            (axis, id.to_owned()),
            Cooling {
                until_ms: if cool_ms == 0 { None } else { Some(now_ms + cool_ms as u128) },
                reason,
            },
        );
    }

    /// Drops every entry, used when the config revision changes.
    pub fn clear(&self) {
        self.cooling.lock().expect("cooling map").clear();
    }
}

/// Milliseconds since the Unix epoch; the cooling clock.
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// The resolved intent behind one request.
#[derive(Clone, Debug)]
pub struct Intent {
    /// The agent (or `None` for `gate/lane/<id>` which bypasses an agent).
    pub agent: Option<Agent>,
    /// The lane an explicit `gate/lane/<id>` named.
    pub lane: Option<String>,
    /// The `model` string the client sent.
    pub virtual_model: String,
    /// Ordered candidates after expansion.
    pub candidates: Vec<Candidate>,
    /// Attempt budget.
    pub max_attempts: u32,
}

/// Pulls the protected fields out of a request body so a caller cannot set them.
fn check_protected(body: &Value) -> Result<(), String> {
    const PROTECTED: [&str; 6] = ["proxy", "credential", "endpoint", "baseURL", "protocol", "model"];
    // Only an override block is inspected: the body's own `model` is the virtual
    // model and is handled by resolution, never forwarded.
    if let Some(overrides) = body.get("gate_overrides").and_then(Value::as_object) {
        for key in PROTECTED {
            if overrides.contains_key(key) {
                return Err(format!("`{key}` is operator-only and cannot be set by a request"));
            }
        }
    }
    Ok(())
}

/// Resolves a `model` string into an intent against one config.
pub fn resolve(config: &GateConfig, model: &str) -> Result<Intent, (u16, &'static str, String)> {
    if let Some(agent_id) = model.strip_prefix("gate/lane/") {
        let lane = config
            .lanes
            .iter()
            .find(|(id, _)| id == agent_id)
            .map(|(_, lane)| lane.clone())
            .ok_or_else(|| {
                (
                    404,
                    code::UNKNOWN_LANE,
                    format!("unknown lane `{agent_id}`"),
                )
            })?;
        let agent = Agent {
            id: String::from("<lane>"),
            lanes: vec![lane.id.clone()],
            select: String::from("ordered"),
            model_args: Value::Object(serde_json::Map::new()),
            budget: config::Budget {
                max_attempts: 3,
                max_output_tokens: None,
            },
        };
        let candidates = config::expand_agent(&agent, &config.lanes, &config.defaults.timeouts);
        return Ok(Intent {
            agent: None,
            lane: Some(lane.id),
            virtual_model: model.to_owned(),
            candidates,
            max_attempts: 3,
        });
    }
    let Some(agent_id) = model.strip_prefix("gate/") else {
        return Err((
            400,
            code::UNKNOWN_MODEL,
            format!("`{model}` is not `gate/<agent>` or `gate/lane/<id>`"),
        ));
    };
    let agent = config
        .agents
        .iter()
        .find(|(id, _)| id == agent_id)
        .map(|(_, agent)| agent.clone())
        .ok_or_else(|| {
            (
                400,
                code::UNKNOWN_AGENT,
                format!("unknown agent `{agent_id}`"),
            )
        })?;
    let candidates = config::expand_agent(&agent, &config.lanes, &config.defaults.timeouts);
    let mut resolved = Vec::with_capacity(candidates.len());
    for mut candidate in candidates {
        if let Some((_, credential)) = config
            .credentials
            .iter()
            .find(|(id, _)| *id == candidate.credential)
        {
            config::apply_credential_scope(&mut candidate, credential);
        }
        resolved.push(candidate);
    }
    Ok(Intent {
        agent: Some(agent.clone()),
        lane: None,
        virtual_model: model.to_owned(),
        candidates: resolved,
        max_attempts: agent.budget.max_attempts,
    })
}

/// Orders candidates for one request: cooling first-class, never dropped.
///
/// A cooling candidate is **moved to the back**, with the reason it moved, so
/// the reader recomputes demotion rather than absence. Ties keep declared order.
pub fn order(candidates: &[Candidate], runtime: &Runtime, now: u128) -> Vec<(Candidate, Option<&'static str>)> {
    let mut warm = Vec::new();
    let mut cooling = Vec::new();
    for candidate in candidates {
        let reason = runtime
            .is_cooling(Axis::Credential, &candidate.credential, now)
            .or_else(|| runtime.is_cooling(Axis::Egress, &candidate.egress, now));
        match reason {
            None => warm.push((candidate.clone(), None)),
            Some(reason) => cooling.push((candidate.clone(), Some(reason))),
        }
    }
    warm.extend(cooling);
    warm
}

/// The ordered candidate plan for one agent, without dispatching anything.
pub fn plan_for_agent(config: &GateConfig, agent_id: &str, runtime: &Runtime) -> Result<Vec<Value>, (u16, &'static str, String)> {
    let agent = config
        .agents
        .iter()
        .find(|(id, _)| id == agent_id)
        .map(|(_, agent)| agent.clone())
        .ok_or_else(|| (400, code::UNKNOWN_AGENT, format!("unknown agent `{agent_id}`")))?;
    let now = now_ms();
    let candidates = config::expand_agent(&agent, &config.lanes, &config.defaults.timeouts);
    Ok(candidate_values(&order(&candidates, runtime, now)))
}

/// Renders ordered candidates as the inspect JSON, with the demotion reason
/// when one applies.
pub fn candidate_values(ordered: &[(Candidate, Option<&'static str>)]) -> Vec<Value> {
    ordered
        .iter()
        .map(|(candidate, cooling_reason)| candidate_value(candidate, *cooling_reason))
        .collect()
}

/// Renders one candidate, optionally with a demotion reason.
pub fn candidate_value(candidate: &Candidate, cooling_reason: Option<&'static str>) -> Value {
    let mut value = serde_json::json!({
        "lane": candidate.lane,
        "endpoint": candidate.endpoint,
        "credential": candidate.credential,
        "egress": candidate.egress,
        "model": candidate.model,
        "modelArgs": candidate.model_args,
    });
    if let Some(reason) = cooling_reason {
        value["demoted"] = Value::String(reason.to_owned());
    }
    value
}

/// What the router needs from the process to do its job.
pub struct Router {
    /// The loaded config and its revision.
    pub config: config::LoadedConfig,
    /// Cooling state.
    pub runtime: Runtime,
    /// Evidence sink.
    pub evidence: Mutex<Evidence>,
    /// Optional local bearer for the client-to-gate hop.
    pub bearer: Option<String>,
    /// The command line that started the gate (for proxy env and secrets).
    pub command_line: CommandLine,
}

/// Handles one `/v1/chat/completions` request end to end.
pub fn chat_completions(router: &Router, request: &Request) -> Response {
    let body: Value = match serde_json::from_slice(&request.body) {
        Ok(value @ Value::Object(_)) => value,
        _ => {
            return refusal_response(400, code::BAD_BODY, "the body is not one JSON object");
        }
    };
    if let Err(reason) = check_protected(&body) {
        return refusal_response(400, code::PROTECTED_FIELD, &reason);
    }
    let model = match body.get("model").and_then(Value::as_str) {
        Some(model) => model.to_owned(),
        None => return refusal_response(400, code::BAD_BODY, "`model` is required"),
    };
    let intent = match resolve(&router.config.config, &model) {
        Ok(intent) => intent,
        Err((status, code, reason)) => return refusal_response(status, code, &reason),
    };
    dispatch(router, request, intent, body)
}

fn dispatch(router: &Router, request: &Request, intent: Intent, body: Value) -> Response {
    let now = now_ms();
    let ordered = order(&intent.candidates, &router.runtime, now);
    let request_id = format!("{}", uuid_like(now, request));
    let mut evidence = router.evidence.lock().expect("evidence sink");
    let mut cursor = Instant::now();
    let mut previous: Option<u32> = None;
    let mut trigger: Option<&'static str> = None;
    let mut attempts = 0u32;
    for (candidate, cooling_reason) in ordered.iter() {
        if attempts >= intent.max_attempts {
            break;
        }
        attempts += 1;
        let number = attempts;
        let Some((_, lane)) = router
            .config
            .config
            .lanes
            .iter()
            .find(|(id, _)| *id == candidate.lane)
        else {
            continue;
        };
        let Some((_, endpoint)) = router
            .config
            .config
            .endpoints
            .iter()
            .find(|(id, _)| *id == candidate.endpoint)
        else {
            continue;
        };
        let Some((_, egress)) = router
            .config
            .config
            .egress
            .iter()
            .find(|(id, _)| *id == candidate.egress)
        else {
            continue;
        };
        let Some((_, credential)) = router
            .config
            .config
            .credentials
            .iter()
            .find(|(id, _)| *id == candidate.credential)
        else {
            continue;
        };
        let caller_credential = request
            .header("x-gate-upstream-authorization")
            .map(str::to_owned);
        let secret = match router
            .command_line
            .read_secret(&credential.secret, credential.client, caller_credential)
        {
            Ok(secret) => secret,
            Err(reason) => {
                let disposition = map::SECRET_UNREADABLE;
                record(
                    &mut evidence,
                    router,
                    RecordInput {
                        ts_ms: now,
                        request_id: &request_id,
                        number,
                        previous,
                        trigger,
                        intent: &intent,
                        candidate,
                        egress_host: host_of(egress.proxy.as_url()),
                        disposition,
                        status: None,
                        latency_ms: cursor.elapsed().as_millis() as u64,
                    },
                );
                if let Some((axis, cool_ms)) = disposition.cool {
                    router
                        .runtime
                        .cool(axis, axis_id(axis, candidate), cool_ms, disposition.reason, now);
                }
                if !disposition.fallback {
                    return refusal_response(502, code::SECRET_UNREADABLE, &reason);
                }
                previous = Some(number);
                trigger = Some(disposition.reason);
                cursor = Instant::now();
                continue;
            }
        };
        let outcome = transport::post_chat(&transport::Target {
            endpoint,
            egress,
            credential,
            lane,
            secret,
            model: &candidate.model,
            model_args: &candidate.model_args,
            body: &body,
            command_line: &router.command_line,
        });
        let disposition = outcome.disposition();
        let latency = cursor.elapsed().as_millis() as u64;
        record(
            &mut evidence,
            router,
            RecordInput {
                ts_ms: now,
                request_id: &request_id,
                number,
                previous,
                trigger,
                intent: &intent,
                candidate,
                egress_host: host_of(egress.proxy.as_url()),
                disposition,
                status: outcome.status(),
                latency_ms: latency,
            },
        );
        if let Some((axis, cool_ms)) = disposition.cool {
            router
                .runtime
                .cool(axis, axis_id(axis, candidate), cool_ms, disposition.reason, now);
        }
        match outcome {
            transport::Outcome::Answered {
                status,
                body: response_body,
                headers,
            } if (200..=299).contains(&status) => {
                let mut response = Response {
                    status,
                    headers: vec![(
                        String::from("Content-Type"),
                        headers
                            .iter()
                            .find(|(name, _)| name == "content-type")
                            .map(|(_, value)| value.clone())
                            .unwrap_or_else(|| String::from("application/json")),
                    )],
                    body: response_body,
                };
                response.headers.extend(evidence_headers(candidate, number, &router.config.revision, host_of(egress.proxy.as_url())));
                return response;
            }
            transport::Outcome::Answered { status, body: response_body, .. } => {
                // An upstream refusal passes through verbatim with the evidence
                // headers added: the gate never rewrites an upstream answer.
                let mut response = Response {
                    status,
                    headers: vec![(
                        String::from("Content-Type"),
                        String::from("application/json"),
                    )],
                    body: response_body,
                };
                response.headers.extend(evidence_headers(candidate, number, &router.config.revision, host_of(egress.proxy.as_url())));
                return response;
            }
            transport::Outcome::Failed { reason, .. } => {
                previous = Some(number);
                trigger = Some(disposition.reason);
                cursor = Instant::now();
                if cooling_reason.is_some() && !disposition.fallback {
                    return refusal_response(502, code::UPSTREAM_UNREACHABLE, &reason);
                }
                if !disposition.fallback {
                    return refusal_response(502, code::UPSTREAM_UNREACHABLE, &reason);
                }
            }
        }
    }
    refusal_response(
        503,
        code::NO_CANDIDATE_AVAILABLE,
        "every candidate was cooling, refused, or retried",
    )
}

fn axis_id(axis: Axis, candidate: &Candidate) -> &str {
    match axis {
        Axis::Credential => &candidate.credential,
        Axis::Egress => &candidate.egress,
        Axis::Lane => &candidate.lane,
    }
}

fn evidence_headers(candidate: &Candidate, attempt: u32, revision: &str, egress_host: String) -> Vec<(String, String)> {
    vec![
        (String::from("x-gate-lane"), candidate.lane.clone()),
        (String::from("x-gate-attempt"), attempt.to_string()),
        (String::from("x-gate-config-revision"), revision.to_owned()),
        (String::from("x-gate-egress"), egress_host),
    ]
}

/// The host and port of a proxy URL, with any userinfo removed.
pub fn host_of(url: Option<String>) -> String {
    let Some(url) = url else {
        return String::new();
    };
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(&url);
    let authority = after_scheme.split('/').next().unwrap_or("");
    let host_and_port = authority.rsplit_once('@').map(|(_, rest)| rest).unwrap_or(authority);
    host_and_port.to_owned()
}

struct RecordInput<'a> {
    ts_ms: u128,
    request_id: &'a str,
    number: u32,
    previous: Option<u32>,
    trigger: Option<&'static str>,
    intent: &'a Intent,
    candidate: &'a Candidate,
    egress_host: String,
    disposition: Disposition,
    status: Option<u16>,
    latency_ms: u64,
}

fn record(evidence: &mut Evidence, router: &Router, input: RecordInput<'_>) {
    let attempt = Attempt {
        ts_ms: input.ts_ms,
        request_id: input.request_id.to_owned(),
        attempt: input.number,
        triggered_by: input.previous,
        trigger_reason: input.trigger,
        workflow_id: None,
        step_id: None,
        task_id: None,
        agent_id: input
            .intent
            .agent
            .as_ref()
            .map(|agent| agent.id.clone())
            .unwrap_or_else(|| String::from("<lane>")),
        virtual_model: input.intent.virtual_model.clone(),
        candidate: input.candidate.clone(),
        egress_host: input.egress_host,
        verdict: input.disposition.verdict,
        status: input.status,
        error_class: if input.disposition.verdict == Verdict::Performed {
            None
        } else {
            Some(input.disposition.reason)
        },
        cooled_axis: input.disposition.cool,
        latency_ms: input.latency_ms,
        usage: None,
        config_revision: router.config.revision.clone(),
        lane_revision: router.config.revision.clone(),
    };
    let _ = evidence.push(&attempt);
}

fn uuid_like(now: u128, request: &Request) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let body = &request.body[..request.body.len().min(4096)];
    for byte in now.to_le_bytes().iter().chain(body.iter()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}{:08x}", now as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_hosts_drop_userinfo_and_path() {
        assert_eq!(host_of(Some(String::from("http://user:pass@127.0.0.1:8891"))), "127.0.0.1:8891");
        assert_eq!(host_of(Some(String::from("http://10.0.0.2:3128/"))), "10.0.0.2:3128");
        assert_eq!(host_of(None), "");
    }

    #[test]
    fn resolve_rejects_a_model_outside_the_namespace() {
        let config = config::parse(MINIMAL, false).expect("config");
        let error = resolve(&config, "gpt-4").expect_err("must refuse");
        assert_eq!(error.1, code::UNKNOWN_MODEL);
    }

    #[test]
    fn resolve_reads_both_virtual_models() {
        let config = config::parse(MINIMAL, false).expect("config");
        let by_agent = resolve(&config, "gate/planner").expect("agent");
        assert_eq!(by_agent.candidates.len(), 4, "2 credentials x 2 egress");
        let by_lane = resolve(&config, "gate/lane/oa").expect("lane");
        assert_eq!(by_lane.candidates.len(), 4);
        assert!(by_lane.agent.is_none(), "a lane model bypasses the agent");
    }

    #[test]
    fn cooling_demotes_instead_of_dropping() {
        let config = config::parse(MINIMAL, false).expect("config");
        let intent = resolve(&config, "gate/planner").expect("agent");
        let runtime = Runtime::default();
        runtime.cool(Axis::Egress, "proxy", 60_000, "upstream_5xx", now_ms());
        let ordered = order(&intent.candidates, &runtime, now_ms());
        // 2 credentials x 2 egress = 4 candidates; none is dropped.
        assert_eq!(ordered.len(), 4, "no candidate is dropped");
        let demoted: Vec<&Candidate> = ordered
            .iter()
            .filter(|(_, reason)| reason.is_some())
            .map(|(candidate, _)| candidate)
            .collect();
        let warm: Vec<&Candidate> = ordered
            .iter()
            .filter(|(_, reason)| reason.is_none())
            .map(|(candidate, _)| candidate)
            .collect();
        assert_eq!(warm.len(), 2, "the two direct candidates stay warm");
        assert_eq!(demoted.len(), 2, "the two proxied candidates move back");
        assert!(warm.iter().all(|candidate| candidate.egress == "direct"));
        assert!(demoted.iter().all(|candidate| candidate.egress == "proxy"));
        assert_eq!(ordered[3].1, Some("upstream_5xx"), "declared order is kept inside each group");
    }

    #[test]
    fn a_config_change_clears_cooling() {
        let runtime = Runtime::default();
        runtime.cool(Axis::Credential, "c", 0, "secret_unreadable", now_ms());
        assert!(runtime.is_cooling(Axis::Credential, "c", now_ms()).is_some());
        runtime.clear();
        assert!(runtime.is_cooling(Axis::Credential, "c", now_ms()).is_none());
    }

    const MINIMAL: &str = r#"{
      "endpoints": { "oa": { "provider": "openai", "protocol": "openai-chat", "baseURL": "http://127.0.0.1:1/v1" } },
      "credentials": {
        "c1": { "label": "c1", "secret": { "env": "GATE_TEST_KEY_ONE" } },
        "c2": { "label": "c2", "secret": { "env": "GATE_TEST_KEY_TWO" } }
      },
      "egress": { "direct": { "proxy": "direct" }, "proxy": { "proxy": { "url": "http://127.0.0.1:9" } } },
      "lanes": { "oa": { "endpoint": "oa", "model": "m", "credentialOrder": ["c1", "c2"], "egressOrder": ["direct", "proxy"] } },
      "agents": { "planner": { "lanes": ["oa"] } }
    }"#;
}
