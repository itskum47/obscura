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
