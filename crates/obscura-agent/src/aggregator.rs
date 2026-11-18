//! Multi-source research synthesis, fact aggregation, and conflict detection.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Precise evidence citation linking an extracted claim to its original source.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCitation {
    pub source_url: String,
    pub domain: String,
    pub snippet_quote: String,
    pub published_at: Option<String>,
}

/// Sourced fact with supporting evidence citations.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FactClaim {
    pub statement: String,
    pub citations: Vec<EvidenceCitation>,
    pub consensus_count: usize,
    pub is_disputed: bool,
}

/// A detected contradiction or inconsistency between independent sources.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DetectedContradiction {
    pub topic: String,
    pub claim_a: String,
    pub source_a: EvidenceCitation,
    pub claim_b: String,
    pub source_b: EvidenceCitation,
    pub discrepancy_detail: String,
}

/// Full provenance metadata for a consulted source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub url: String,
    pub title: String,
    pub domain: String,
    pub retrieval_timestamp: String,
    pub published_time: Option<String>,
    pub relevance_score: f64,
    pub retrieval_method: String,
}

/// Synthesized research report aggregating multi-source findings with provenance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SynthesizedReport {
    pub objective: String,
    pub executive_summary: String,
    pub verified_claims: Vec<FactClaim>,
    pub contradictions: Vec<DetectedContradiction>,
    pub sources: Vec<SourceProvenance>,
    pub limitations: Vec<String>,
}

/// Aggregates and cross-references extracted information from multiple web sources.
pub struct MultiSourceAggregator;

impl MultiSourceAggregator {
    /// Ingests extracted pages, clusters sentences, eliminates near-duplicates, and identifies contradictions.
    pub fn aggregate(
        objective: &str,
        extracted_pages: &[(SourceProvenance, String)], // (provenance, markdown_text)
    ) -> SynthesizedReport {
        let mut report = SynthesizedReport {
            objective: objective.to_string(),
            sources: extracted_pages.iter().map(|(p, _)| p.clone()).collect(),
            ..Default::default()
        };

        if extracted_pages.is_empty() {
            report.limitations.push("No relevant web sources could be retrieved.".to_string());
            return report;
        }

        let mut candidate_sentences: Vec<(String, EvidenceCitation)> = Vec::new();

        // 1. Sentence extraction and attribution
        for (prov, content) in extracted_pages {
            for raw_line in content.lines() {
                let trimmed = raw_line.trim().trim_start_matches(|c| c == '-' || c == '*' || c == '#').trim();
                if trimmed.len() < 30 || trimmed.len() > 300 {
                    continue;
                }

                candidate_sentences.push((
                    trimmed.to_string(),
                    EvidenceCitation {
                        source_url: prov.url.clone(),
                        domain: prov.domain.clone(),
                        snippet_quote: trimmed.to_string(),
                        published_at: prov.published_time.clone(),
                    },
                ));
            }
        }

        // 2. Sentence clustering and near-deduplication (Jaccard token similarity)
        let mut clustered_claims: Vec<FactClaim> = Vec::new();
        let mut seen_tokens: Vec<HashSet<String>> = Vec::new();

        for (sentence, citation) in candidate_sentences {
            let tokens: HashSet<String> = sentence
                .to_ascii_lowercase()
                .split_whitespace()
                .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|s| s.len() > 3)
                .collect();

            if tokens.is_empty() {
                continue;
            }

            let mut matched_cluster = None;
            for (idx, cluster_tokens) in seen_tokens.iter().enumerate() {
                let intersection = tokens.intersection(cluster_tokens).count();
                let union = tokens.union(cluster_tokens).count();
                let jaccard = if union > 0 { intersection as f64 / union as f64 } else { 0.0 };

                if jaccard > 0.60 {
                    matched_cluster = Some(idx);
                    break;
                }
            }

            if let Some(c_idx) = matched_cluster {
                clustered_claims[c_idx].citations.push(citation);
                clustered_claims[c_idx].consensus_count += 1;
            } else {
                seen_tokens.push(tokens);
                clustered_claims.push(FactClaim {
                    statement: sentence,
                    citations: vec![citation],
                    consensus_count: 1,
                    is_disputed: false,
                });
            }
        }

