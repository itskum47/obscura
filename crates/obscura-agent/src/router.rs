//! Intelligent Execution Router for Obscura.
//!
//! Analyzes page characteristics, response headers, and DOM indicators to determine
//! whether lightweight HTTP retrieval is sufficient or whether escalation to the
//! full V8 browser engine is strictly required.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Recommended execution mode for a web request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEngine {
    /// Lightweight HTTP fetch + pure-Rust DOM extraction (< 20ms, zero V8).
    HttpLightweight,
    /// Escalated headless browser execution with V8 JavaScript runtime & DOM interaction.
    BrowserEscalation,
    /// Direct structured API / Feed consumption (JSON, XML, Sitemap, RSS).
    StructuredFeedOrApi,
}

/// Routing decision explaining why an execution engine was chosen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected engine.
    pub engine: ExecutionEngine,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Concrete reason for the routing decision.
    pub reason: String,
    /// Observable markers detected in the response.
    pub detected_signals: Vec<String>,
}

/// Analyzes an HTTP response to determine the optimal execution path.
pub struct ExecutionRouter;

impl ExecutionRouter {
    /// Analyzes response status, headers, and body HTML to produce a routing decision.
    pub fn decide(
        status_code: u16,
        headers: &HashMap<String, String>,
        body: &str,
    ) -> RoutingDecision {
        let mut signals = Vec::new();

        // 1. Content-type check for feeds and APIs
        let content_type = headers
            .get("content-type")
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if content_type.contains("application/json") || content_type.contains("text/json") {
            signals.push("content_type:json".to_string());
            return RoutingDecision {
                engine: ExecutionEngine::StructuredFeedOrApi,
                confidence: 1.0,
                reason: "Direct JSON response detected; no DOM or browser required".to_string(),
                detected_signals: signals,
            };
        }

        if content_type.contains("xml") || body.trim_start().starts_with("<?xml") {
            if body.contains("<urlset") || body.contains("<sitemapindex") || body.contains("<rss") || body.contains("<feed") {
                signals.push("content_type:xml_feed".to_string());
                return RoutingDecision {
                    engine: ExecutionEngine::StructuredFeedOrApi,
                    confidence: 1.0,
                    reason: "XML Sitemap or RSS/Atom feed detected".to_string(),
                    detected_signals: signals,
                };
            }
        }

        // 2. Client-side SPA and dynamic framework markers
        let body_lower = body.to_ascii_lowercase();

        let mut spa_signals = Vec::new();
        if body_lower.contains("id=\"root\"") || body_lower.contains("id='root'") {
            spa_signals.push("react_root_container");
        }
        if body_lower.contains("id=\"app\"") || body_lower.contains("id='app'") {
            spa_signals.push("vue_or_app_container");
        }
        if body_lower.contains("<app-root") {
            spa_signals.push("angular_app_root");
        }
        if body_lower.contains("__next_data__") && body_lower.contains("\"page\":\"/") {
            spa_signals.push("nextjs_hydration_marker");
        }

        // 3. Noscript warning detection
        let mut noscript_gate = false;
        if body_lower.contains("<noscript>") {
            if body_lower.contains("enable javascript")
                || body_lower.contains("requires javascript")
                || body_lower.contains("javascript is disabled")
                || body_lower.contains("please turn on javascript")
            {
                noscript_gate = true;
                signals.push("noscript_js_required".to_string());
            }
        }

        // 4. Text density vs script volume
        let script_count = body_lower.matches("<script").count();
        let rough_text_len = body
            .split('<')
            .filter_map(|s| s.split_once('>').map(|(_, text)| text.trim()))
            .map(|t| t.len())
            .sum::<usize>();

        // Heuristics:
        // Case A: Strong JavaScript gate or empty SPA shell
        if noscript_gate || (!spa_signals.is_empty() && rough_text_len < 250 && script_count >= 2) {
            for s in spa_signals {
                signals.push(s.to_string());
            }
            signals.push(format!("low_text_len:{}", rough_text_len));
            signals.push(format!("script_count:{}", script_count));

            return RoutingDecision {
                engine: ExecutionEngine::BrowserEscalation,
                confidence: 0.95,
                reason: "Client-rendered SPA container with minimal static text requires JavaScript execution".to_string(),
                detected_signals: signals,
            };
        }

        // Case B: Static informational content is rich and readable (> 300 characters of text)
        if rough_text_len > 300 {
            signals.push(format!("rich_text_len:{}", rough_text_len));
            return RoutingDecision {
                engine: ExecutionEngine::HttpLightweight,
                confidence: 0.90,
                reason: "Document contains rich static textual content; fast HTTP + pure-Rust extraction optimal".to_string(),
                detected_signals: signals,
            };
        }

        // Default: If status is OK and not clearly an empty SPA, prefer lightweight HTTP first
        if status_code == 200 {
            signals.push("default_http_first".to_string());
            RoutingDecision {
                engine: ExecutionEngine::HttpLightweight,
                confidence: 0.75,
                reason: "Standard HTTP response without explicit SPA gating; attempting lightweight extraction first".to_string(),
                detected_signals: signals,
            }
        } else {
            signals.push(format!("http_status:{}", status_code));
            RoutingDecision {
                engine: ExecutionEngine::BrowserEscalation,
                confidence: 0.60,
                reason: format!("Non-200 HTTP status ({}); escalating to browser for interactive verification", status_code),
                detected_signals: signals,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_detects_json() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json; charset=utf-8".to_string());
        let decision = ExecutionRouter::decide(200, &headers, r#"{"status":"ok","items":[]}"#);
        assert_eq!(decision.engine, ExecutionEngine::StructuredFeedOrApi);
        assert_eq!(decision.confidence, 1.0);
    }

    #[test]
    fn test_router_detects_static_article() {
        let headers = HashMap::new();
        let body = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Technical Architecture</title></head>
            <body>
                <h1>Deep Dive into Rust Memory Management</h1>
                <p>Rust achieves memory safety without garbage collection through its strict ownership, borrowing, and lifetime rules.</p>
                <p>When compiling high-throughput distributed systems, RAII ensures deterministic teardown of network sockets and file handles.</p>
                <p>This article covers how concurrency models interact with Send and Sync marker traits across worker thread pools.</p>
            </body>
            </html>
        "#;
        let decision = ExecutionRouter::decide(200, &headers, body);
        assert_eq!(decision.engine, ExecutionEngine::HttpLightweight);
        assert!(decision.confidence >= 0.85);
    }

    #[test]
    fn test_router_escalates_on_empty_spa_shell() {
        let headers = HashMap::new();
        let body = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>React App</title>
                <script src="/static/js/bundle.main.19482.js"></script>
                <script src="/static/js/vendor.29482.js"></script>
            </head>
            <body>
                <noscript>You need to enable JavaScript to run this app.</noscript>
                <div id="root"></div>
            </body>
            </html>
        "#;
        let decision = ExecutionRouter::decide(200, &headers, body);
        assert_eq!(decision.engine, ExecutionEngine::BrowserEscalation);
        assert!(decision.confidence >= 0.90);
        assert!(decision.reason.contains("Client-rendered SPA"));
    }
}
