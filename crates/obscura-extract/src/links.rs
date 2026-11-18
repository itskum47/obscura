//! Pure-Rust link extraction, normalization, and domain classification.

use obscura_dom::DomTree;
use serde::{Deserialize, Serialize};
use url::Url;

/// Structured hyperlink extracted from a document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedLink {
    /// Fully resolved absolute URL.
    pub url: String,
    /// Anchor text or title of the link.
    pub text: String,
    /// Relationship attribute (e.g. "nofollow", "noopener").
    pub rel: Option<String>,
    /// Whether the link points to an external domain relative to the document base.
    pub is_external: bool,
}

/// Extracts all hyperlinks from a DOM tree, resolving relative paths against `base_url`.
pub fn extract_links(dom: &DomTree, base_url: Option<&str>) -> Vec<ExtractedLink> {
    let mut links = Vec::new();
    let parsed_base = base_url.and_then(|b| Url::parse(b).ok());
    let base_host = parsed_base.as_ref().and_then(|u| u.host_str().map(ToString::to_string));

    if let Ok(a_nodes) = dom.query_selector_all("a[href]") {
        for a_id in a_nodes {
            let (raw_href, rel) = dom
                .with_node(a_id, |n| {
                    let href = n.get_attribute("href").map(ToString::to_string);
                    let rel = n.get_attribute("rel").map(ToString::to_string);
                    (href, rel)
                })
                .unwrap_or((None, None));

            let Some(raw_href) = raw_href else { continue };
            let trimmed_href = raw_href.trim();
            if trimmed_href.is_empty() || trimmed_href.starts_with('#') || trimmed_href.starts_with("javascript:") || trimmed_href.starts_with("mailto:") {
                continue;
            }

            let text = dom.text_content(a_id).trim().to_string();

            let (absolute_url, is_external) = if let Ok(u) = Url::parse(trimmed_href) {
                let is_ext = match (base_host.as_ref(), u.host_str()) {
                    (Some(bh), Some(uh)) => bh != uh,
                    _ => false,
                };
                (u.to_string(), is_ext)
            } else if let Some(ref base) = parsed_base {
                match base.join(trimmed_href) {
                    Ok(u) => (u.to_string(), false),
                    Err(_) => continue,
                }
            } else {
                continue;
            };

            links.push(ExtractedLink {
                url: absolute_url,
                text,
                rel,
                is_external,
            });
        }
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use obscura_dom::parse_html;

    #[test]
    fn test_extract_and_resolve_links() {
        let html = r##"
            <div>
                <a href="/docs/guide" rel="help">Getting Started</a>
                <a href="https://external.org/info" rel="nofollow">External Info</a>
                <a href="#section-2">Skip Link</a>
                <a href="javascript:void(0)">Do Nothing</a>
            </div>
        "##;
        let dom = parse_html(html);
        let links = extract_links(&dom, Some("https://example.com/start"));

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/docs/guide");
        assert_eq!(links[0].text, "Getting Started");
        assert!(!links[0].is_external);

        assert_eq!(links[1].url, "https://external.org/info");
        assert_eq!(links[1].text, "External Info");
        assert!(links[1].is_external);
        assert_eq!(links[1].rel.as_deref(), Some("nofollow"));
    }
}
