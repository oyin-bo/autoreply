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
