//! Source discovery engine and search provider adapters.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;
use url::Url;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("Network error during discovery: {0}")]
    Network(String),
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

/// A discovered web source candidate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredSource {
    /// Normalized absolute URL.
    pub url: String,
    /// Title of the search result.
    pub title: String,
    /// Extracted search snippet.
    pub snippet: String,
    /// Hostname / domain.
    pub domain: String,
    /// Discovery provenance (e.g. "duckduckgo", "brave", "sitemap", "mock").
    pub provider: String,
}

/// Pluggable search provider adapter trait.
pub trait SearchProvider: Send + Sync {
    /// Discovers candidate sources for a query up to `limit`.
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<DiscoveredSource>, DiscoveryError>>;
    /// Identifies the provider name.
    fn provider_name(&self) -> &'static str;
}

/// In-memory mock search provider for unit testing, offline verification, and deterministic CI.
pub struct MockSearchProvider {
    canned_results: Vec<DiscoveredSource>,
}

impl MockSearchProvider {
    pub fn new() -> Self {
        Self {
            canned_results: Vec::new(),
        }
    }

    pub fn with_canned(results: Vec<DiscoveredSource>) -> Self {
        Self {
            canned_results: results,
        }
    }

    pub fn add_result(&mut self, url: &str, title: &str, snippet: &str) {
        let domain = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(ToString::to_string))
            .unwrap_or_default();
        self.canned_results.push(DiscoveredSource {
            url: url.to_string(),
            title: title.to_string(),
            snippet: snippet.to_string(),
            domain,
            provider: "mock".to_string(),
        });
    }
}

impl Default for MockSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for MockSearchProvider {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<DiscoveredSource>, DiscoveryError>> {
        Box::pin(async move {
            let q_lower = query.to_ascii_lowercase();
            let terms: Vec<&str> = q_lower.split_whitespace().collect();

            let mut matched: Vec<DiscoveredSource> = self
                .canned_results
                .iter()
                .filter(|r| {
                    if terms.is_empty() {
                        return true;
                    }
                    let haystack = format!("{} {} {}", r.title, r.snippet, r.url).to_ascii_lowercase();
                    terms.iter().any(|&term| haystack.contains(term))
                })
                .cloned()
                .take(limit)
                .collect();

            // If no canned results matched, generate query-relevant deterministic synthetic hits
            if matched.is_empty() {
                let encoded_q = query.replace(' ', "-").to_ascii_lowercase();
                for i in 1..=limit.min(3) {
                    let domain = format!("source{}.example.org", i);
                    let url = format!("https://{}/articles/{}", domain, encoded_q);
                    matched.push(DiscoveredSource {
                        url,
                        title: format!("Authoritative Analysis of {}: Part {}", query, i),
                        snippet: format!("Comprehensive technical guide and empirical data covering {} in depth.", query),
                        domain,
                        provider: "mock_synthesizer".to_string(),
                    });
                }
            }

            Ok(matched)
        })
    }

    fn provider_name(&self) -> &'static str {
        "mock"
    }
}

/// Brave Search API provider.
///
/// Requires the `BRAVE_API_KEY` environment variable. Uses the Brave Search
/// Web Search API (https://api.search.brave.com/res/v1/web/search).
pub struct BraveSearchProvider {
    api_key: String,
    client: reqwest::Client,
}

impl BraveSearchProvider {
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("BRAVE_API_KEY").ok()?;
        if key.trim().is_empty() {
            return None;
        }
        Some(Self {
            api_key: key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        })
    }

    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl SearchProvider for BraveSearchProvider {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<DiscoveredSource>, DiscoveryError>> {
        Box::pin(async move {
            let resp = self
                .client
                .get("https://api.search.brave.com/res/v1/web/search")
                .query(&[("q", query), ("count", &limit.min(20).to_string())])
                .header("X-Subscription-Token", &self.api_key)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| DiscoveryError::Network(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(DiscoveryError::Provider(format!(
                    "Brave API returned {}",
                    resp.status()
                )));
            }

            let text = resp
                .text()
                .await
                .map_err(|e| DiscoveryError::Provider(e.to_string()))?;
            let body: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| DiscoveryError::Provider(e.to_string()))?;

            let mut results = Vec::new();
            if let Some(web) = body.get("web").and_then(|w| w.get("results")) {
                if let Some(arr) = web.as_array() {
                    for item in arr {
                        let url = item
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let title = item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let snippet = item
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        if url.is_empty() {
                            continue;
                        }
                        let domain = url::Url::parse(&url)
                            .ok()
                            .and_then(|u| u.host_str().map(ToString::to_string))
                            .unwrap_or_default();
                        results.push(DiscoveredSource {
                            url,
                            title,
                            snippet,
                            domain,
                            provider: "brave".to_string(),
                        });
                    }
                }
            }
            Ok(results)
        })
    }

    fn provider_name(&self) -> &'static str {
        "brave"
    }
}

