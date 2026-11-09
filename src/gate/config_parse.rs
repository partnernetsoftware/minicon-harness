// Parser half of the definition plane. Included by `config.rs` so the plane
// stays one unit without a public module for every struct.
//
// Strict parse: an unknown field is a refusal, never ignored, so a typo in a
// lane cannot silently leave a default in place.

fn json_path(parts: &[&str]) -> String {
    let mut out = String::from("$");
    for part in parts {
        out.push('[');
        out.push_str(&format!("{}", serde_json::Value::String((*part).to_owned())));
        out.push(']');
    }
    out
}

fn as_object<'a>(
    value: &'a Value,
    path: &[&str],
    out: &mut Vec<Refusal>,
) -> Option<&'a serde_json::Map<String, Value>> {
    match value.as_object() {
        Some(object) => Some(object),
        None => {
            out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                "must be an object",
            ));
            None
        }
    }
}

fn string(value: &Value, path: &[&str], field: &str, out: &mut Vec<Refusal>) -> Option<String> {
    match value.get(field).and_then(Value::as_str) {
        Some(text) => Some(text.to_owned()),
        None => {
            out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                format!("`{field}` must be a string"),
            ));
            None
        }
    }
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn string_list(value: &Value, path: &[&str], field: &str, out: &mut Vec<Refusal>) -> Vec<String> {
    match value.get(field) {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            let mut list = Vec::with_capacity(items.len());
            for item in items {
                match item.as_str() {
                    Some(text) => list.push(text.to_owned()),
                    None => out.push(Refusal::new(
                        json_path(path),
                        code::BAD_CONFIG,
                        format!("`{field}` entries must be strings"),
                    )),
                }
            }
            list
        }
        Some(_) => {
            out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                format!("`{field}` must be an array"),
            ));
            Vec::new()
        }
    }
}

fn check_fields(
    object: &serde_json::Map<String, Value>,
    path: &[&str],
    allowed: &[&str],
    out: &mut Vec<Refusal>,
) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                format!("unknown field `{key}`"),
            ));
        }
    }
}

fn timeouts_of(value: &Value, path: &[&str], base: Timeouts, out: &mut Vec<Refusal>) -> Timeouts {
    let Some(object) = value.as_object() else {
        return base;
    };
    let mut number = |field: &str, fallback: u64| -> u64 {
        match object.get(field) {
            None => fallback,
            Some(item) => match item.as_u64() {
                Some(n) => n,
                None => {
                    out.push(Refusal::new(
                        json_path(path),
                        code::BAD_CONFIG,
                        format!("`{field}` must be a non-negative integer"),
                    ));
                    fallback
                }
            },
        }
    };
    Timeouts {
        header_ms: number("headerMs", base.header_ms),
        chunk_ms: number("chunkMs", base.chunk_ms),
    }
}

fn default_model_args() -> Value {
    Value::Object(serde_json::Map::new())
}

fn model_args_of(value: &Value, path: &[&str], field: &str, out: &mut Vec<Refusal>) -> Value {
    match value.get(field) {
        None => default_model_args(),
        Some(Value::Object(_)) => value[field].clone(),
        Some(_) => {
            out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                format!("`{field}` must be an object"),
            ));
            default_model_args()
        }
    }
}

