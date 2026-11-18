//! Pure-Rust HTML metadata extraction (OpenGraph, Twitter Cards, SEO tags, Canonical URLs).

use obscura_dom::DomTree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Structured metadata extracted from an HTML document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageMetadata {
    /// Document title (prefers og:title / twitter:title / <title>).
    pub title: Option<String>,
    /// Meta description or og:description.
    pub description: Option<String>,
    /// Canonical URL declared by the document.
    pub canonical_url: Option<String>,
    /// Author name declared in meta tags.
    pub author: Option<String>,
    /// Publication timestamp (e.g. article:published_time or date).
    pub published_time: Option<String>,
    /// Document language from <html lang="...">.
    pub language: Option<String>,
    /// Declared site name (e.g. og:site_name).
    pub site_name: Option<String>,
    /// Keywords list.
    pub keywords: Vec<String>,
    /// All OpenGraph properties (e.g. og:image, og:type, og:locale).
    pub open_graph: HashMap<String, String>,
    /// All Twitter Card properties (e.g. twitter:card, twitter:image).
    pub twitter_card: HashMap<String, String>,
}

/// Extracts all standard metadata, OpenGraph, and Twitter Card attributes from a DOM tree.
pub fn extract_metadata(dom: &DomTree) -> PageMetadata {
    let mut meta = PageMetadata::default();

    // 1. Extract <html lang="...">
    if let Ok(Some(html_id)) = dom.query_selector("html") {
        if let Some(lang) = dom.with_node(html_id, |n| n.get_attribute("lang").map(|s| s.to_string())).flatten() {
            if !lang.trim().is_empty() {
                meta.language = Some(lang.trim().to_string());
            }
        }
    }

    // 2. Extract <title>
    if let Ok(Some(title_id)) = dom.query_selector("title") {
        let text = dom.text_content(title_id).trim().to_string();
        if !text.is_empty() {
            meta.title = Some(text);
        }
    }

    // 3. Extract <link rel="canonical" href="...">
    if let Ok(links) = dom.query_selector_all("link[rel='canonical']") {
        for link_id in links {
            if let Some(href) = dom.with_node(link_id, |n| n.get_attribute("href").map(|s| s.to_string())).flatten() {
                if !href.trim().is_empty() {
                    meta.canonical_url = Some(href.trim().to_string());
                    break;
                }
            }
        }
    }

    // 4. Extract all <meta> tags
    if let Ok(meta_tags) = dom.query_selector_all("meta") {
        for tag_id in meta_tags {
            let (name, property, content) = dom
                .with_node(tag_id, |n| {
                    let name = n.get_attribute("name").map(|s| s.to_ascii_lowercase());
                    let property = n.get_attribute("property").map(|s| s.to_ascii_lowercase());
                    let content = n.get_attribute("content").map(|s| s.to_string());
                    (name, property, content)
                })
                .unwrap_or((None, None, None));

            let Some(content) = content else { continue };
            let trimmed_content = content.trim();
            if trimmed_content.is_empty() {
                continue;
            }

            // OpenGraph tags (property="og:..." or name="og:...")
            let og_key = property.as_ref().or(name.as_ref()).filter(|k| k.starts_with("og:"));
            if let Some(key) = og_key {
                meta.open_graph.insert(key.clone(), trimmed_content.to_string());
                match key.as_str() {
                    "og:title" => {
                        if meta.title.is_none() {
                            meta.title = Some(trimmed_content.to_string());
                        }
                    }
                    "og:description" => {
                        if meta.description.is_none() {
                            meta.description = Some(trimmed_content.to_string());
                        }
                    }
                    "og:site_name" => {
                        meta.site_name = Some(trimmed_content.to_string());
                    }
                    "og:url" => {
                        if meta.canonical_url.is_none() {
                            meta.canonical_url = Some(trimmed_content.to_string());
                        }
                    }
                    _ => {}
                }
            }

            // Twitter Card tags (name="twitter:..." or property="twitter:...")
            let tw_key = name.as_ref().or(property.as_ref()).filter(|k| k.starts_with("twitter:"));
            if let Some(key) = tw_key {
                meta.twitter_card.insert(key.clone(), trimmed_content.to_string());
                if key == "twitter:title" && meta.title.is_none() {
                    meta.title = Some(trimmed_content.to_string());
                }
                if key == "twitter:description" && meta.description.is_none() {
                    meta.description = Some(trimmed_content.to_string());
                }
            }

            // Article metadata (article:published_time, article:author, etc.)
            let art_key = property.as_ref().or(name.as_ref()).filter(|k| k.starts_with("article:"));
            if let Some(key) = art_key {
                if key == "article:published_time" {
                    meta.published_time = Some(trimmed_content.to_string());
                } else if key == "article:author" && meta.author.is_none() {
                    meta.author = Some(trimmed_content.to_string());
                }
            }

            // Standard metadata
            if let Some(ref n) = name {
                match n.as_str() {
                    "description" => {
                        if meta.description.is_none() {
                            meta.description = Some(trimmed_content.to_string());
                        }
                    }
                    "author" => {
                        if meta.author.is_none() {
                            meta.author = Some(trimmed_content.to_string());
                        }
                    }
                    "keywords" => {
                        meta.keywords = trimmed_content
                            .split(',')
                            .map(|k| k.trim().to_string())
                            .filter(|k| !k.is_empty())
                            .collect();
                    }
                    "pubdate" | "publishdate" | "date" => {
                        if meta.published_time.is_none() {
                            meta.published_time = Some(trimmed_content.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use obscura_dom::parse_html;

    #[test]
    fn test_extract_metadata_full() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="en-US">
            <head>
                <title>Test Page Title</title>
                <meta name="description" content="A comprehensive test description.">
                <meta name="author" content="Jane Doe">
                <meta name="keywords" content="rust, browser, agent, mcp">
                <link rel="canonical" href="https://example.com/canonical-test">
                <meta property="og:title" content="OpenGraph Title">
                <meta property="og:description" content="OG description text.">
                <meta property="og:site_name" content="ExampleSite">
                <meta property="og:image" content="https://example.com/og.png">
                <meta property="article:published_time" content="2026-11-18T12:00:00Z">
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:site" content="@example">
            </head>
            <body><h1>Hello World</h1></body>
            </html>
        "#;
        let dom = parse_html(html);
        let meta = extract_metadata(&dom);

        assert_eq!(meta.title.as_deref(), Some("Test Page Title"));
        assert_eq!(meta.description.as_deref(), Some("A comprehensive test description."));
        assert_eq!(meta.canonical_url.as_deref(), Some("https://example.com/canonical-test"));
        assert_eq!(meta.author.as_deref(), Some("Jane Doe"));
        assert_eq!(meta.language.as_deref(), Some("en-US"));
        assert_eq!(meta.site_name.as_deref(), Some("ExampleSite"));
        assert_eq!(meta.published_time.as_deref(), Some("2026-11-18T12:00:00Z"));
        assert_eq!(meta.keywords, vec!["rust", "browser", "agent", "mcp"]);
        assert_eq!(meta.open_graph.get("og:image").map(String::as_str), Some("https://example.com/og.png"));
        assert_eq!(meta.twitter_card.get("twitter:card").map(String::as_str), Some("summary_large_image"));
    }
}
