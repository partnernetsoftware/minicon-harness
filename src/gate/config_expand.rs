// Expansion half of the definition plane: the pure resolution function.
// Included by `config.rs`.

/// The expansion for one lane in a fixed credential order and egress order:
/// credential outer, egress inner, in declared order (the Kronecker product),
/// deduplicated by `(endpoint, credential, egress)`.
pub fn expand_lane(lane: &Lane) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for credential in &lane.credential_order {
        for egress in &lane.egress_order {
            let pair = (credential.clone(), egress.clone());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
    }
    out
}

/// Deep-merges `over` onto `base`: objects merge key by key, everything else
/// (arrays included) is replaced wholesale.
pub fn merge_model_args(base: &Value, over: &Value) -> Value {
    match (base, over) {
        (Value::Object(base_map), Value::Object(over_map)) => {
            let mut merged = base_map.clone();
            for (key, value) in over_map {
                let next = match merged.get(key) {
                    Some(existing) => merge_model_args(existing, value),
                    None => value.clone(),
                };
                merged.insert(key.clone(), next);
            }
            Value::Object(merged)
        }
        (_, over) => over.clone(),
    }
}

/// Resolves one agent to its ordered candidates.
///
/// Precedence, lowest to highest: gate defaults, the credential's `scope`, the
/// lane's `modelArgs`, the agent's `modelArgs`. Workflow-node overrides are a
/// further layer applied by [`resolve_node`], and request overrides never reach
/// these operator-owned fields.
pub fn expand_agent(
    agent: &Agent,
    lanes: &[(String, Lane)],
    _defaults: &Timeouts,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for lane_id in &agent.lanes {
        let Some((_, lane)) = lanes.iter().find(|(id, _)| id == lane_id) else {
            continue;
        };
        for (credential, egress) in expand_lane(lane) {
            candidates.push(Candidate {
                lane: lane.id.clone(),
                endpoint: lane.endpoint.clone(),
                credential,
                egress,
                model: lane.model.clone(),
                model_args: merge_model_args(&lane.model_args, &agent.model_args),
            });
        }
    }
    candidates
}

/// Resolves one workflow node: the agent's candidates with the node's own
/// `egress` and `modelArgs` applied.
///
/// A node `egress` **replaces** the lane's egress order (the G8 decision): the
/// same arrays-replace rule as every other precedence layer, never a hidden
/// prepend.
pub fn resolve_node(
    node: &Node,
    agent: &Agent,
    lanes: &[(String, Lane)],
    defaults: &Timeouts,
) -> Vec<Candidate> {
    let candidates = expand_agent(agent, lanes, defaults);
    match &node.egress {
        None => candidates
            .into_iter()
            .map(|mut candidate| {
                candidate.model_args = merge_model_args(&candidate.model_args, &node.model_args);
                candidate
            })
            .collect(),
        Some(egress) => candidates
            .into_iter()
            .filter(|candidate| &candidate.egress == egress)
            .map(|mut candidate| {
                candidate.model_args = merge_model_args(&candidate.model_args, &node.model_args);
                candidate
            })
            .collect(),
    }
}

/// Applies a credential's `scope` to a candidate's model arguments: the lowest
/// layer above the gate defaults.
pub fn apply_credential_scope(candidate: &mut Candidate, credential: &Credential) {
    if let Some(scope) = &credential.scope {
        candidate.model_args = merge_model_args(scope, &candidate.model_args);
    }
}
