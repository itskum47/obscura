//! Obscura Content & Structured Data Extraction Engine.
//!
//! Provides pure-Rust extraction for web intelligence:
//! - Readability / main-article extraction and boilerplate elimination
//! - Document metadata (OpenGraph, Twitter Cards, canonical URLs, author, published date)
//! - JSON-LD Schema.org structured data normalization
//! - HTML table parsing to Markdown and structured JSON
//! - Link harvesting and resolution
//! - Sitemap and RSS/Atom feed parsing

pub mod feeds;
pub mod json_ld;
pub mod links;
pub mod metadata;
pub mod readability;
pub mod tables;

pub use feeds::{parse_feed, parse_sitemap, FeedEntry, FeedError, SitemapEntry};
pub use json_ld::{extract_structured_data, ArticleEntity, FaqItem, ProductEntity, StructuredDataResult};
pub use links::{extract_links, ExtractedLink};
pub use metadata::{extract_metadata, PageMetadata};
pub use readability::{extract_readability, ReadabilityOptions, ReadabilityResult};
pub use tables::{extract_tables, ExtractedTable};

use obscura_dom::parse_html;
use serde::{Deserialize, Serialize};

/// Comprehensive unified extraction result for a web page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UnifiedExtractionResult {
    /// Document and OpenGraph metadata.
    pub metadata: PageMetadata,
    /// Extracted main readable content.
    pub readability: ReadabilityResult,
    /// Extracted Schema.org structured data.
    pub structured_data: StructuredDataResult,
    /// Extracted data tables.
    pub tables: Vec<ExtractedTable>,
    /// Harvested hyperlinks.
    pub links: Vec<ExtractedLink>,
}

/// One-shot extraction parsing HTML once into a DomTree and extracting all structured components.
pub fn extract_all(html: &str, base_url: Option<&str>) -> UnifiedExtractionResult {
    let dom = parse_html(html);

    let metadata = extract_metadata(&dom);
    let readability = extract_readability(&dom, &ReadabilityOptions::default());
    let structured_data = extract_structured_data(&dom);
    let tables = extract_tables(&dom);
    let links = extract_links(&dom, base_url);

    UnifiedExtractionResult {
        metadata,
        readability,
        structured_data,
        tables,
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_all_end_to_end() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <title>Comprehensive Test Page</title>
                <meta name="description" content="A rich test document.">
                <meta property="og:title" content="OpenGraph Test Page">
                <link rel="canonical" href="https://example.com/test-page">
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "Article",
                    "headline": "Testing Rust Extractors",
                    "datePublished": "2026-11-18"
                }
                </script>
            </head>
            <body>
                <header><nav><a href="/">Home</a></nav></header>
                <article>
                    <h1>Main Article Title</h1>
                    <p>This is the first paragraph with vital technical information about autonomous web agents.</p>
                    <p>A second paragraph confirms that pure-Rust parsing yields sub-20ms latency.</p>
                    <table>
                        <thead><tr><th>Metric</th><th>Obscura</th><th>Chromium</th></tr></thead>
                        <tbody><tr><td>Startup</td><td>8.5ms</td><td>450ms</td></tr></tbody>
                    </table>
                </article>
                <footer><a href="/privacy">Privacy Policy</a></footer>
            </body>
            </html>
        "#;

        let result = extract_all(html, Some("https://example.com/test-page"));

        assert_eq!(result.metadata.title.as_deref(), Some("Comprehensive Test Page"));
        assert_eq!(result.metadata.canonical_url.as_deref(), Some("https://example.com/test-page"));
        assert!(result.readability.content_markdown.contains("This is the first paragraph"));
        assert_eq!(result.structured_data.articles.len(), 1);
        assert_eq!(result.structured_data.articles[0].headline.as_deref(), Some("Testing Rust Extractors"));
        assert_eq!(result.tables.len(), 1);
        assert_eq!(result.tables[0].headers, vec!["Metric", "Obscura", "Chromium"]);
        assert!(result.links.iter().any(|l| l.url == "https://example.com/privacy"));
    }
}
