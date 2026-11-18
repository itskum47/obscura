//! Pure-Rust XML sitemap and RSS/Atom feed parser.

use roxmltree::Document;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FeedError {
    #[error("XML parsing error: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("Unsupported or unrecognized feed format")]
    UnrecognizedFormat,
}

/// A discovered URL entry within an XML sitemap.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SitemapEntry {
    pub loc: String,
    pub lastmod: Option<String>,
    pub changefreq: Option<String>,
    pub priority: Option<f32>,
    pub is_sitemap_index: bool,
}

/// An article or update item extracted from an RSS or Atom feed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedEntry {
    pub title: Option<String>,
    pub link: String,
    pub summary: Option<String>,
    pub published: Option<String>,
    pub author: Option<String>,
}

/// Parses an XML sitemap or sitemap index into structured URL entries.
pub fn parse_sitemap(xml: &str) -> Result<Vec<SitemapEntry>, FeedError> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    let mut entries = Vec::new();

    let tag = root.tag_name().name();
    if tag == "urlset" {
        for node in root.children() {
            if node.tag_name().name() == "url" {
                let mut entry = SitemapEntry::default();
                for child in node.children() {
                    let child_tag = child.tag_name().name();
                    let text = child.text().map(str::trim).unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    match child_tag {
                        "loc" => entry.loc = text.to_string(),
                        "lastmod" => entry.lastmod = Some(text.to_string()),
                        "changefreq" => entry.changefreq = Some(text.to_string()),
                        "priority" => entry.priority = text.parse::<f32>().ok(),
                        _ => {}
                    }
                }
                if !entry.loc.is_empty() {
                    entries.push(entry);
                }
            }
        }
    } else if tag == "sitemapindex" {
        for node in root.children() {
            if node.tag_name().name() == "sitemap" {
                let mut entry = SitemapEntry {
                    is_sitemap_index: true,
                    ..Default::default()
                };
                for child in node.children() {
                    let child_tag = child.tag_name().name();
                    let text = child.text().map(str::trim).unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    match child_tag {
                        "loc" => entry.loc = text.to_string(),
                        "lastmod" => entry.lastmod = Some(text.to_string()),
                        _ => {}
                    }
                }
                if !entry.loc.is_empty() {
                    entries.push(entry);
                }
            }
        }
    } else {
        return Err(FeedError::UnrecognizedFormat);
    }

    Ok(entries)
}

/// Parses an RSS 2.0 or Atom feed into structured feed entries.
pub fn parse_feed(xml: &str) -> Result<Vec<FeedEntry>, FeedError> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    let tag = root.tag_name().name();
    let mut entries = Vec::new();

    if tag == "rss" {
        // RSS 2.0: /rss/channel/item
        if let Some(channel) = root.children().find(|n| n.tag_name().name() == "channel") {
            for item in channel.children().filter(|n| n.tag_name().name() == "item") {
                let mut entry = FeedEntry::default();
                for child in item.children() {
                    let child_tag = child.tag_name().name();
                    let text = child.text().map(str::trim).unwrap_or_default();
                    match child_tag {
                        "title" if !text.is_empty() => entry.title = Some(text.to_string()),
                        "link" if !text.is_empty() => entry.link = text.to_string(),
                        "description" if !text.is_empty() => entry.summary = Some(text.to_string()),
                        "pubDate" if !text.is_empty() => entry.published = Some(text.to_string()),
                        "author" | "dc:creator" if !text.is_empty() => entry.author = Some(text.to_string()),
                        _ => {}
                    }
                }
                if !entry.link.is_empty() {
                    entries.push(entry);
                }
            }
        }
    } else if tag == "feed" {
        // Atom: /feed/entry
        for item in root.children().filter(|n| n.tag_name().name() == "entry") {
            let mut entry = FeedEntry::default();
            for child in item.children() {
                let child_tag = child.tag_name().name();
                match child_tag {
                    "title" => {
                        let text = child.text().map(str::trim).unwrap_or_default();
                        if !text.is_empty() {
                            entry.title = Some(text.to_string());
                        }
                    }
                    "link" => {
                        if let Some(href) = child.attribute("href") {
                            entry.link = href.trim().to_string();
                        } else {
                            let text = child.text().map(str::trim).unwrap_or_default();
                            if !text.is_empty() {
                                entry.link = text.to_string();
                            }
                        }
                    }
                    "summary" | "content" => {
                        let text = child.text().map(str::trim).unwrap_or_default();
                        if !text.is_empty() && entry.summary.is_none() {
                            entry.summary = Some(text.to_string());
                        }
                    }
                    "updated" | "published" => {
                        let text = child.text().map(str::trim).unwrap_or_default();
                        if !text.is_empty() && entry.published.is_none() {
                            entry.published = Some(text.to_string());
                        }
                    }
                    "author" => {
                        if let Some(name_node) = child.children().find(|n| n.tag_name().name() == "name") {
                            let text = name_node.text().map(str::trim).unwrap_or_default();
                            if !text.is_empty() {
                                entry.author = Some(text.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !entry.link.is_empty() {
                entries.push(entry);
            }
        }
    } else {
        return Err(FeedError::UnrecognizedFormat);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url>
                <loc>https://example.com/page1</loc>
                <lastmod>2026-11-18</lastmod>
                <changefreq>daily</changefreq>
                <priority>0.9</priority>
            </url>
            <url>
                <loc>https://example.com/page2</loc>
            </url>
        </urlset>"#;

        let entries = parse_sitemap(xml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].loc, "https://example.com/page1");
        assert_eq!(entries[0].lastmod.as_deref(), Some("2026-11-18"));
        assert_eq!(entries[0].priority, Some(0.9));
        assert!(!entries[0].is_sitemap_index);
        assert_eq!(entries[1].loc, "https://example.com/page2");
    }

    #[test]
    fn test_parse_rss_and_atom() {
        let rss_xml = r#"<?xml version="1.0"?>
        <rss version="2.0">
            <channel>
                <title>Tech News</title>
                <item>
                    <title>AI Agent Evolution</title>
                    <link>https://example.com/ai-agents</link>
                    <description>Detailed insights on autonomous web search.</description>
                    <pubDate>Wed, 18 Nov 2026 12:00:00 GMT</pubDate>
                </item>
            </channel>
        </rss>"#;
        let rss_entries = parse_feed(rss_xml).unwrap();
        assert_eq!(rss_entries.len(), 1);
        assert_eq!(rss_entries[0].title.as_deref(), Some("AI Agent Evolution"));
        assert_eq!(rss_entries[0].link, "https://example.com/ai-agents");

        let atom_xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <title>Example Feed</title>
            <entry>
                <title>Atom Post</title>
                <link href="https://example.com/atom-post"/>
                <summary>Atom summary content.</summary>
                <updated>2026-11-18T12:00:00Z</updated>
            </entry>
        </feed>"#;
        let atom_entries = parse_feed(atom_xml).unwrap();
        assert_eq!(atom_entries.len(), 1);
        assert_eq!(atom_entries[0].title.as_deref(), Some("Atom Post"));
        assert_eq!(atom_entries[0].link, "https://example.com/atom-post");
    }
}
