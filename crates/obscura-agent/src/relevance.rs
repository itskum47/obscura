//! Transparent, explainable source relevance ranking pipeline.
//!
//! Evaluates candidate sources using BM25 query term matching, title salience,
//! content completeness, and freshness signals. Avoids black-box heuristics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Detailed breakdown of an explainable relevance evaluation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RelevanceEvaluation {
    /// Normalized overall score (0.0 to 1.0).
    pub final_score: f64,
    /// Term frequency match component (BM25-derived).
    pub term_salience: f64,
    /// Title term overlap ratio.
    pub title_match: f64,
    /// Content completeness and substantive word count factor.
    pub completeness_factor: f64,
    /// Freshness indicator (if publication date detected).
    pub freshness_factor: f64,
    /// Plain-English explanation of the score components.
    pub explanation: String,
}

/// Computes explainable relevance scores between a query and extracted page content.
pub struct RelevanceScorer {
    k1: f64,
    b: f64,
}

impl RelevanceScorer {
    pub fn new() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }

    /// Evaluates a candidate page against a research query.
    pub fn score(
        &self,
        query: &str,
        title: &str,
        main_content: &str,
        published_time: Option<&str>,
        word_count: usize,
    ) -> RelevanceEvaluation {
        let q_terms: Vec<String> = query
            .to_ascii_lowercase()
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|t| !t.is_empty())
            .collect();

        if q_terms.is_empty() {
            return RelevanceEvaluation {
                final_score: 0.5,
                explanation: "Empty query terms; assigned baseline score".to_string(),
                ..Default::default()
            };
        }

        // 1. Title term overlap
        let title_lower = title.to_ascii_lowercase();
        let title_matches = q_terms.iter().filter(|&t| title_lower.contains(t.as_str())).count();
        let title_match_ratio = title_matches as f64 / q_terms.len() as f64;

        // 2. BM25 Term Salience over main content
        let content_lower = main_content.to_ascii_lowercase();
        let doc_tokens: Vec<&str> = content_lower.split_whitespace().collect();
        let doc_len = doc_tokens.len().max(1);
        let avg_doc_len = 350.0; // Standard typical informational article length

        let mut term_freqs: HashMap<&str, usize> = HashMap::new();
        for token in &doc_tokens {
            let clean = token.trim_matches(|c: char| !c.is_alphanumeric());
            *term_freqs.entry(clean).or_insert(0) += 1;
        }

        let mut raw_bm25 = 0.0;
        for term in &q_terms {
            let tf = *term_freqs.get(term.as_str()).unwrap_or(&0) as f64;
            if tf > 0.0 {
                let numerator = tf * (self.k1 + 1.0);
                let denominator = tf + self.k1 * (1.0 - self.b + self.b * (doc_len as f64 / avg_doc_len));
                raw_bm25 += numerator / denominator;
            }
        }

        // Normalize BM25 component to roughly 0.0 - 1.0
        let term_salience = (raw_bm25 / (q_terms.len() as f64 * 1.5)).clamp(0.0, 1.0);

        // 3. Completeness factor (penalizes empty pages < 50 words, saturates at 300+ words)
        let completeness_factor = if word_count < 30 {
            0.2
        } else if word_count < 100 {
            0.5
        } else if word_count < 300 {
            0.8
        } else {
            1.0
        };

        // 4. Freshness factor
        let freshness_factor = if let Some(pub_str) = published_time {
            if pub_str.contains("2026") || pub_str.contains("2025") {
                1.0
            } else if pub_str.contains("2024") {
                0.8
            } else {
                0.6
            }
        } else {
            0.5 // Unknown date
        };

        // 5. Final composite score calculation:
        // Weights: Title (35%), Term Salience (40%), Completeness (15%), Freshness (10%)
        let final_score = (title_match_ratio * 0.35)
            + (term_salience * 0.40)
            + (completeness_factor * 0.15)
            + (freshness_factor * 0.10);

        let final_score = (final_score * 100.0).round() / 100.0;

        let explanation = format!(
            "Score {:.2}: Title match {}/{} terms ({:.0}%), content term frequency {:.2}, completeness {:.0}% ({} words), freshness factor {:.1}",
            final_score,
            title_matches,
            q_terms.len(),
            title_match_ratio * 100.0,
            term_salience,
            completeness_factor * 100.0,
            word_count,
            freshness_factor
        );

        RelevanceEvaluation {
            final_score,
            term_salience,
            title_match: title_match_ratio,
            completeness_factor,
            freshness_factor,
            explanation,
        }
    }
}

impl Default for RelevanceScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relevance_scorer_salience_and_explanation() {
        let scorer = RelevanceScorer::new();
        let query = "rust browser engine memory";
        let title = "Building a High Performance Rust Browser Engine";
        let body = "This article details how our headless browser in Rust manages memory efficiently without V8 bloat. The engine scales linearly.";

        let eval = scorer.score(query, title, body, Some("2026-11-18"), 120);

        assert!(eval.final_score > 0.5);
        assert!(eval.title_match > 0.7);
        assert!(eval.explanation.contains("Title match"));
    }
}
