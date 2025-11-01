#[cfg(test)]
mod tests {
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
    async fn login_schema_handle_is_optional() {
        // Verify that handle is not in required fields
        let tools = crate::mcp::build_tools_array();
        let tools_arr = tools.as_array().expect("tools array");
        let login = tools_arr
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("login"))
            .expect("login tool present");
        let schema = login.get("inputSchema").expect("login schema");

        // Check required fields
        let required = schema.get("required").and_then(|r| r.as_array());

        // Handle should NOT be in required fields (it's optional for OAuth)
        if let Some(req_fields) = required {
            let has_handle = req_fields.iter().any(|f| f.as_str() == Some("handle"));
            assert!(
                !has_handle,
                "handle should be optional in login schema, not required"
            );
        }

        // But handle should exist in properties
        let properties = schema.get("properties").expect("properties object");
        assert!(
            properties.get("handle").is_some(),
            "handle should exist as an optional property"
        );
    }

    #[tokio::test]
    async fn login_description_mentions_optional_handle() {
        // Verify the tool description mentions that handle is optional
        let tools = crate::mcp::build_tools_array();
        let tools_arr = tools.as_array().expect("tools array");
        let login = tools_arr
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("login"))
            .expect("login tool present");
        let description = login
            .get("description")
            .and_then(|d| d.as_str())
            .expect("login description");

        assert!(
            description.to_lowercase().contains("optional")
                || description.to_lowercase().contains("selection"),
            "Login description should mention optional handle or account selection"
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
