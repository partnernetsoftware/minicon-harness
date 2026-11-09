// Tests for the definition plane. Included by `config.rs`.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE: &str = include_str!("../../tests/fixtures/gate-fixture.jsonc");

    /// The plan's fixture, verbatim. It already names `allowedHosts` for the
    /// `client` credential, so a full load must succeed unchanged.
    fn fixture() -> GateConfig {
        parse(FIXTURE, false).expect("the plan's fixture should load")
    }

    fn codes(refusals: &[Refusal]) -> Vec<&'static str> {
        refusals.iter().map(|r| r.code).collect()
    }

    fn minimal(text: &str) -> Result<GateConfig, Vec<Refusal>> {
        parse(text, false)
    }

    // --- the plan section 4.8 fixture ------------------------------------

    #[test]
    fn fixture_loads_with_two_endpoints_two_credentials_two_egress() {
        let config = fixture();
        assert_eq!(config.endpoints.len(), 2);
        assert_eq!(config.credentials.len(), 4);
        assert_eq!(config.egress.len(), 2);
        assert_eq!(config.lanes.len(), 2);
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.workflows.len(), 1);
    }

    #[test]
    fn fixture_expands_oa_sol_to_four_ordered_candidates() {
        let config = fixture();
        let planner = config
            .agents
            .iter()
            .find(|(id, _)| id == "planner")
            .map(|(_, agent)| agent)
            .expect("planner");
        let candidates = expand_agent(planner, &config.lanes, &config.defaults.timeouts);
        // oa-sol gives 2x2, ds-fast gives 1x1: five candidates, credential outer.
        assert_eq!(candidates.len(), 5);
        let pairs: Vec<(String, String)> = candidates
            .iter()
            .map(|c| (c.credential.clone(), c.egress.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                (String::from("oa-personal"), String::from("proxy8891")),
                (String::from("oa-personal"), String::from("direct")),
                (String::from("oa-work"), String::from("proxy8891")),
                (String::from("oa-work"), String::from("direct")),
                (String::from("ds-work"), String::from("direct")),
            ]
        );
    }

    #[test]
    fn agent_model_args_override_the_lane() {
        let config = fixture();
        let planner = config
            .agents
            .iter()
            .find(|(id, _)| id == "planner")
            .map(|(_, agent)| agent)
            .expect("planner");
        let candidates = expand_agent(planner, &config.lanes, &config.defaults.timeouts);
        let first = &candidates[0];
        // lane says "high", agent says "medium": agent wins.
        assert_eq!(first.model_args["reasoningEffort"], json!("medium"));
    }

    #[test]
    fn client_credential_without_allowed_hosts_is_refused() {
        let stripped = FIXTURE.replace(
            ",\n                     \"allowedHosts\": [\"relay.internal.example\"]",
            "",
        );
        assert_ne!(stripped, FIXTURE, "the allowedHosts line moved; update this test");
        let refusals = parse(&stripped, false).expect_err("must refuse");
        assert!(codes(&refusals).contains(&code::CLIENT_HOSTS_MISSING));
    }    // --- expansion -------------------------------------------------------

    #[test]
    fn expansion_is_credential_outer_egress_inner() {
        let lane = Lane {
            id: String::from("l"),
            endpoint: String::from("e"),
            model: String::from("m"),
            credential_order: vec![String::from("c1"), String::from("c2")],
            egress_order: vec![String::from("x1"), String::from("x2")],
            model_args: json!({}),
            limits: Limits::default(),
        };
        assert_eq!(
            expand_lane(&lane),
            vec![
                (String::from("c1"), String::from("x1")),
                (String::from("c1"), String::from("x2")),
                (String::from("c2"), String::from("x1")),
                (String::from("c2"), String::from("x2")),
            ]
        );
    }

    #[test]
    fn expansion_deduplicates_a_repeated_axis_entry() {
        let lane = Lane {
            id: String::from("l"),
            endpoint: String::from("e"),
            model: String::from("m"),
            credential_order: vec![String::from("c1"), String::from("c1")],
            egress_order: vec![String::from("x1")],
            model_args: json!({}),
            limits: Limits::default(),
        };
        assert_eq!(expand_lane(&lane).len(), 1);
    }

    #[test]
    fn merge_is_deep_for_objects_and_replace_for_arrays() {
        let base = json!({ "temperature": 0.2, "extra": { "a": 1, "b": 2 }, "stop": [1, 2] });
        let over = json!({ "extra": { "b": 3 }, "stop": [9] });
        let merged = merge_model_args(&base, &over);
        assert_eq!(merged["temperature"], json!(0.2));
        assert_eq!(merged["extra"], json!({ "a": 1, "b": 3 }));
        assert_eq!(merged["stop"], json!([9]));
    }

    #[test]
    fn node_egress_replaces_the_lane_order() {
        let lane = Lane {
            id: String::from("l"),
            endpoint: String::from("e"),
            model: String::from("m"),
            credential_order: vec![String::from("c1")],
            egress_order: vec![String::from("x1"), String::from("x2")],
            model_args: json!({}),
            limits: Limits::default(),
        };
        let lanes = vec![(String::from("l"), lane)];
        let agent = Agent {
            id: String::from("a"),
            lanes: vec![String::from("l")],
            select: String::from("ordered"),
            model_args: json!({}),
            budget: Budget { max_attempts: 3, max_output_tokens: None },
        };
        let node = Node {
            agent: String::from("a"),
            egress: Some(String::from("x2")),
            model_args: json!({ "reasoningEffort": "low" }),
        };
        let timeouts = Timeouts { header_ms: 1, chunk_ms: 1 };
        let resolved = resolve_node(&node, &agent, &lanes, &timeouts);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].egress, "x2");
        assert_eq!(resolved[0].model_args["reasoningEffort"], json!("low"));
    }

    // --- refusals --------------------------------------------------------

    const MINIMAL: &str = r#"{
      "endpoints": { "oa": { "provider": "openai", "protocol": "openai-chat", "baseURL": "https://api.openai.com/v1" } },
      "credentials": { "c": { "label": "c", "secret": { "env": "KEY" } } },
      "egress": { "direct": { "proxy": "direct" } },
      "lanes": { "l": { "endpoint": "oa", "model": "m", "credentialOrder": ["c"], "egressOrder": ["direct"] } },
      "agents": { "a": { "lanes": ["l"] } }
    }"#;

    #[test]
    fn minimal_config_loads() {
        assert!(minimal(MINIMAL).is_ok());
    }

    #[test]
    fn unknown_lane_egress_is_named() {
        let text = MINIMAL.replace("\"egressOrder\": [\"direct\"]", "\"egressOrder\": [\"nope\"]");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::UNKNOWN_EGRESS]);
    }

    #[test]
    fn unknown_lane_credential_is_named() {
        let text = MINIMAL.replace("\"credentialOrder\": [\"c\"]", "\"credentialOrder\": [\"nope\"]");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::UNKNOWN_CREDENTIAL]);
    }

    #[test]
    fn unknown_lane_endpoint_is_named() {
        let text = MINIMAL.replace("\"endpoint\": \"oa\", \"model\"", "\"endpoint\": \"nope\", \"model\"");
        let refusals = minimal(&text).expect_err("must refuse");
        assert!(codes(&refusals).contains(&code::UNKNOWN_ENDPOINT));
    }

    #[test]
    fn credential_bound_to_another_endpoint_is_refused() {
        let text = MINIMAL.replace(
            "\"label\": \"c\",",
            "\"label\": \"c\", \"endpoint\": \"other\",",
        );
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::CREDENTIAL_ENDPOINT_MISMATCH]);
    }

    #[test]
    fn oauth_file_secret_is_named_not_in_this_slice() {
        let text = MINIMAL.replace("\"secret\": { \"env\": \"KEY\" }", "\"secret\": { \"oauthFile\": \"t.json\" }");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::NOT_IN_THIS_SLICE]);
    }

    #[test]
    fn unknown_field_is_refused() {
        let text = MINIMAL.replace("\"provider\": \"openai\",", "\"provider\": \"openai\", \"typo\": 1,");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::BAD_CONFIG]);
        assert!(refusals[0].reason.contains("typo"));
    }

    #[test]
    fn non_loopback_bind_is_refused_without_the_flag() {
        let text = MINIMAL.replace("{\n      \"endpoints\"", "{\n      \"listen\": { \"bind\": \"0.0.0.0:8787\" },\n      \"endpoints\"");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::NON_LOOPBACK_BIND]);
        assert!(parse(&text, true).is_ok(), "the flag accepts it");
    }

    #[test]
    fn protocol_other_than_openai_chat_is_refused() {
        let text = MINIMAL.replace("\"openai-chat\", \"baseURL\"", "\"anthropic-messages\", \"baseURL\"");
        let refusals = minimal(&text).expect_err("must refuse");
        assert_eq!(codes(&refusals), vec![code::PROTOCOL_UNSUPPORTED]);
    }

    #[test]
    fn empty_no_proxy_defaults_to_loopback_list() {
        let text = MINIMAL.replace(
            "\"direct\": { \"proxy\": \"direct\" }",
            "\"direct\": { \"proxy\": { \"url\": \"http://127.0.0.1:8891\" } }",
        );
        let config = minimal(&text).expect("loads");
        let Proxy::Url { no_proxy, .. } = &config.egress[0].1.proxy else {
            panic!("expected a URL proxy");
        };
        assert_eq!(no_proxy, &vec![String::from("localhost"), String::from("127.0.0.1"), String::from("::1")]);
    }

    #[test]
    fn cyclic_workflow_is_refused() {
        let text = MINIMAL.replace(
            "\"agents\": { \"a\": { \"lanes\": [\"l\"] } }",
            "\"agents\": { \"a\": { \"lanes\": [\"l\"] } },
             \"workflows\": { \"w\": { \"entry\": \"x\", \"nodes\": { \"x\": { \"agent\": \"a\" }, \"y\": { \"agent\": \"a\" } },
               \"edges\": [ { \"from\": \"x\", \"to\": \"y\" }, { \"from\": \"y\", \"to\": \"x\" } ] } }",
        );
        let refusals = minimal(&text).expect_err("must refuse");
        assert!(
            refusals.iter().any(|r| r.reason.contains("cycle")),
            "expected a cycle refusal, got {refusals:?}"
        );
    }

    #[test]
    fn acyclic_workflow_is_accepted() {
        let text = MINIMAL.replace(
            "\"agents\": { \"a\": { \"lanes\": [\"l\"] } }",
            "\"agents\": { \"a\": { \"lanes\": [\"l\"] } },
             \"workflows\": { \"w\": { \"entry\": \"x\", \"nodes\": { \"x\": { \"agent\": \"a\" }, \"y\": { \"agent\": \"a\" } },
               \"edges\": [ { \"from\": \"x\", \"to\": \"y\" } ] } }",
        );
        let config = minimal(&text).expect("a diamond-free DAG loads");
        assert_eq!(config.workflows.len(), 1);
    }

    #[test]
    fn agent_expansion_over_bound_is_refused() {
        // 6 credentials x 6 egress = 36 candidates for one agent.
        let mut text = String::from(MINIMAL);
        let credentials: Vec<String> = (0..6)
            .map(|i| format!("\"c{i}\": {{ \"label\": \"c{i}\", \"secret\": {{ \"env\": \"K{i}\" }} }}"))
            .collect();
        let egress: Vec<String> = (0..6)
            .map(|i| format!("\"x{i}\": {{ \"proxy\": \"direct\" }}"))
            .collect();
        text = text.replace(
            "\"c\": { \"label\": \"c\", \"secret\": { \"env\": \"KEY\" } }",
            &credentials.join(", "),
        );
        text = text.replace(
            "\"direct\": { \"proxy\": \"direct\" }",
            &egress.join(", "),
        );
        text = text.replace(
            "\"credentialOrder\": [\"c\"], \"egressOrder\": [\"direct\"]",
            &format!(
                "\"credentialOrder\": [{}], \"egressOrder\": [{}]",
                (0..6).map(|i| format!("\"c{i}\"")).collect::<Vec<_>>().join(", "),
                (0..6).map(|i| format!("\"x{i}\"")).collect::<Vec<_>>().join(", "),
            ),
        );
        let refusals = minimal(&text).expect_err("must refuse");
        assert!(codes(&refusals).contains(&code::EXPANSION_OVER_BOUND));
    }

    // --- revision --------------------------------------------------------

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"The quick brown fox jumps over the lazy dog"),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn revision_normalises_crlf() {
        assert_eq!(revision_of("a\r\nb"), revision_of("a\nb"));
        assert_ne!(revision_of("a\nb"), revision_of("ab"));
    }
}
