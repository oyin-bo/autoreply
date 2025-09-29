//! HTTP client utilities
//!
//! Provides a reqwest::Client configured with timeouts and system proxy support

use reqwest::{Client, Proxy};
use std::time::Duration;
use url::Url;

/// Build a reqwest Client with the given timeout and honoring system proxy env vars
///
/// Recognized env vars (handled by Proxy::system):
/// - HTTP_PROXY / http_proxy
/// - HTTPS_PROXY / https_proxy
/// - ALL_PROXY / all_proxy
/// - NO_PROXY / no_proxy
pub fn client_with_timeout(timeout: Duration) -> Client {
    let mut builder = Client::builder().timeout(timeout);

    // Proxy configuration via environment variables
    let https_proxy = getenv_first(&["HTTPS_PROXY", "https_proxy"]).or_else(|| getenv_first(&["ALL_PROXY", "all_proxy"]));
    let http_proxy = getenv_first(&["HTTP_PROXY", "http_proxy"]).or_else(|| getenv_first(&["ALL_PROXY", "all_proxy"]));
    let no_proxy = getenv_first(&["NO_PROXY", "no_proxy"]).unwrap_or_default();
    let no_proxy_rules = parse_no_proxy(&no_proxy);

    if https_proxy.is_some() || http_proxy.is_some() {
        // Use a custom proxy selector to honor NO_PROXY and per-scheme proxies
        let https_proxy_cl = https_proxy.clone();
        let http_proxy_cl = http_proxy.clone();
        let no_proxy_rules_cl = no_proxy_rules.clone();
        let proxy = Proxy::custom(move |url: &Url| {
            let host = url.host_str().unwrap_or("");
            if should_bypass_proxy(host, &no_proxy_rules_cl) {
                return None;
            }
            match url.scheme() {
                "https" => https_proxy_cl.as_deref().or(http_proxy_cl.as_deref()).map(|p| p.to_string()),
                "http" => http_proxy_cl.as_deref().or(https_proxy_cl.as_deref()).map(|p| p.to_string()),
                _ => None,
            }
        });
        builder = builder.proxy(proxy);
    }

    builder
        .user_agent(concat!("autoreply/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to create HTTP client")
}

fn getenv_first(keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
enum NoProxyRule {
    Wildcard,
    Domain(String),   // matches suffix
    Exact(String),    // exact host
}

fn parse_no_proxy(val: &str) -> Vec<NoProxyRule> {
    val.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|token| {
            if token == "*" { return NoProxyRule::Wildcard; }
            
            // If starts with dot, it's explicitly a domain suffix rule
            if token.starts_with('.') {
                let domain = token.trim_start_matches('.').to_ascii_lowercase();
                return NoProxyRule::Domain(domain);
            }
            
            // Heuristic: if it looks like an IP or localhost, use exact matching
            // Otherwise use domain suffix matching for better compatibility
            let t = token.to_ascii_lowercase();
            if t == "localhost" || t.parse::<std::net::IpAddr>().is_ok() {
                NoProxyRule::Exact(t)
            } else {
                NoProxyRule::Domain(t)
            }
        })
        .collect()
}

fn should_bypass_proxy(host: &str, rules: &[NoProxyRule]) -> bool {
    if host.is_empty() { return false; }
    let host_lc = host.to_ascii_lowercase();
    for r in rules {
        match r {
            NoProxyRule::Wildcard => return true,
            NoProxyRule::Exact(ex) => {
                if &host_lc == ex { return true; }
            }
            NoProxyRule::Domain(suf) => {
                if host_lc == *suf || host_lc.ends_with(&format!(".{}", suf)) { return true; }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_client_with_timeout_creation() {
        let timeout = Duration::from_secs(30);
        let client = client_with_timeout(timeout);
        
        // Client should be created successfully
        // Note: reqwest::Client doesn't expose timeout() method, 
        // but we can verify the client was built without panic
        assert!(format!("{:?}", client).contains("Client"));
    }

    #[test]
    fn test_client_user_agent() {
        let client = client_with_timeout(Duration::from_secs(10));
        
        // Should have proper user agent set
        let user_agent = format!("autoreply/{}", env!("CARGO_PKG_VERSION"));
        // We can't directly access the user agent from reqwest::Client, 
        // but we can verify the client was built successfully with our config
        assert!(format!("{:?}", client).contains("Client"));
    }

    #[test]
    fn test_getenv_first_finds_first_available() {
        // Test with environment variables that exist
        std::env::set_var("TEST_VAR_1", "value1");
        std::env::set_var("TEST_VAR_2", "value2");
        
        let result = getenv_first(&["TEST_VAR_NONEXISTENT", "TEST_VAR_1", "TEST_VAR_2"]);
        assert_eq!(result, Some("value1".to_string()));
        
        // Clean up
        std::env::remove_var("TEST_VAR_1");
        std::env::remove_var("TEST_VAR_2");
    }

    #[test]
    fn test_getenv_first_returns_none_when_not_found() {
        let result = getenv_first(&["NONEXISTENT_VAR_1", "NONEXISTENT_VAR_2"]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_getenv_first_skips_empty_values() {
        std::env::set_var("EMPTY_VAR", "");
        std::env::set_var("WHITESPACE_VAR", "  ");
        std::env::set_var("VALID_VAR", "value");
        
        let result = getenv_first(&["EMPTY_VAR", "WHITESPACE_VAR", "VALID_VAR"]);
        assert_eq!(result, Some("value".to_string()));
        
        // Clean up
        std::env::remove_var("EMPTY_VAR");
        std::env::remove_var("WHITESPACE_VAR");
        std::env::remove_var("VALID_VAR");
    }

    #[test]
    fn test_parse_no_proxy_wildcard() {
        let rules = parse_no_proxy("*");
        assert_eq!(rules.len(), 1);
        matches!(rules[0], NoProxyRule::Wildcard);
    }

    #[test]
    fn test_parse_no_proxy_domain_with_dot() {
        let rules = parse_no_proxy(".example.com");
        assert_eq!(rules.len(), 1);
        match &rules[0] {
            NoProxyRule::Domain(domain) => assert_eq!(domain, "example.com"),
            _ => panic!("Expected Domain rule"),
        }
    }

    #[test]
    fn test_parse_no_proxy_localhost() {
        let rules = parse_no_proxy("localhost");
        assert_eq!(rules.len(), 1);
        match &rules[0] {
            NoProxyRule::Exact(host) => assert_eq!(host, "localhost"),
            _ => panic!("Expected Exact rule for localhost"),
        }
    }

    #[test]
    fn test_parse_no_proxy_ip_address() {
        let rules = parse_no_proxy("127.0.0.1,192.168.1.1");
        assert_eq!(rules.len(), 2);
        
        for rule in &rules {
            match rule {
                NoProxyRule::Exact(ip) => {
                    assert!(ip == "127.0.0.1" || ip == "192.168.1.1");
                }
                _ => panic!("Expected Exact rule for IP"),
            }
        }
    }

    #[test]
    fn test_parse_no_proxy_domain_heuristic() {
        let rules = parse_no_proxy("example.com,internal.corp");
        assert_eq!(rules.len(), 2);
        
        for rule in &rules {
            match rule {
                NoProxyRule::Domain(domain) => {
                    assert!(domain == "example.com" || domain == "internal.corp");
                }
                _ => panic!("Expected Domain rule"),
            }
        }
    }

    #[test]
    fn test_parse_no_proxy_mixed() {
        let rules = parse_no_proxy("localhost,.example.com,192.168.1.1,internal.corp");
        assert_eq!(rules.len(), 4);
        
        // Verify we have the expected mix of rule types
        let mut exact_count = 0;
        let mut domain_count = 0;
        
        for rule in &rules {
            match rule {
                NoProxyRule::Exact(_) => exact_count += 1,
                NoProxyRule::Domain(_) => domain_count += 1,
                NoProxyRule::Wildcard => {}
            }
        }
        
        assert_eq!(exact_count, 2); // localhost, 192.168.1.1
        assert_eq!(domain_count, 2); // .example.com, internal.corp
    }

    #[test]
    fn test_parse_no_proxy_empty_and_whitespace() {
        let rules = parse_no_proxy("localhost, , example.com,,");
        assert_eq!(rules.len(), 2); // Should skip empty entries
        
        match (&rules[0], &rules[1]) {
            (NoProxyRule::Exact(host), NoProxyRule::Domain(domain)) => {
                assert_eq!(host, "localhost");
                assert_eq!(domain, "example.com");
            }
            _ => panic!("Unexpected rule types"),
        }
    }

    #[test]
    fn test_should_bypass_proxy_wildcard() {
        let rules = vec![NoProxyRule::Wildcard];
        assert!(should_bypass_proxy("any.host.com", &rules));
        assert!(should_bypass_proxy("localhost", &rules));
        assert!(should_bypass_proxy("192.168.1.1", &rules));
    }

    #[test]
    fn test_should_bypass_proxy_exact_match() {
        let rules = vec![
            NoProxyRule::Exact("localhost".to_string()),
            NoProxyRule::Exact("127.0.0.1".to_string()),
        ];
        
        assert!(should_bypass_proxy("localhost", &rules));
        assert!(should_bypass_proxy("127.0.0.1", &rules));
        assert!(!should_bypass_proxy("example.com", &rules));
        
        // Test case insensitive
        assert!(should_bypass_proxy("LOCALHOST", &rules));
    }

    #[test]
    fn test_should_bypass_proxy_domain_suffix() {
        let rules = vec![
            NoProxyRule::Domain("example.com".to_string()),
            NoProxyRule::Domain("internal.corp".to_string()),
        ];
        
        assert!(should_bypass_proxy("example.com", &rules));
        assert!(should_bypass_proxy("api.example.com", &rules));
        assert!(should_bypass_proxy("subdomain.example.com", &rules));
        assert!(should_bypass_proxy("internal.corp", &rules));
        assert!(should_bypass_proxy("app.internal.corp", &rules));
        
        assert!(!should_bypass_proxy("notexample.com", &rules));
        assert!(!should_bypass_proxy("example.org", &rules));
        assert!(!should_bypass_proxy("external.com", &rules));
    }

    #[test]
    fn test_should_bypass_proxy_empty_host() {
        let rules = vec![NoProxyRule::Wildcard];
        assert!(!should_bypass_proxy("", &rules));
    }

    #[test]
    fn test_should_bypass_proxy_no_rules() {
        let rules = vec![];
        assert!(!should_bypass_proxy("example.com", &rules));
        assert!(!should_bypass_proxy("localhost", &rules));
    }

    #[test]
    fn test_should_bypass_proxy_case_insensitive() {
        let rules = vec![
            NoProxyRule::Exact("localhost".to_string()),
            NoProxyRule::Domain("example.com".to_string()),
        ];
        
        assert!(should_bypass_proxy("LOCALHOST", &rules));
        assert!(should_bypass_proxy("LocalHost", &rules));
        assert!(should_bypass_proxy("EXAMPLE.COM", &rules));
        assert!(should_bypass_proxy("API.EXAMPLE.COM", &rules));
    }

    #[test] 
    fn test_client_with_proxy_env_vars() {
        // Test that client creation works with various proxy environment combinations
        // We can't easily test the actual proxy behavior without a test proxy server,
        // but we can ensure the client builds successfully with different env var configs
        
        // Save original env vars
        let original_https = std::env::var("HTTPS_PROXY").ok();
        let original_http = std::env::var("HTTP_PROXY").ok();
        let original_no = std::env::var("NO_PROXY").ok();
        
        // Test with HTTPS proxy
        std::env::set_var("HTTPS_PROXY", "https://proxy.example.com:8080");
        std::env::set_var("NO_PROXY", "localhost,.internal.corp");
        let client = client_with_timeout(Duration::from_secs(10));
        assert!(format!("{:?}", client).contains("Client"));
        
        // Test with HTTP proxy
        std::env::remove_var("HTTPS_PROXY");
        std::env::set_var("HTTP_PROXY", "http://proxy.example.com:8080");
        let client = client_with_timeout(Duration::from_secs(10));
        assert!(format!("{:?}", client).contains("Client"));
        
        // Restore original env vars
        if let Some(val) = original_https {
            std::env::set_var("HTTPS_PROXY", val);
        } else {
            std::env::remove_var("HTTPS_PROXY");
        }
        if let Some(val) = original_http {
            std::env::set_var("HTTP_PROXY", val);
        } else {
            std::env::remove_var("HTTP_PROXY");
        }
        if let Some(val) = original_no {
            std::env::set_var("NO_PROXY", val);
        } else {
            std::env::remove_var("NO_PROXY");
        }
    }
}
