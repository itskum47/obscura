//! Pure-Rust HTML table extraction and tabular structuring.

use obscura_dom::DomTree;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Extracted structured HTML table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedTable {
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl ExtractedTable {
    /// Formats the extracted table into a clean GitHub-flavored Markdown table.
    pub fn to_markdown(&self) -> String {
        if self.headers.is_empty() && self.rows.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        if let Some(ref cap) = self.caption {
            out.push_str(&format!("**Table: {}**\n\n", cap));
        }

        let col_count = self.headers.len().max(self.rows.iter().map(|r| r.len()).max().unwrap_or(0));
        if col_count == 0 {
            return String::new();
        }

        // Headers
        out.push('|');
        for i in 0..col_count {
            let h = self.headers.get(i).map(String::as_str).unwrap_or("");
            out.push_str(&format!(" {} |", h));
        }
        out.push('\n');

        // Separator
        out.push('|');
        for _ in 0..col_count {
            out.push_str(" --- |");
        }
        out.push('\n');

        // Rows
        for row in &self.rows {
            out.push('|');
            for i in 0..col_count {
                let cell = row.get(i).map(String::as_str).unwrap_or("").replace('|', "\\|").replace('\n', " ");
                out.push_str(&format!(" {} |", cell.trim()));
            }
            out.push('\n');
        }

        out
    }

    /// Converts table rows into an array of JSON objects keyed by header names.
    pub fn to_json(&self) -> Value {
        let mut list = Vec::new();
        for row in &self.rows {
            let mut map = serde_json::Map::new();
            for (i, cell) in row.iter().enumerate() {
                let key = self.headers.get(i).cloned().unwrap_or_else(|| format!("col_{}", i + 1));
                map.insert(key, json!(cell.trim()));
            }
            list.push(Value::Object(map));
        }
        Value::Array(list)
    }
}

/// Extracts all tables from a DOM tree.
pub fn extract_tables(dom: &DomTree) -> Vec<ExtractedTable> {
    let mut tables = Vec::new();

    if let Ok(table_nodes) = dom.query_selector_all("table") {
        for t_id in table_nodes {
            let mut extracted = ExtractedTable::default();

            // Caption
            if let Ok(Some(cap_id)) = dom.query_selector_from(t_id, "caption") {
                let text = dom.text_content(cap_id).trim().to_string();
                if !text.is_empty() {
                    extracted.caption = Some(text);
                }
            }

            // Headers from <th>
            if let Ok(th_nodes) = dom.query_selector_all_from(t_id, "thead th, th") {
                for th in th_nodes {
                    let text = dom.text_content(th).trim().to_string();
                    if !text.is_empty() {
                        extracted.headers.push(text);
                    }
                }
            }

            // Data rows
            if let Ok(tr_nodes) = dom.query_selector_all_from(t_id, "tr") {
                for tr in tr_nodes {
                    let mut row = Vec::new();
                    if let Ok(cells) = dom.query_selector_all_from(tr, "td") {
                        if cells.is_empty() {
                            continue;
                        }
                        for cell in cells {
                            row.push(dom.text_content(cell).trim().to_string());
                        }
                    }
                    if !row.is_empty() {
                        extracted.rows.push(row);
                    }
                }
            }

            // If no <th> tags existed, attempt to promote the first row to headers if multiple rows exist
            if extracted.headers.is_empty() && extracted.rows.len() > 1 {
                extracted.headers = extracted.rows.remove(0);
            }

            if !extracted.headers.is_empty() || !extracted.rows.is_empty() {
                tables.push(extracted);
            }
        }
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;
    use obscura_dom::parse_html;

    #[test]
    fn test_extract_table_to_markdown_and_json() {
        let html = r#"
            <table>
                <caption>Cloud Pricing Comparison</caption>
                <thead>
                    <tr><th>Provider</th><th>vCPU</th><th>RAM</th><th>Monthly Cost</th></tr>
                </thead>
                <tbody>
                    <tr><td>AWS</td><td>2</td><td>4GB</td><td>$28.00</td></tr>
                    <tr><td>GCP</td><td>2</td><td>4GB</td><td>$26.50</td></tr>
                </tbody>
            </table>
        "#;
        let dom = parse_html(html);
        let tables = extract_tables(&dom);

        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.caption.as_deref(), Some("Cloud Pricing Comparison"));
        assert_eq!(t.headers, vec!["Provider", "vCPU", "RAM", "Monthly Cost"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec!["AWS", "2", "4GB", "$28.00"]);

        let md = t.to_markdown();
        assert!(md.contains("**Table: Cloud Pricing Comparison**"));
        assert!(md.contains("| Provider | vCPU | RAM | Monthly Cost |"));
        assert!(md.contains("| AWS | 2 | 4GB | $28.00 |"));

        let json_val = t.to_json();
        let arr = json_val.as_array().expect("array of objects");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["Provider"], "AWS");
        assert_eq!(arr[0]["Monthly Cost"], "$28.00");
    }
}