/// DuckDuckGo HTML search provider (free, no API key required).
///
/// Scrapes DuckDuckGo's HTML results page. Fragile but free.
pub struct DuckDuckGoSearchProvider {
    client: reqwest::Client,
}

impl DuckDuckGoSearchProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (compatible; ObscuraBot/1.0)")
                .build()
                .unwrap_or_default(),
        }
    }
}

impl SearchProvider for DuckDuckGoSearchProvider {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<DiscoveredSource>, DiscoveryError>> {
        Box::pin(async move {
            let resp = self
                .client
                .get("https://api.duckduckgo.com/")
                .query(&[("q", query), ("format", "json"), ("no_html", "1"), ("skip_disambig", "1")])
                .send()
                .await
                .map_err(|e| DiscoveryError::Network(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(DiscoveryError::Provider(format!(
                    "DuckDuckGo returned {}",
                    resp.status()
                )));
            }

            let text = resp.text().await.map_err(|e| DiscoveryError::Provider(e.to_string()))?;
            let body: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| DiscoveryError::Provider(e.to_string()))?;

            let mut results = Vec::new();

            // Extract from Results (direct URLs)
            if let Some(arr) = body.get("Results").and_then(|v| v.as_array()) {
                for item in arr {
                    let url = item.get("FirstURL").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let title = item.get("Text").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    if url.is_empty() { continue; }
                    let domain = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(ToString::to_string)).unwrap_or_default();
                    results.push(DiscoveredSource {
                        url,
                        title,
                        snippet: String::new(),
                        domain,
                        provider: "duckduckgo".to_string(),
                    });
                    if results.len() >= limit { break; }
                }
            }

            // Extract Abstract URL (usually Wikipedia)
            if results.len() < limit {
                if let Some(url) = body.get("AbstractURL").and_then(|v| v.as_str()) {
                    if !url.is_empty() {
                        let title = body.get("Heading").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let snippet = body.get("Abstract").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let domain = url::Url::parse(url).ok().and_then(|u| u.host_str().map(ToString::to_string)).unwrap_or_default();
                        results.push(DiscoveredSource {
                            url: url.to_string(),
                            title,
                            snippet,
                            domain,
                            provider: "duckduckgo".to_string(),
                        });
                    }
                }
            }

            // Extract from RelatedTopics (some have external URLs)
            if results.len() < limit {
                if let Some(topics) = body.get("RelatedTopics").and_then(|v| v.as_array()) {
                    for item in topics {
                        if results.len() >= limit { break; }
                        if let Some(url) = item.get("FirstURL").and_then(|v| v.as_str()) {
                            if url.is_empty() || url.contains("duckduckgo.com") { continue; }
                            let title = item.get("Text").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let domain = url::Url::parse(url).ok().and_then(|u| u.host_str().map(ToString::to_string)).unwrap_or_default();
                            results.push(DiscoveredSource {
                                url: url.to_string(),
                                title,
                                snippet: String::new(),
                                domain,
                                provider: "duckduckgo".to_string(),
                            });
                        }
                    }
                }
            }

            Ok(results)
        })
    }

    fn provider_name(&self) -> &'static str {
        "duckduckgo"
    }
}


/// Normalizes URLs by removing tracking parameters and trailing slashes.
pub struct UrlNormalizer;

