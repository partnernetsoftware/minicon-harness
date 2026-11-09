// The wire surface: one handler onto the loopback server. Included by
// `config.rs`, which already imports `Value` and `code` for the parser.

use std::sync::Arc;

use super::server::{refusal_response, Handler, Request, Response, Server};

fn body_or_refusal(request: &Request) -> Result<Value, Response> {
    match serde_json::from_slice(&request.body) {
        Ok(value @ Value::Object(_)) => Ok(value),
        _ => Err(refusal_response(400, code::BAD_BODY, "the body is not one JSON object")),
    }
}

/// Builds the handler the loopback server calls.
pub fn handler(router: Arc<super::router::Router>) -> Arc<Handler> {
    Arc::new(move |request: &Request| match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/v1/chat/completions") => {
            if let Err(response) = authorize(&router, request) {
                return response;
            }
            super::router::chat_completions(&router, request)
        }
        ("GET", "/v1/models") => models(&router),
        ("GET", "/gate/v1/health") => health(&router),
        ("GET", "/gate/v1/pool") => pool(&router),
        ("GET", "/gate/v1/agents") => agents(&router),
        ("POST", "/gate/v1/resolve") => match body_or_refusal(request) {
            Ok(body) => resolve_body(&router, &body),
            Err(response) => response,
        },
        ("GET", "/gate/v1/workflows") => workflows(&router),
        ("GET", "/gate/v1/events") => events(&router),
        // `/gate/v1/*` is read-only by contract: no create/update/delete verb.
        (_, path) if path.starts_with("/gate/v1/") => {
            refusal_response(405, "method_not_allowed", "the inspect surface is read-only")
        }
        _ => refusal_response(404, "not_found", "no such path"),
    })
}

fn authorize(router: &super::router::Router, request: &Request) -> Result<(), Response> {
    let Some(expected) = &router.bearer else {
        return Ok(());
    };
    match request.header("authorization") {
        Some(value) => {
            let token = value.strip_prefix("Bearer ").unwrap_or(value);
            if constant_time_eq(token.as_bytes(), expected.as_bytes()) {
                Ok(())
            } else {
                Err(refusal_response(401, "unauthorized", "the gate bearer is not this token"))
            }
        }
        None => Err(refusal_response(401, "unauthorized", "the gate bearer is required")),
    }
}

/// Constant-time compare, so a token length or prefix cannot be timed out.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

fn models(router: &super::router::Router) -> Response {
    let mut data = Vec::new();
    for (id, _) in &router.config.config.agents {
        data.push(serde_json::json!({ "id": format!("gate/{id}"), "object": "model", "owned_by": "minicon-gate" }));
    }
    for (id, _) in &router.config.config.lanes {
        data.push(serde_json::json!({ "id": format!("gate/lane/{id}"), "object": "model", "owned_by": "minicon-gate" }));
    }
    Response::json(200, &serde_json::json!({ "object": "list", "data": data }))
}

fn health(router: &super::router::Router) -> Response {
    Response::json(
        200,
        &serde_json::json!({
            "status": "ok",
            "config_revision": router.config.revision,
            "config_path": router.config.path.to_string_lossy(),
        }),
    )
}

fn pool(router: &super::router::Router) -> Response {
    let config = &router.config.config;
    let endpoints: Vec<Value> = config
        .endpoints
        .iter()
        .map(|(id, endpoint)| {
            serde_json::json!({
                "id": id,
                "provider": endpoint.provider,
                "protocol": endpoint.protocol,
                "baseURL": endpoint.base_url,
            })
        })
        .collect();
    let credentials: Vec<Value> = config
        .credentials
        .iter()
        .map(|(id, credential)| {
            serde_json::json!({
                "id": id,
                "label": credential.label,
                "endpoint": credential.endpoint,
                "client": credential.client,
                "secret_shape": credential.secret.shape(),
            })
        })
        .collect();
    let egress: Vec<Value> = config
        .egress
        .iter()
        .map(|(id, entry)| {
            serde_json::json!({
                "id": id,
                "kind": match entry.proxy {
                    Proxy::Direct => "direct",
                    Proxy::Env => "env",
                    Proxy::Url { .. } => "url",
                },
                "host": super::router::host_of(entry.proxy.as_url()),
            })
        })
        .collect();
    let lanes: Vec<Value> = config
        .lanes
        .iter()
        .map(|(id, lane)| {
            serde_json::json!({
                "id": id,
                "endpoint": lane.endpoint,
                "model": lane.model,
                "credentialOrder": lane.credential_order,
                "egressOrder": lane.egress_order,
            })
        })
        .collect();
    Response::json(
        200,
        &serde_json::json!({
            "config_revision": router.config.revision,
            "endpoints": endpoints,
            "credentials": credentials,
            "egress": egress,
            "lanes": lanes,
        }),
    )
}

