use schemars::schema_for;

fn has_top_level_combinator(val: &serde_json::Value) -> bool {
    let Some(obj) = val.as_object() else {
        return false;
    };
    obj.contains_key("oneOf") || obj.contains_key("anyOf") || obj.contains_key("allOf")
}

#[test]
fn mcp_tool_inputs_have_no_top_level_combinator() {
    let pairs = [
        (
            "WriteArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::WriteArgs)).unwrap(),
        ),
        (
            "GetArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::GetArgs)).unwrap(),
        ),
        (
            "UpdateArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::UpdateArgs)).unwrap(),
        ),
        (
            "DeprecateArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::DeprecateArgs)).unwrap(),
        ),
        (
            "SearchArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::SearchArgs)).unwrap(),
        ),
        (
            "RecentArgs",
            serde_json::to_value(schema_for!(kb_server::mcp::RecentArgs)).unwrap(),
        ),
    ];
    let mut bad = Vec::new();
    for (name, val) in &pairs {
        if has_top_level_combinator(val) {
            bad.push(*name);
        }
    }
    assert!(
        bad.is_empty(),
        "Anthropic tool API forbids top-level oneOf/anyOf/allOf in input_schema; offenders: {bad:?}"
    );
}
