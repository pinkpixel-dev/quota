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
fn maps_every_agent_kind_that_has_a_provider() {
    assert_eq!(provider_for_agent_kind("claude"), Some("claude"));
    assert_eq!(provider_for_agent_kind("codex"), Some("codex"));
    assert_eq!(provider_for_agent_kind("cursor"), Some("cursor"));
    assert_eq!(provider_for_agent_kind("grok"), Some("grok"));
    assert_eq!(provider_for_agent_kind("agy"), Some("antigravity"));
    assert_eq!(provider_for_agent_kind("kiro"), Some("kiro"));
}

#[test]
fn maps_the_aliases_herdr_also_uses_for_the_same_agent() {
    // Herdr names some agents more than one way. An unmatched kind is skipped
    // silently, which on screen is indistinguishable from a broken provider.
    for kind in ["claude", "claude-code", "anthropic", "  Claude  ", "CLAUDE"] {
        assert_eq!(provider_for_agent_kind(kind), Some("claude"), "kind {}", kind);
    }
    for kind in ["agy", "antigravity", "antigravity-cli"] {
        assert_eq!(
            provider_for_agent_kind(kind),
            Some("antigravity"),
            "kind {}",
            kind
        );
    }
}

#[test]
fn gemini_is_not_treated_as_antigravity() {
    // The Gemini Code Assist CLI is a different OAuth client from Antigravity's,
    // and the Antigravity provider only handles its own. Mapping it here would
    // report one account's usage on another account's pane.
    assert_eq!(provider_for_agent_kind("gemini"), None);
}

#[test]
fn unmapped_agent_kinds_report_no_provider() {
    assert_eq!(provider_for_agent_kind("opencode"), None);
    assert_eq!(provider_for_agent_kind("nonsense"), None);
}

#[test]
fn reads_the_agent_kind_from_the_session_when_the_pane_omits_it() {
    let raw = r#"{"result":{"agents":[
        {"pane_id":"w1:p1","workspace_id":"w1","agent_session":{"agent":"codex"}}
    ]}}"#;

    let panes = parse_agent_list(raw).expect("parse");
    assert_eq!(panes[0].agent, "codex");
}

#[test]
fn one_unusable_pane_does_not_discard_the_rest() {
    // A pane described in a shape we do not expect should cost that one pane
    // its number. Failing the whole parse blanks every pane in the sidebar.
    let raw = r#"{"result":{"agents":[
        {"agent":"claude","pane_id":"w1:p1","workspace_id":"w1"},
        {"agent":"codex"},
        {"pane_id":"w1:p3","workspace_id":"w1"},
        {"agent":"codex","pane_id":"w1:p4","workspace_id":"w1"}
    ]}}"#;

    let panes = parse_agent_list(raw).expect("parse");

    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].pane_id, "w1:p1");
    assert_eq!(panes[1].pane_id, "w1:p4");
}

#[test]
fn a_missing_agents_array_is_an_error_rather_than_an_empty_list() {
    // Silently reporting nothing would look identical to "no agents running".
    assert!(parse_agent_list(r#"{"result":{}}"#).is_err());
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