impl UrlNormalizer {
    const TRACKING_PARAMS: &'static [&'static str] = &[
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "mc_cid",
        "mc_eid",
        "ref",
        "source",
        "_ga",
    ];

    /// Cleans and canonicalizes a URL string.
    pub fn normalize(raw_url: &str) -> Result<String, DiscoveryError> {
        let mut parsed = Url::parse(raw_url.trim()).map_err(|e| DiscoveryError::InvalidUrl(e.to_string()))?;

        // Clear fragment (#section)
        parsed.set_fragment(None);

        // Strip tracking query parameters
        let filtered_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !Self::TRACKING_PARAMS.contains(&k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if filtered_pairs.is_empty() {
            parsed.set_query(None);
        } else {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in filtered_pairs {
                serializer.append_pair(&k, &v);
            }
            parsed.set_query(Some(&serializer.finish()));
        }

        let mut res = parsed.to_string();
        // Remove trailing slash if path is only '/'
        if res.ends_with('/') && parsed.path() == "/" && parsed.query().is_none() {
            res.pop();
        }

        Ok(res)
    }
}

/// Orchestrates multi-provider discovery and deduplication.
pub struct DiscoveryEngine {
    providers: Vec<Box<dyn SearchProvider>>,
}

impl DiscoveryEngine {
    pub fn new(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        Self { providers }
    }

    /// Discovers candidate sources across registered search providers, deduplicating URLs.
    pub async fn discover(
        &self,
        query: &str,
        limit: usize,
        domain_allowlist: Option<&[String]>,
        domain_denylist: Option<&[String]>,
    ) -> Result<Vec<DiscoveredSource>, DiscoveryError> {
        let mut results = Vec::new();
        let mut seen_urls = HashSet::new();

        for provider in &self.providers {
            if results.len() >= limit {
                break;
            }

            let provider_hits = provider.search(query, limit).await?;
            for hit in provider_hits {
                let normalized_url = match UrlNormalizer::normalize(&hit.url) {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                if seen_urls.contains(&normalized_url) {
                    continue;
                }

                // Domain filters
                if let Some(allow) = domain_allowlist {
                    if !allow.iter().any(|d| hit.domain.ends_with(d)) {
                        continue;
                    }
                }

                if let Some(deny) = domain_denylist {
                    if deny.iter().any(|d| hit.domain.ends_with(d)) {
                        continue;
                    }
                }

                seen_urls.insert(normalized_url.clone());
                results.push(DiscoveredSource {
                    url: normalized_url,
                    title: hit.title,
                    snippet: hit.snippet,
                    domain: hit.domain,
                    provider: provider.provider_name().to_string(),
                });

                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_url_normalizer_strips_tracking_params() {
        let url = "https://example.com/blog/article/?utm_source=twitter&utm_medium=social&id=123#comments";
        let normalized = UrlNormalizer::normalize(url).unwrap();
        assert_eq!(normalized, "https://example.com/blog/article/?id=123");
    }

    #[tokio::test]
    async fn test_discovery_engine_with_mock_provider() {
        let mut mock = MockSearchProvider::new();
        mock.add_result(
            "https://docs.example.org/guide?utm_source=test",
            "Example Guide",
            "Introductory guide to systems programming.",
        );
        mock.add_result(
            "https://denied.com/spam",
            "Spam Site",
            "Low quality marketing spam.",
        );

        let engine = DiscoveryEngine::new(vec![Box::new(mock) as Box<dyn SearchProvider>]);
        let deny = vec!["denied.com".to_string()];
        let hits = engine.discover("guide", 5, None, Some(&deny)).await.unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://docs.example.org/guide");
        assert_eq!(hits[0].title, "Example Guide");
        assert_eq!(hits[0].domain, "docs.example.org");
    }
}

#[cfg(test)]
mod ddg_tests {
    use super::*;

    #[tokio::test]
    async fn test_duckduckgo_search() {
        let provider = DuckDuckGoSearchProvider::new();
        let results = provider.search("rust programming language", 5).await.unwrap();
        println!("Got {} results", results.len());
        for r in &results { println!("  {} - {}", r.title, r.url); }
        println!("Got {} results", results.len());
        for r in &results {
            println!("  {} - {}", r.title, r.url);
        }
        assert!(!results.is_empty(), "Should get at least one result");
    }
}
