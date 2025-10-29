#[cfg(test)]
mod tests {
    use serde_json::json;

    #[tokio::test]
    async fn tools_list_schema_excludes_prompt_id() {
        // Build tools array using the same function as server
        let tools = crate::mcp::build_tools_array();
        let tools_arr = tools.as_array().expect("tools array");
        let login = tools_arr
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("login"))
            .expect("login tool present");
        let schema = login.get("inputSchema").expect("login schema");
        let schema_str = serde_json::to_string(schema).unwrap();
        assert!(
            !schema_str.contains("prompt_id"),
            "prompt_id should be excluded from MCP-facing login schema"
        );
    }

    #[tokio::test]
    async fn login_fallback_sets_is_error_flag() {
        // Create a minimal ServerContext without elicitation support
        let context = crate::mcp::ServerContext::new(None);
        // Call helper to build fallback
        let tr = crate::tools::login::create_elicitation_unavailable_error(&context, "handle");
        let json = serde_json::to_value(&tr).unwrap();
        assert_eq!(json.get("isError").and_then(|v| v.as_bool()), Some(true));
    }
}
