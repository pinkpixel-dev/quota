use quota_cli::herdr::{parse_agent_list, provider_for_agent_kind, TokenCache};

#[test]
fn parses_the_agent_list_envelope() {
    let raw = r#"{"id":"cli:agent:list","result":{"agents":[
        {"agent":"claude","pane_id":"w4:p1","workspace_id":"w4","agent_status":"working"},
        {"agent":"codex","pane_id":"w4:p2","workspace_id":"w4","agent_status":"idle"}
    ],"type":"agent_list"}}"#;

    let panes = parse_agent_list(raw).expect("parse");
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].agent, "claude");
    assert_eq!(panes[0].pane_id, "w4:p1");
    assert_eq!(panes[0].workspace_id, "w4");
}

#[test]
fn an_empty_agent_list_is_not_an_error() {
    let raw = r#"{"id":"cli:agent:list","result":{"agents":[],"type":"agent_list"}}"#;
    assert!(parse_agent_list(raw).expect("parse").is_empty());
}

#[test]
fn unknown_fields_in_the_envelope_are_tolerated() {
    let raw = r#"{"id":"x","result":{"agents":[
        {"agent":"claude","pane_id":"w1:p1","workspace_id":"w1","something_new":42}
    ],"type":"agent_list","future_field":true}}"#;

    let panes = parse_agent_list(raw).expect("parse");
    assert_eq!(panes[0].pane_id, "w1:p1");
}

#[test]
fn maps_the_claude_agent_kind_to_the_claude_provider() {
    assert_eq!(provider_for_agent_kind("claude"), Some("claude"));
}

#[test]
fn unmapped_agent_kinds_report_no_provider() {
    assert_eq!(provider_for_agent_kind("cursor"), None);
    assert_eq!(provider_for_agent_kind("nonsense"), None);
}

#[test]
fn a_cache_inside_the_ttl_is_fresh() {
    let cache = TokenCache {
        fetched_at_ms: 1_000_000,
        token: "5h 89% · Wk 57%".to_string(),
    };
    assert!(!cache.is_stale_at_ms(1_000_000 + 119_000, 120_000));
}

#[test]
fn a_cache_past_the_ttl_is_stale() {
    let cache = TokenCache {
        fetched_at_ms: 1_000_000,
        token: "5h 89% · Wk 57%".to_string(),
    };
    assert!(cache.is_stale_at_ms(1_000_000 + 121_000, 120_000));
}

#[test]
fn a_cache_from_the_future_is_treated_as_stale() {
    let cache = TokenCache {
        fetched_at_ms: 5_000_000,
        token: "stale".to_string(),
    };
    assert!(cache.is_stale_at_ms(1_000_000, 120_000));
}
