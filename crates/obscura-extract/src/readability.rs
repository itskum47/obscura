//! Pure-Rust readability main-content extraction algorithm.
//!
//! Evaluates DOM node candidate scores using semantic tag weighting, class/id heuristics,
//! text density, and link density analysis to extract primary article text while
//! eliminating navigation, sidebars, advertisements, and footer boilerplate.

use obscura_dom::{DomTree, NodeData, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Outcome of the readability extraction algorithm.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReadabilityResult {
    /// Extracted article or page title.
    pub title: String,
    /// Extracted author/byline if detected.
    pub byline: Option<String>,
    /// Brief excerpt or leading paragraph summary.
    pub excerpt: Option<String>,
    /// Extracted main content formatted as clean, structured Markdown.
    pub content_markdown: String,
    /// Extracted main content as plain text.
    pub content_text: String,
    /// Extracted main content HTML subtree.
    pub content_html: String,
    /// Word count of primary content.
    pub word_count: usize,
    /// Estimated reading time in minutes (based on 200 words per minute).
    pub reading_time_mins: f32,
}

/// Configuration options for the readability extractor.
#[derive(Clone, Debug)]
pub struct ReadabilityOptions {
    /// Minimum character length for candidate paragraphs.
    pub min_paragraph_len: usize,
    /// Maximum link density permitted for content candidates before severe penalty.
    pub max_link_density: f64,
}

impl Default for ReadabilityOptions {
    fn default() -> Self {
        Self {
            min_paragraph_len: 25,
            max_link_density: 0.5,
        }
    }
}

/// Extracts primary readability content from a DOM tree.
pub fn extract_readability(dom: &DomTree, options: &ReadabilityOptions) -> ReadabilityResult {
    let mut title = String::new();
    if let Ok(Some(t_id)) = dom.query_selector("title") {
        title = dom.text_content(t_id).trim().to_string();
    }
    if title.is_empty() {
        if let Ok(Some(h1_id)) = dom.query_selector("h1") {
            title = dom.text_content(h1_id).trim().to_string();
        }
    }

    // Identify candidate container nodes
    let candidate_selectors = ["article", "main", "[role='main']", ".content", "#content", ".post", ".article", "section", "body"];
    let mut best_node: Option<NodeId> = None;
    let mut best_score: f64 = -1000.0;

    for selector in candidate_selectors {
        if let Ok(nodes) = dom.query_selector_all(selector) {
            for node_id in nodes {
                let score = score_candidate(dom, node_id, options);
                if score > best_score {
                    best_score = score;
                    best_node = Some(node_id);
                }
            }
        }
    }

    // Fall back to <body> or document root if no good candidate emerged
    let target_node = best_node
        .or_else(|| dom.query_selector("body").ok().flatten())
        .unwrap_or_else(|| dom.document());

    let mut markdown = String::new();
    let mut excerpt = None;
    let mut seen_text_chunks = HashSet::new();

    render_readability_markdown(dom, target_node, &mut markdown, &mut excerpt, &mut seen_text_chunks);

    let clean_md = markdown.trim().to_string();
    let clean_text = clean_md
        .lines()
        .filter(|line| !line.starts_with('#') || line.starts_with("# "))
        .map(|l| l.trim_start_matches(|c| c == '#' || c == '*' || c == '-' || c == '`' || c == ' '))
        .collect::<Vec<_>>()
        .join("\n");

    let words: Vec<&str> = clean_text.split_whitespace().collect();
    let word_count = words.len();
    let reading_time_mins = (word_count as f32 / 200.0).max(0.1);

    let content_html = dom.outer_html(target_node);

    ReadabilityResult {
        title,
        byline: None,
        excerpt,
        content_markdown: clean_md,
        content_text: clean_text,
        content_html,
        word_count,
        reading_time_mins,
    }
}

/// Evaluates a candidate DOM container node and calculates its quality score.
fn score_candidate(dom: &DomTree, node_id: NodeId, options: &ReadabilityOptions) -> f64 {
    let (tag, class, id) = dom
        .with_node(node_id, |n| {
            let tag = n.as_element().map(|name| name.local.to_string()).unwrap_or_default();
            let class = n.get_attribute("class").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let id = n.get_attribute("id").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            (tag, class, id)
        })
        .unwrap_or_default();

    if tag.is_empty() {
        return -1000.0;
    }

    let mut score = 0.0;

    // 1. Tag weight
    match tag.as_str() {
        "article" => score += 35.0,
        "main" => score += 30.0,
        "section" => score += 15.0,
        "div" => score += 5.0,
        "body" => score += 0.0,
        _ => score -= 10.0,
    }

    // 2. Class and ID heuristics
    let positive_patterns = [
        "article", "content", "post", "body", "entry", "main", "story", "text", "blog",
        "markdown", "prose", "readable",
    ];
    let negative_patterns = [
        "comment", "footer", "header", "menu", "nav", "sidebar", "sponsor", "widget",
        "banner", "promo", "share", "social", "cookie", "advert", "popup", "modal",
        "newsletter", "disclaimer", "breadcrumb",
    ];

    for pat in positive_patterns {
        if class.contains(pat) { score += 25.0; }
        if id.contains(pat) { score += 25.0; }
    }

    for pat in negative_patterns {
        if class.contains(pat) { score -= 30.0; }
        if id.contains(pat) { score -= 30.0; }
    }

    // 3. Paragraph count and text volume
    let text = dom.text_content(node_id);
    let total_len = text.trim().len();
    score += (total_len as f64) * 0.1;

    // Count paragraphs
    let mut p_count = 0;
    if let Ok(paragraphs) = dom.query_selector_all_from(node_id, "p") {
        for p_id in paragraphs {
            let p_len = dom.text_content(p_id).trim().len();
            if p_len >= options.min_paragraph_len {
                p_count += 1;
                score += 10.0;
            }
        }
    }

    // 4. Link density penalty
    let mut link_len = 0;
    if let Ok(links) = dom.query_selector_all_from(node_id, "a") {
        for a_id in links {
            link_len += dom.text_content(a_id).trim().len();
        }
    }

    if total_len > 0 {
        let link_density = link_len as f64 / total_len as f64;
        if link_density > options.max_link_density {
            score -= (link_density - options.max_link_density) * 150.0;
        }
    }

    if p_count == 0 && total_len < 100 {
        score -= 50.0;
    }

    score
}

/// Recursively formats the selected DOM subtree into readable Markdown.
fn render_readability_markdown(
    dom: &DomTree,
    node_id: NodeId,
    out: &mut String,
    excerpt: &mut Option<String>,
    seen: &mut HashSet<String>,
) {
    let (tag, attrs, is_element) = dom
        .with_node(node_id, |n| match &n.data {
            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.to_string().to_ascii_lowercase();
                let mut map = std::collections::HashMap::new();
                for a in attrs {
                    map.insert(a.qualified_name(), a.value.clone());
                }
                (tag, map, true)
            }
            NodeData::Text { contents } => (contents.clone(), std::collections::HashMap::new(), false),
            _ => (String::new(), std::collections::HashMap::new(), false),
        })
        .unwrap_or((String::new(), std::collections::HashMap::new(), false));

    if !is_element {
        let trimmed = tag.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push(' ');
        }
        return;
    }

    // Skip unwanted elements
    let tag_str = tag.as_str();
    if matches!(
        tag_str,
        "script" | "style" | "noscript" | "svg" | "iframe" | "header" | "footer" | "nav"
            | "aside" | "form"
    ) {
        return;
    }

    let class = attrs.get("class").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let id = attrs.get("id").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    if class.contains("ad-") || class.contains("banner") || class.contains("cookie") || id.contains("cookie") {
        return;
    }

    match tag_str {
        "h1" => {
            out.push_str("\n\n# ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
            out.push_str("\n\n");
        }
        "h2" => {
            out.push_str("\n\n## ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
            out.push_str("\n\n");
        }
        "h3" => {
            out.push_str("\n\n### ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
            out.push_str("\n\n");
        }
        "h4" | "h5" | "h6" => {
            out.push_str("\n\n#### ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
            out.push_str("\n\n");
        }
        "p" => {
            let text = dom.text_content(node_id).trim().to_string();
            if !text.is_empty() && !seen.contains(&text) {
                seen.insert(text.clone());
                if excerpt.is_none() && text.len() > 40 {
                    *excerpt = Some(text);
                }
                out.push_str("\n\n");
                for child in dom.children(node_id) {
                    render_readability_markdown(dom, child, out, excerpt, seen);
                }
                out.push_str("\n\n");
            }
        }
        "blockquote" => {
            out.push_str("\n\n> ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
            out.push_str("\n\n");
        }
        "li" => {
            out.push_str("\n- ");
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
        }
        "pre" | "code" => {
            let code_text = dom.text_content(node_id);
            if tag_str == "pre" {
                out.push_str("\n\n```\n");
                out.push_str(code_text.trim());
                out.push_str("\n```\n\n");
            } else {
                out.push('`');
                out.push_str(code_text.trim());
                out.push('`');
            }
        }
        _ => {
            for child in dom.children(node_id) {
                render_readability_markdown(dom, child, out, excerpt, seen);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obscura_dom::parse_html;

    #[test]
    fn test_extract_readability_article() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Exciting Discovery in AI</title></head>
            <body>
                <nav class="navbar"><a href="/">Home</a><a href="/news">News</a></nav>
                <aside class="sidebar"><h3>Ads</h3><p>Buy our sponsored widget!</p></aside>
                <article class="main-content">
                    <h1>Exciting Discovery in AI</h1>
                    <p class="intro">Researchers have developed a groundbreaking method for agentic web navigation.</p>
                    <p>The system leverages pure-Rust DOM parsing combined with lightweight HTTP retrieval to cut latency down to sub-20ms.</p>
                    <h2>Key Architecture Insights</h2>
                    <p>By bypassing heavy V8 isolates for static informational pages, energy and compute costs drop by over 70 percent.</p>
                </article>
                <footer class="footer"><p>Copyright 2026. All rights reserved.</p></footer>
            </body>
            </html>
        "#;
        let dom = parse_html(html);
        let result = extract_readability(&dom, &ReadabilityOptions::default());

        assert_eq!(result.title, "Exciting Discovery in AI");
        assert!(result.content_markdown.contains("# Exciting Discovery in AI"));
        assert!(result.content_markdown.contains("Researchers have developed a groundbreaking method"));
        assert!(result.content_markdown.contains("## Key Architecture Insights"));
        assert!(result.content_markdown.contains("By bypassing heavy V8 isolates"));
        // Navigation and footer should be excluded
        assert!(!result.content_markdown.contains("Buy our sponsored widget"));
        assert!(!result.content_markdown.contains("Copyright 2026"));
        assert!(result.word_count > 30);
    }
}