/// Parses one config text into the definition plane, collecting every refusal
/// rather than stopping at the first, so one run reports the whole config.
pub fn parse(text: &str, allow_non_loopback: bool) -> Result<GateConfig, Vec<Refusal>> {
    let mut refusals = Vec::new();
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            return Err(vec![Refusal::new(
                "$",
                code::BAD_CONFIG,
                format!("not JSON: {error}"),
            )]);
        }
    };
    let root_path: [&str; 0] = [];
    let Some(root) = as_object(&value, &root_path, &mut refusals) else {
        return Err(refusals);
    };
    check_fields(
        root,
        &root_path,
        &[
            "listen",
            "defaults",
            "events",
            "endpoints",
            "credentials",
            "egress",
            "lanes",
            "agents",
            "workflows",
        ],
        &mut refusals,
    );

    let listen = parse_listen(root.get("listen"), allow_non_loopback, &mut refusals);
    let defaults = parse_defaults(root.get("defaults"), &mut refusals);
    let events = parse_events(root.get("events"), &mut refusals);
    let endpoints = parse_endpoints(root.get("endpoints"), &mut refusals);
    let credentials = parse_credentials(root.get("credentials"), &mut refusals);
    let egress = parse_egress(root.get("egress"), &mut refusals);
    let lanes = parse_lanes(root.get("lanes"), &mut refusals);
    let agents = parse_agents(root.get("agents"), &mut refusals);
    let workflows = parse_workflows(root.get("workflows"), &mut refusals);

    check_bounds(&endpoints, &credentials, &egress, &lanes, &agents, &mut refusals);

    // Cross-axis references. A lane may only name members that exist, and a
    // credential may only serve the endpoint it belongs to.
    let endpoint_ids: Vec<&str> = endpoints.iter().map(|(id, _)| id.as_str()).collect();
    let credential_ids: Vec<&str> = credentials.iter().map(|(id, _)| id.as_str()).collect();
    let egress_ids: Vec<&str> = egress.iter().map(|(id, _)| id.as_str()).collect();
    let lane_ids: Vec<&str> = lanes.iter().map(|(id, _)| id.as_str()).collect();
    let agent_ids: Vec<&str> = agents.iter().map(|(id, _)| id.as_str()).collect();

    for (index, (id, lane)) in lanes.iter().enumerate() {
        let lane_path = [format!("lanes"), id.clone()];
        let lane_ref: Vec<&str> = lane_path.iter().map(String::as_str).collect();
        if !endpoint_ids.contains(&lane.endpoint.as_str()) {
            refusals.push(Refusal::new(
                json_path(&lane_ref),
                code::UNKNOWN_ENDPOINT,
                format!("lane `{id}` names endpoint `{}`", lane.endpoint),
            ));
        }
        for credential in &lane.credential_order {
            if !credential_ids.contains(&credential.as_str()) {
                refusals.push(Refusal::new(
                    json_path(&lane_ref),
                    code::UNKNOWN_CREDENTIAL,
                    format!("lane `{id}` names credential `{credential}`"),
                ));
                continue;
            }
            // A credential bound to one endpoint may only serve that endpoint.
            if let Some((_, bound)) = credentials
                .iter()
                .find(|(credential_id, _)| credential_id == credential)
            {
                if let Some(endpoint) = &bound.endpoint {
                    if endpoint != &lane.endpoint {
                        refusals.push(Refusal::new(
                            json_path(&lane_ref),
                            code::CREDENTIAL_ENDPOINT_MISMATCH,
                            format!(
                                "credential `{credential}` belongs to endpoint `{endpoint}` but lane `{id}` uses `{}`",
                                lane.endpoint
                            ),
                        ));
                    }
                }
            }
        }
        for egress_id in &lane.egress_order {
            if !egress_ids.contains(&egress_id.as_str()) {
                refusals.push(Refusal::new(
                    json_path(&lane_ref),
                    code::UNKNOWN_EGRESS,
                    format!("lane `{id}` names egress `{egress_id}`"),
                ));
            }
        }
        if lane.egress_order.is_empty() {
            refusals.push(Refusal::new(
                json_path(&lane_ref),
                code::BAD_CONFIG,
                format!("lane `{id}` has an empty egress list"),
            ));
        }
        if lane.credential_order.is_empty() {
            refusals.push(Refusal::new(
                json_path(&lane_ref),
                code::BAD_CONFIG,
                format!("lane `{id}` has an empty credential list"),
            ));
        }
        let _ = index;
    }

    for (id, agent) in &agents {
        let agent_path = [String::from("agents"), id.clone()];
        let agent_ref: Vec<&str> = agent_path.iter().map(String::as_str).collect();
        if agent.lanes.is_empty() {
            refusals.push(Refusal::new(
                json_path(&agent_ref),
                code::BAD_CONFIG,
                format!("agent `{id}` has no lanes"),
            ));
        }
        for lane in &agent.lanes {
            if !lane_ids.contains(&lane.as_str()) {
                refusals.push(Refusal::new(
                    json_path(&agent_ref),
                    code::UNKNOWN_LANE,
                    format!("agent `{id}` names lane `{lane}`"),
                ));
            }
        }
        // The expanded candidate bound is checked here so a config that cannot
        // route is refused before the gate ever starts.
        let expanded = expand_agent(agent, &lanes, &defaults.timeouts);
        if expanded.len() > MAX_CANDIDATES {
            refusals.push(Refusal::new(
                json_path(&agent_ref),
                code::EXPANSION_OVER_BOUND,
                format!(
                    "agent `{id}` expands to {} candidates, over the {MAX_CANDIDATES} bound",
                    expanded.len()
                ),
            ));
        }
    }

    for (id, workflow) in &workflows {
        let workflow_path = [String::from("workflows"), id.clone()];
        let workflow_ref: Vec<&str> = workflow_path.iter().map(String::as_str).collect();
        let node_ids: Vec<&str> = workflow.nodes.iter().map(|(node, _)| node.as_str()).collect();
        if !node_ids.contains(&workflow.entry.as_str()) {
            refusals.push(Refusal::new(
                json_path(&workflow_ref),
                code::BAD_CONFIG,
                format!("workflow `{id}` entry `{}` is not a node", workflow.entry),
            ));
        }
        for (node, body) in &workflow.nodes {
            if !agent_ids.contains(&body.agent.as_str()) {
                refusals.push(Refusal::new(
                    json_path(&workflow_ref),
                    code::UNKNOWN_AGENT,
                    format!("workflow `{id}` node `{node}` names agent `{}`", body.agent),
                ));
            }
            if let Some(egress_id) = &body.egress {
                if !egress_ids.contains(&egress_id.as_str()) {
                    refusals.push(Refusal::new(
                        json_path(&workflow_ref),
                        code::UNKNOWN_EGRESS,
                        format!("workflow `{id}` node `{node}` names egress `{egress_id}`"),
                    ));
                }
            }
        }
        for edge in &workflow.edges {
            if !node_ids.contains(&edge.from.as_str()) || !node_ids.contains(&edge.to.as_str()) {
                refusals.push(Refusal::new(
                    json_path(&workflow_ref),
                    code::BAD_CONFIG,
                    format!("workflow `{id}` edge {}->{} names an unknown node", edge.from, edge.to),
                ));
            }
        }
        if has_cycle(workflow) {
            refusals.push(Refusal::new(
                json_path(&workflow_ref),
                code::BAD_CONFIG,
                format!("workflow `{id}` has a cycle; `allowCycle` is not part of 0.0.20"),
            ));
        }
    }

    for (id, credential) in &credentials {
        if credential.client && credential.allowed_hosts.is_empty() {
            refusals.push(Refusal::new(
                json_path(&["credentials", id.as_str()]),
                code::CLIENT_HOSTS_MISSING,
                format!("client credential `{id}` must name `allowedHosts`"),
            ));
        }
    }

    if !refusals.is_empty() {
        return Err(refusals);
    }
    Ok(GateConfig {
        listen,
        defaults,
        events,
        endpoints,
        credentials,
        egress,
        lanes,
        agents,
        workflows,
    })
}