        // 3. Contradiction detection heuristic
        // Flags sentences sharing subject words but containing differing numbers or opposite sentiment
        let mut contradictions = Vec::new();
        for i in 0..clustered_claims.len() {
            for j in (i + 1)..clustered_claims.len() {
                let text_a = &clustered_claims[i].statement;
                let text_b = &clustered_claims[j].statement;

                let numbers_a: Vec<&str> = text_a.split_whitespace().filter(|s| s.chars().any(|c| c.is_ascii_digit())).collect();
                let numbers_b: Vec<&str> = text_b.split_whitespace().filter(|s| s.chars().any(|c| c.is_ascii_digit())).collect();

                // If both discuss identical keywords but cite distinct numeric metrics
                if !numbers_a.is_empty() && !numbers_b.is_empty() && numbers_a != numbers_b {
                    let words_a: HashSet<&str> = text_a.split_whitespace().collect();
                    let words_b: HashSet<&str> = text_b.split_whitespace().collect();
                    let shared = words_a.intersection(&words_b).count();

                    if shared >= 4 {
                        let cite_a = clustered_claims[i].citations.first().cloned().unwrap_or_default();
                        let cite_b = clustered_claims[j].citations.first().cloned().unwrap_or_default();

                        contradictions.push(DetectedContradiction {
                            topic: format!("Numeric discrepancy in statements sharing {} words", shared),
                            claim_a: text_a.clone(),
                            source_a: cite_a,
                            claim_b: text_b.clone(),
                            source_b: cite_b,
                            discrepancy_detail: format!("Source A mentions {:?} whereas Source B states {:?}", numbers_a, numbers_b),
                        });
                        clustered_claims[i].is_disputed = true;
                        clustered_claims[j].is_disputed = true;
                    }
                }
            }
        }

        // 4. Executive Summary Generation
        let top_facts: Vec<&FactClaim> = clustered_claims.iter().take(4).collect();
        let summary = if !top_facts.is_empty() {
            format!(
                "Synthesis across {} consulted sources for '{}'. Key consensus indicates: {}",
                extracted_pages.len(),
                objective,
                top_facts.iter().map(|f| f.statement.as_str()).collect::<Vec<_>>().join(" ")
            )
        } else {
            format!("Retrieved {} sources for '{}'.", extracted_pages.len(), objective)
        };

        report.executive_summary = summary;
        report.verified_claims = clustered_claims;
        report.contradictions = contradictions;

        if report.sources.len() < 3 {
            report.limitations.push(format!(
                "Research drew upon only {} sources; broader cross-validation recommended.",
                report.sources.len()
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_source_aggregator_clustering_and_citations() {
        let prov1 = SourceProvenance {
            url: "https://source1.org/benchmarks".to_string(),
            title: "Source 1 Benchmarks".to_string(),
            domain: "source1.org".to_string(),
            retrieval_timestamp: "2026-11-18T12:00:00Z".to_string(),
            published_time: Some("2026-11-18".to_string()),
            relevance_score: 0.95,
            retrieval_method: "http_lightweight".to_string(),
        };

        let prov2 = SourceProvenance {
            url: "https://source2.org/latency".to_string(),
            title: "Source 2 Latency".to_string(),
            domain: "source2.org".to_string(),
            retrieval_timestamp: "2026-11-18T12:00:00Z".to_string(),
            published_time: Some("2026-11-18".to_string()),
            relevance_score: 0.92,
            retrieval_method: "http_lightweight".to_string(),
        };

        let text1 = "The pure-Rust parser processes documents with sub-20ms latency on modern hardware.";
        let text2 = "Pure-Rust parser processes standard web documents with sub-20ms latency on hardware.";

        let report = MultiSourceAggregator::aggregate(
            "Rust browser latency",
            &[(prov1, text1.to_string()), (prov2, text2.to_string())],
        );

        assert_eq!(report.sources.len(), 2);
        assert!(!report.verified_claims.is_empty());
        assert_eq!(report.verified_claims[0].citations.len(), 2); // Clustered together!
        assert!(report.executive_summary.contains("sub-20ms latency"));
    }
}
