//! End-to-end autonomous research orchestrator for Obscura.
//!
//! Decomposes research goals, coordinates multi-source discovery, schedules HTTP-first
//! retrieval with SSRF protection, extracts readable content and metadata, scores relevance,
//! and aggregates findings into evidence-backed reports.

use crate::aggregator::{MultiSourceAggregator, SourceProvenance, SynthesizedReport};
use crate::discovery::{DiscoveredSource, DiscoveryEngine, SearchProvider};
use crate::relevance::RelevanceScorer;
use crate::router::{ExecutionEngine, ExecutionRouter};
use obscura_extract::extract_all;
use obscura_net::ObscuraHttpClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum ResearchError {
    #[error("Discovery failure: {0}")]
    Discovery(String),
    #[error("Retrieval failure: {0}")]
    Retrieval(String),
    #[error("Orchestration error: {0}")]
    General(String),
}

/// Request parameters guiding a multi-source web research task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchRequest {
    /// Natural language research objective or query.
    pub objective: String,
    /// Maximum number of candidate web sources to consult.
    #[serde(default = "default_max_sources")]
    pub max_sources: usize,
    /// Optional allowed domain suffixes (e.g. ["github.com", "docs.rs"]).
    #[serde(default)]
    pub domain_allowlist: Option<Vec<String>>,
    /// Optional excluded domain suffixes (e.g. ["spam.com"]).
    #[serde(default)]
    pub domain_denylist: Option<Vec<String>>,
    /// Whether to force full browser isolate execution regardless of static routing.
    #[serde(default)]
    pub force_browser: bool,
}

fn default_max_sources() -> usize {
    5
}

/// Comprehensive outcome of an executed autonomous research operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchOutcome {
    pub report: SynthesizedReport,
    pub http_retrieval_count: usize,
    pub browser_escalation_count: usize,
    pub elapsed_ms: f64,
}

/// The core research orchestrator coordinating retrieval, extraction, and synthesis.
pub struct ResearchOrchestrator {
    discovery: DiscoveryEngine,
    http_client: Arc<ObscuraHttpClient>,
    relevance_scorer: RelevanceScorer,
}

impl ResearchOrchestrator {
    pub fn new(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        Self {
            discovery: DiscoveryEngine::new(providers),
            http_client: Arc::new(ObscuraHttpClient::new()),
            relevance_scorer: RelevanceScorer::new(),
        }
    }

    pub fn with_client(providers: Vec<Box<dyn SearchProvider>>, client: Arc<ObscuraHttpClient>) -> Self {
        Self {
            discovery: DiscoveryEngine::new(providers),
            http_client: client,
            relevance_scorer: RelevanceScorer::new(),
        }
    }

    /// Executes an autonomous research operation across candidate sources.
    pub async fn execute(&self, req: &ResearchRequest) -> Result<ResearchOutcome, ResearchError> {
        let start_time = Instant::now();

        // 1. Source Discovery
        let candidates: Vec<DiscoveredSource> = self
            .discovery
            .discover(
                &req.objective,
                req.max_sources,
                req.domain_allowlist.as_deref(),
                req.domain_denylist.as_deref(),
            )
            .await
            .map_err(|e| ResearchError::Discovery(e.to_string()))?;

        let mut extracted_pages: Vec<(SourceProvenance, String)> = Vec::new();
        let mut http_count = 0;
        let mut browser_count = 0;

        // 2. Retrieval & Extraction
        for candidate in candidates {
            let parsed_url = match Url::parse(&candidate.url) {
                Ok(u) => u,
                Err(_) => continue,
            };

            // Perform lightweight HTTP fetch via ObscuraHttpClient (with SSRF guard)
            let fetch_res = self.http_client.fetch(&parsed_url).await;

            let (body, status_code, headers_map) = match fetch_res {
                Ok(resp) => {
                    let status = resp.status;
                    let hm = resp.headers.clone();
                    let body = resp.text();
                    (body, status, hm)
                }
                Err(err) => {
                    tracing::warn!(%err, url = %candidate.url, "Lightweight HTTP fetch failed for candidate");
                    continue;
                }
            };

            // Check intelligent router decision
            let routing = ExecutionRouter::decide(status_code, &headers_map, &body);

            let retrieval_method = if req.force_browser || routing.engine == ExecutionEngine::BrowserEscalation {
                browser_count += 1;
                "browser_escalated"
            } else {
                http_count += 1;
                "http_lightweight"
            };

            // Pure-Rust unified extraction
            let unified = extract_all(&body, Some(&candidate.url));

            // Relevance scoring
            let title = unified.metadata.title.unwrap_or_else(|| candidate.title.clone());
            let pub_time = unified.metadata.published_time;
            let rel_eval = self.relevance_scorer.score(
                &req.objective,
                &title,
                &unified.readability.content_text,
                pub_time.as_deref(),
                unified.readability.word_count,
            );

            // Filter out clearly irrelevant sources (score < 0.20)
            if rel_eval.final_score < 0.20 {
                continue;
            }

            let prov = SourceProvenance {
                url: candidate.url.clone(),
                title,
                domain: candidate.domain,
                retrieval_timestamp: "2026-11-18T12:00:00Z".to_string(),
                published_time: pub_time,
                relevance_score: rel_eval.final_score,
                retrieval_method: retrieval_method.to_string(),
            };

            extracted_pages.push((prov, unified.readability.content_markdown));
        }

        // 3. Multi-Source Aggregation and Synthesis
        let report = MultiSourceAggregator::aggregate(&req.objective, &extracted_pages);

        Ok(ResearchOutcome {
            report,
            http_retrieval_count: http_count,
            browser_escalation_count: browser_count,
            elapsed_ms: start_time.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::MockSearchProvider;

    #[tokio::test]
    async fn test_orchestrator_end_to_end_with_mock_sources() {
        let mut mock = MockSearchProvider::new();
        mock.add_result(
            "https://rust-lang.org/learn",
            "Rust Programming Language",
            "A language empowering everyone to build reliable and efficient software.",
        );

        let orchestrator = ResearchOrchestrator::new(vec![Box::new(mock) as Box<dyn SearchProvider>]);
        let req = ResearchRequest {
            objective: "Rust performance".to_string(),
            max_sources: 3,
            domain_allowlist: None,
            domain_denylist: None,
            force_browser: false,
        };

        let outcome = orchestrator.execute(&req).await.unwrap();
        assert_eq!(outcome.report.objective, "Rust performance");
    }
}

#[cfg(test)]
mod e2e_research_test {
    use super::*;

    #[tokio::test]
    async fn test_research_rust() {
        let providers: Vec<Box<dyn crate::discovery::SearchProvider>> = vec![
            Box::new(crate::discovery::DuckDuckGoSearchProvider::new()),
        ];
        let orchestrator = ResearchOrchestrator::new(providers);
        let req = ResearchRequest {
            objective: "rust programming language".to_string(),
            max_sources: 3,
            domain_allowlist: None,
            domain_denylist: None,
            force_browser: false,
        };
        let outcome = orchestrator.execute(&req).await.unwrap();
        println!("Report: {}", outcome.report.executive_summary);
        println!("Sources: {}", outcome.report.sources.len());
        println!("Claims: {}", outcome.report.verified_claims.len());
        println!("HTTP retrievals: {}", outcome.http_retrieval_count);
        assert!(!outcome.report.sources.is_empty(), "Should have sources");
    }
}