fn check_bounds(
    endpoints: &[(String, Endpoint)],
    credentials: &[(String, Credential)],
    egress: &[(String, Egress)],
    lanes: &[(String, Lane)],
    agents: &[(String, Agent)],
    out: &mut Vec<Refusal>,
) {
    let pairs: [(&str, usize, usize); 5] = [
        ("endpoints", endpoints.len(), MAX_ENDPOINTS),
        ("credentials", credentials.len(), MAX_CREDENTIALS),
        ("egress", egress.len(), MAX_EGRESS),
        ("lanes", lanes.len(), MAX_LANES),
        ("agents", agents.len(), MAX_AGENTS),
    ];
    for (name, count, bound) in pairs {
        if count > bound {
            out.push(Refusal::new(
                json_path(&[name]),
                code::OVER_BOUND,
                format!("{name} has {count} entries, over the {bound} bound"),
            ));
        }
    }
}

fn has_cycle(workflow: &Workflow) -> bool {
    // Kahn's algorithm: repeatedly remove every node that still has an incoming
    // edge from a node that is still present. A node that survives every pass
    // is on, or behind, a cycle.
    let mut remaining: Vec<&str> = workflow.nodes.iter().map(|(id, _)| id.as_str()).collect();
    loop {
        let removable: Vec<&str> = remaining
            .iter()
            .copied()
            .filter(|node| {
                !workflow
                    .edges
                    .iter()
                    .any(|edge| edge.to == *node && remaining.contains(&edge.from.as_str()))
            })
            .collect();
        if removable.is_empty() {
            break;
        }
        remaining.retain(|node| !removable.contains(node));
    }
    !remaining.is_empty()
}