fn agents(router: &super::router::Router) -> Response {
    let data: Vec<Value> = router
        .config
        .config
        .agents
        .iter()
        .map(|(id, agent)| {
            serde_json::json!({
                "id": id,
                "lanes": agent.lanes,
                "select": agent.select,
                "budget": { "maxAttempts": agent.budget.max_attempts },
                "resolved": super::router::plan_for_agent(&router.config.config, id, &router.runtime).unwrap_or_default(),
            })
        })
        .collect();
    Response::json(200, &serde_json::json!({ "config_revision": router.config.revision, "agents": data }))
}

fn workflows(router: &super::router::Router) -> Response {
    let data: Vec<Value> = router
        .config
        .config
        .workflows
        .iter()
        .map(|(id, workflow)| {
            serde_json::json!({
                "id": id,
                "entry": workflow.entry,
                "nodes": workflow.nodes.iter().map(|(node, body)| serde_json::json!({
                    "id": node,
                    "agent": body.agent,
                    "egress": body.egress,
                })).collect::<Vec<_>>(),
                "edges": workflow.edges.iter().map(|edge| serde_json::json!({ "from": edge.from, "to": edge.to })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Response::json(200, &serde_json::json!({ "config_revision": router.config.revision, "workflows": data }))
}

fn resolve_body(router: &super::router::Router, body: &Value) -> Response {
    const PROTECTED: [&str; 6] = ["proxy", "credential", "endpoint", "baseURL", "protocol", "model"];
    if let Some(overrides) = body.get("overrides").and_then(Value::as_object) {
        for key in PROTECTED {
            if overrides.contains_key(key) {
                return refusal_response(
                    400,
                    code::PROTECTED_FIELD,
                    &format!("`{key}` is operator-only and cannot be overridden"),
                );
            }
        }
    }
    if let Some(agent_id) = body.get("agent").and_then(Value::as_str) {
        return match super::router::plan_for_agent(&router.config.config, agent_id, &router.runtime) {
            Ok(plan) => Response::json(
                200,
                &serde_json::json!({ "kind": "agent", "id": agent_id, "config_revision": router.config.revision, "plan": plan }),
            ),
            Err((status, code, reason)) => refusal_response(status, code, &reason),
        };
    }
    if let Some(workflow_id) = body.get("workflow").and_then(Value::as_str) {
        let Some((_, workflow)) = router
            .config
            .config
            .workflows
            .iter()
            .find(|(id, _)| id == workflow_id)
        else {
            return refusal_response(404, code::UNKNOWN_WORKFLOW, &format!("unknown workflow `{workflow_id}`"));
        };
        let mut nodes = Vec::new();
        for (node_id, node) in &workflow.nodes {
            let Some((_, agent)) = router
                .config
                .config
                .agents
                .iter()
                .find(|(id, _)| *id == node.agent)
            else {
                continue;
            };
            nodes.push(serde_json::json!({
                "id": node_id,
                "agent": node.agent,
                "plan": node_plan(&router.config.config, agent, node).iter().map(|candidate| super::router::candidate_value(candidate, None)).collect::<Vec<_>>(),
            }));
        }
        return Response::json(
            200,
            &serde_json::json!({
                "kind": "workflow",
                "id": workflow_id,
                "entry": workflow.entry,
                "config_revision": router.config.revision,
                "nodes": nodes,
            }),
        );
    }
    refusal_response(400, code::BAD_BODY, "`agent` or `workflow` is required")
}

fn events(router: &super::router::Router) -> Response {
    let evidence = router.evidence.lock().expect("evidence sink");
    let rows: Vec<Value> = evidence.rows().cloned().rev().take(200).collect();
    Response::json(200, &serde_json::json!({ "config_revision": router.config.revision, "events": rows }))
}

/// Binds the handler and returns the running server.
pub fn bind(
    router: Arc<super::router::Router>,
    workers: usize,
    allow_non_loopback: bool,
) -> Result<Server, String> {
    let _ = allow_non_loopback;
    super::server::Server::start(
        &router.config.config.listen.bind,
        workers,
        handler(Arc::clone(&router)),
    )
}