fn parse_listen(value: Option<&Value>, allow_non_loopback: bool, out: &mut Vec<Refusal>) -> Listen {
    let path = ["listen"];
    let Some(value) = value else {
        return Listen {
            bind: String::from("127.0.0.1:0"),
            auth: None,
        };
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return Listen {
            bind: String::from("127.0.0.1:0"),
            auth: None,
        };
    };
    check_fields(parsed, &path, &["bind", "auth"], out);
    let bind = string(value, &path, "bind", out).unwrap_or_else(|| String::from("127.0.0.1:0"));
    if !allow_non_loopback && !is_loopback_bind(&bind) {
        out.push(Refusal::new(
            json_path(&path),
            code::NON_LOOPBACK_BIND,
            format!("bind `{bind}` is not loopback; pass --allow-non-loopback to accept the risk"),
        ));
    }
    let auth = match value.get("auth") {
        None | Some(Value::Null) => None,
        Some(auth) => {
            let auth_object = auth.as_object();
            match auth_object.and_then(|a| a.get("secret")) {
                Some(secret) => SecretRef::parse(secret, &json_path(&path), out),
                None => {
                    out.push(Refusal::new(
                        json_path(&path),
                        code::BAD_CONFIG,
                        "`auth.secret` is required when `auth` is present",
                    ));
                    None
                }
            }
        }
    };
    Listen { bind, auth }
}

/// Whether a bind string names a loopback host.
pub fn is_loopback_bind(bind: &str) -> bool {
    let host = match bind.rsplit_once(':') {
        Some((host, _port)) => host.trim_matches(|c| c == '[' || c == ']'),
        None => bind,
    };
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

fn parse_defaults(value: Option<&Value>, out: &mut Vec<Refusal>) -> Defaults {
    let path = ["defaults"];
    let mut defaults = Defaults {
        timeouts: Timeouts {
            header_ms: 300_000,
            chunk_ms: 900_000,
        },
        model_args: default_model_args(),
    };
    let Some(value) = value else {
        return defaults;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return defaults;
    };
    check_fields(parsed, &path, &["timeout", "modelArgs"], out);
    if let Some(timeout) = value.get("timeout") {
        defaults.timeouts = timeouts_of(timeout, &path, defaults.timeouts, out);
    }
    defaults.model_args = model_args_of(value, &path, "modelArgs", out);
    defaults
}

fn parse_events(value: Option<&Value>, out: &mut Vec<Refusal>) -> Events {
    let path = ["events"];
    let mut events = Events {
        ring: 10_000,
        mirror: None,
    };
    let Some(value) = value else {
        return events;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return events;
    };
    check_fields(parsed, &path, &["ring", "mirror"], out);
    if let Some(ring) = value.get("ring") {
        match ring.as_u64() {
            Some(n) if n > 0 && n <= 10_000 => events.ring = n as usize,
            Some(n) => out.push(Refusal::new(
                json_path(&path),
                code::OVER_BOUND,
                format!("`ring` = {n} is outside 1..=10000"),
            )),
            None => out.push(Refusal::new(
                json_path(&path),
                code::BAD_CONFIG,
                "`ring` must be an integer",
            )),
        }
    }
    if let Some(mirror) = value.get("mirror") {
        match mirror.as_str() {
            Some(path) if !path.is_empty() => {
                let parent = std::path::Path::new(path)
                    .parent()
                    .map(std::path::Path::to_path_buf);
                let exists = parent.map(|p| p.as_os_str().is_empty() || p.exists());
                if exists != Some(true) {
                    out.push(Refusal::new(
                        json_path(&["events"]),
                        code::MIRROR_PARENT_MISSING,
                        "the mirror's parent directory does not exist",
                    ));
                }
                events.mirror = Some(path.to_owned());
            }
            _ => out.push(Refusal::new(
                json_path(&path),
                code::BAD_CONFIG,
                "`mirror` must be a non-empty string",
            )),
        }
    }
    events
}

fn parse_endpoints(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Endpoint)> {
    let path = ["endpoints"];
    let mut endpoints = Vec::new();
    let Some(value) = value else {
        return endpoints;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return endpoints;
    };
    for (id, body) in parsed {
        let entry_path = ["endpoints", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(body_object, &entry_path, &["provider", "protocol", "baseURL", "headers"], out);
        let provider = string(body, &entry_path, "provider", out).unwrap_or_default();
        let protocol = string(body, &entry_path, "protocol", out).unwrap_or_default();
        if protocol != "openai-chat" {
            out.push(Refusal::new(
                json_path(&entry_path),
                code::PROTOCOL_UNSUPPORTED,
                format!("protocol `{protocol}` is not `openai-chat` in 0.0.20"),
            ));
        }
        let base_url = string(body, &entry_path, "baseURL", out).unwrap_or_default();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            out.push(Refusal::new(
                json_path(&entry_path),
                code::BAD_CONFIG,
                "`baseURL` must start with http:// or https://",
            ));
        }
        let mut headers = Vec::new();
        if let Some(values) = body.get("headers") {
            match values.as_object() {
                Some(map) => {
                    for (name, header) in map {
                        match header.as_str() {
                            Some(text) => headers.push((name.clone(), text.to_owned())),
                            None => out.push(Refusal::new(
                                json_path(&entry_path),
                                code::BAD_CONFIG,
                                format!("header `{name}` must be a string"),
                            )),
                        }
                    }
                }
                None => out.push(Refusal::new(
                    json_path(&entry_path),
                    code::BAD_CONFIG,
                    "`headers` must be an object",
                )),
            }
        }
        endpoints.push((
            id.clone(),
            Endpoint {
                provider,
                protocol,
                base_url,
                headers,
            },
        ));
    }
    endpoints
}

fn parse_credentials(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Credential)> {
    let path = ["credentials"];
    let mut credentials = Vec::new();
    let Some(value) = value else {
        return credentials;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return credentials;
    };
    for (id, body) in parsed {
        let entry_path = ["credentials", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(
            body_object,
            &entry_path,
            &["endpoint", "label", "secret", "scope", "client", "allowedHosts"],
            out,
        );
        let client = body.get("client").and_then(Value::as_bool).unwrap_or(false);
        let label = optional_string(body, "label").unwrap_or_default();
        let endpoint = optional_string(body, "endpoint");
        let scope = body.get("scope").filter(|s| !s.is_null()).cloned();
        if let Some(scope) = &scope {
            if !scope.is_object() {
                out.push(Refusal::new(
                    json_path(&entry_path),
                    code::BAD_CONFIG,
                    "`scope` must be an object",
                ));
            }
        }
        let secret = if client {
            // A client credential holds nothing; the caller supplies it.
            SecretRef::Env(String::new())
        } else {
            match body.get("secret") {
                Some(secret) => {
                    match SecretRef::parse(secret, &json_path(&entry_path), out) {
                        Some(secret) => {
                            if !secret.supported() {
                                out.push(Refusal::new(
                                    json_path(&entry_path),
                                    code::NOT_IN_THIS_SLICE,
                                    format!("secret shape `{}` is not in 0.0.20", secret.shape()),
                                ));
                            }
                            secret
                        }
                        None => continue,
                    }
                }
                None => {
                    out.push(Refusal::new(
                        json_path(&entry_path),
                        code::BAD_CONFIG,
                        "`secret` is required (or set `client: true`)",
                    ));
                    continue;
                }
            }
        };
        let allowed_hosts = string_list(body, &entry_path, "allowedHosts", out);
        credentials.push((
            id.clone(),
            Credential {
                id: id.clone(),
                endpoint,
                label,
                secret,
                scope,
                client,
                allowed_hosts,
            },
        ));
    }
    credentials
}

fn parse_egress(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Egress)> {
    let path = ["egress"];
    let mut egress = Vec::new();
    let Some(value) = value else {
        return egress;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return egress;
    };
    for (id, body) in parsed {
        let entry_path = ["egress", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(body_object, &entry_path, &["proxy", "noProxy", "timeouts"], out);
        let proxy = match body.get("proxy") {
            Some(Value::String(text)) if text == "direct" => Proxy::Direct,
            Some(Value::String(text)) if text == "env" => Proxy::Env,
            Some(Value::String(text)) => {
                out.push(Refusal::new(
                    json_path(&entry_path),
                    code::BAD_CONFIG,
                    format!("`proxy` = `{text}` is not `direct`, `env` or an object"),
                ));
                Proxy::Direct
            }
            Some(Value::Object(_)) => {
                let url = match body["proxy"].get("url").and_then(Value::as_str) {
                    Some(url) => url.to_owned(),
                    None => {
                        out.push(Refusal::new(
                            json_path(&entry_path),
                            code::BAD_CONFIG,
                            "`proxy.url` must be a string",
                        ));
                        String::new()
                    }
                };
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    out.push(Refusal::new(
                        json_path(&entry_path),
                        code::BAD_CONFIG,
                        "`proxy.url` must start with http:// or https://",
                    ));
                }
                // noProxy is mandatory and defaults to the loopback list; the
                // client-to-gate hop must never be proxied.
                let mut no_proxy = string_list(body, &entry_path, "noProxy", out);
                if no_proxy.is_empty() {
                    no_proxy = vec![
                        String::from("localhost"),
                        String::from("127.0.0.1"),
                        String::from("::1"),
                    ];
                }
                Proxy::Url { url, no_proxy }
            }
            _ => {
                out.push(Refusal::new(
                    json_path(&entry_path),
                    code::BAD_CONFIG,
                    "`proxy` is required (`direct`, `env`, or `{url, noProxy}`)",
                ));
                Proxy::Direct
            }
        };
        let timeouts = match body.get("timeouts") {
            Some(timeout) => Some(timeouts_of(
                timeout,
                &entry_path,
                Timeouts {
                    header_ms: 300_000,
                    chunk_ms: 900_000,
                },
                out,
            )),
            None => None,
        };
        egress.push((
            id.clone(),
            Egress {
                id: id.clone(),
                proxy,
                timeouts,
            },
        ));
    }
    egress
}

fn parse_limits(value: &Value, path: &[&str], out: &mut Vec<Refusal>) -> Limits {
    let mut limits = Limits::default();
    let Some(body) = value.get("limits") else {
        return limits;
    };
    let Some(limits_map) = body.as_object() else {
        out.push(Refusal::new(
            json_path(path),
            code::BAD_CONFIG,
            "`limits` must be an object",
        ));
        return limits;
    };
    check_fields(limits_map, path, &["concurrency"], out);
    if let Some(concurrency) = limits_map.get("concurrency") {
        match concurrency.as_u64() {
            Some(n) if (1..=16).contains(&n) => limits.concurrency = n as u32,
            Some(n) => out.push(Refusal::new(
                json_path(path),
                code::OVER_BOUND,
                format!("`limits.concurrency` = {n} is outside 1..=16"),
            )),
            None => out.push(Refusal::new(
                json_path(path),
                code::BAD_CONFIG,
                "`limits.concurrency` must be an integer",
            )),
        }
    }
    limits
}

fn parse_lanes(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Lane)> {
    let path = ["lanes"];
    let mut lanes = Vec::new();
    let Some(value) = value else {
        return lanes;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return lanes;
    };
    for (id, body) in parsed {
        let entry_path = ["lanes", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(
            body_object,
            &entry_path,
            &["endpoint", "model", "credentialOrder", "egressOrder", "modelArgs", "limits", "tags", "disabled"],
            out,
        );
        let endpoint = string(body, &entry_path, "endpoint", out).unwrap_or_default();
        let model = string(body, &entry_path, "model", out).unwrap_or_default();
        let credential_order = string_list(body, &entry_path, "credentialOrder", out);
        let egress_order = string_list(body, &entry_path, "egressOrder", out);
        let model_args = model_args_of(body, &entry_path, "modelArgs", out);
        let limits = parse_limits(body, &entry_path, out);
        lanes.push((
            id.clone(),
            Lane {
                id: id.clone(),
                endpoint,
                model,
                credential_order,
                egress_order,
                model_args,
                limits,
            },
        ));
    }
    lanes
}

fn parse_agents(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Agent)> {
    let path = ["agents"];
    let mut agents = Vec::new();
    let Some(value) = value else {
        return agents;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return agents;
    };
    for (id, body) in parsed {
        let entry_path = ["agents", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(body_object, &entry_path, &["lanes", "select", "modelArgs", "budget"], out);
        let lanes = string_list(body, &entry_path, "lanes", out);
        let select = optional_string(body, "select").unwrap_or_else(|| String::from("ordered"));
        if select != "ordered" {
            out.push(Refusal::new(
                json_path(&entry_path),
                code::NOT_IN_THIS_SLICE,
                format!("select `{select}` is not `ordered` in 0.0.20"),
            ));
        }
        let model_args = model_args_of(body, &entry_path, "modelArgs", out);
        let mut budget = Budget {
            max_attempts: 3,
            max_output_tokens: None,
        };
        if let Some(value) = body.get("budget") {
            let Some(budget_map) = value.as_object() else {
                out.push(Refusal::new(
                    json_path(&entry_path),
                    code::BAD_CONFIG,
                    "`budget` must be an object",
                ));
                agents.push((
                    id.clone(),
                    Agent {
                        id: id.clone(),
                        lanes,
                        select,
                        model_args,
                        budget,
                    },
                ));
                continue;
            };
            check_fields(budget_map, &entry_path, &["maxAttempts", "maxOutputTokens"], out);
            if let Some(attempts) = budget_map.get("maxAttempts") {
                match attempts.as_u64() {
                    Some(n) if (1..=4).contains(&n) => budget.max_attempts = n as u32,
                    Some(n) => out.push(Refusal::new(
                        json_path(&entry_path),
                        code::OVER_BOUND,
                        format!("`budget.maxAttempts` = {n} is outside 1..=4"),
                    )),
                    None => out.push(Refusal::new(
                        json_path(&entry_path),
                        code::BAD_CONFIG,
                        "`budget.maxAttempts` must be an integer",
                    )),
                }
            }
            if let Some(tokens) = budget_map.get("maxOutputTokens") {
                match tokens.as_u64() {
                    Some(n) => budget.max_output_tokens = Some(n),
                    None => out.push(Refusal::new(
                        json_path(&entry_path),
                        code::BAD_CONFIG,
                        "`budget.maxOutputTokens` must be an integer",
                    )),
                }
            }
        }
        agents.push((
            id.clone(),
            Agent {
                id: id.clone(),
                lanes,
                select,
                model_args,
                budget,
            },
        ));
    }
    agents
}

fn parse_workflows(value: Option<&Value>, out: &mut Vec<Refusal>) -> Vec<(String, Workflow)> {
    let path = ["workflows"];
    let mut workflows = Vec::new();
    let Some(value) = value else {
        return workflows;
    };
    let Some(parsed) = as_object(value, &path, out) else {
        return workflows;
    };
    for (id, body) in parsed {
        let entry_path = ["workflows", id.as_str()];
        let Some(body_object) = as_object(body, &entry_path, out) else {
            continue;
        };
        check_fields(body_object, &entry_path, &["entry", "nodes", "edges"], out);
        let entry = string(body, &entry_path, "entry", out).unwrap_or_default();
        let mut nodes = Vec::new();
        if let Some(node_map) = body.get("nodes").and_then(Value::as_object) {
            if node_map.len() > MAX_WORKFLOW_NODES {
                out.push(Refusal::new(
                    json_path(&entry_path),
                    code::OVER_BOUND,
                    format!("workflow has {} nodes, over the {MAX_WORKFLOW_NODES} bound", node_map.len()),
                ));
            }
            for (node_id, node_body) in node_map {
                let node_path = ["workflows", id.as_str(), node_id.as_str()];
                let Some(node_object) = as_object(node_body, &node_path, out) else {
                    continue;
                };
                check_fields(node_object, &node_path, &["agent", "egress", "modelArgs"], out);
                let agent = string(node_body, &node_path, "agent", out).unwrap_or_default();
                let egress = optional_string(node_body, "egress");
                let model_args = model_args_of(node_body, &node_path, "modelArgs", out);
                nodes.push((
                    node_id.clone(),
                    Node {
                        agent,
                        egress,
                        model_args,
                    },
                ));
            }
        } else {
            out.push(Refusal::new(
                json_path(&entry_path),
                code::BAD_CONFIG,
                "`nodes` must be an object",
            ));
        }
        let mut edges = Vec::new();
        match body.get("edges") {
            None => {}
            Some(Value::Array(items)) => {
                for item in items {
                    let Some(edge_path) = as_object(item, &entry_path, out) else {
                        continue;
                    };
                    check_fields(edge_path, &entry_path, &["from", "to", "kind"], out);
                    let from = item.get("from").and_then(Value::as_str).unwrap_or_default();
                    let to = item.get("to").and_then(Value::as_str).unwrap_or_default();
                    edges.push(Edge {
                        from: from.to_owned(),
                        to: to.to_owned(),
                    });
                }
            }
            Some(_) => out.push(Refusal::new(
                json_path(&entry_path),
                code::BAD_CONFIG,
                "`edges` must be an array",
            )),
        }
        workflows.push((
            id.clone(),
            Workflow {
                id: id.clone(),
                entry,
                nodes,
                edges,
            },
        ));
    }
    workflows
}
