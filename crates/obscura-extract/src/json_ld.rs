//! Pure-Rust JSON-LD and Schema.org structured data extractor.

use obscura_dom::DomTree;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Strongly normalized article entity from Schema.org JSON-LD.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleEntity {
    pub headline: Option<String>,
    pub author: Option<String>,
    pub date_published: Option<String>,
    pub date_modified: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
}

/// Strongly normalized product entity from Schema.org JSON-LD.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProductEntity {
    pub name: Option<String>,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub in_stock: Option<bool>,
}

/// Normalized FAQ item from Schema.org JSON-LD FAQPage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaqItem {
    pub question: String,
    pub answer: String,
}

/// Comprehensive structured data payload extracted from JSON-LD blocks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StructuredDataResult {
    /// All raw parsed JSON-LD objects.
    pub raw_json_ld: Vec<Value>,
    /// Normalized article entities.
    pub articles: Vec<ArticleEntity>,
    /// Normalized product entities.
    pub products: Vec<ProductEntity>,
    /// Normalized FAQ items.
    pub faq_items: Vec<FaqItem>,
}

/// Extracts all JSON-LD blocks and normalizes common Schema.org structures.
pub fn extract_structured_data(dom: &DomTree) -> StructuredDataResult {
    let mut result = StructuredDataResult::default();

    if let Ok(scripts) = dom.query_selector_all("script[type='application/ld+json']") {
        for script_id in scripts {
            let text = dom.text_content(script_id);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                process_json_ld_value(&val, &mut result);
            }
        }
    }

    result
}

fn process_json_ld_value(val: &Value, result: &mut StructuredDataResult) {
    match val {
        Value::Array(arr) => {
            for item in arr {
                process_json_ld_value(item, result);
            }
        }
        Value::Object(map) => {
            // Check for @graph wrapper
            if let Some(graph) = map.get("@graph").and_then(Value::as_array) {
                for item in graph {
                    process_json_ld_value(item, result);
                }
                return;
            }

            result.raw_json_ld.push(val.clone());

            let type_str = map
                .get("@type")
                .and_then(|t| t.as_str().or_else(|| t.as_array().and_then(|a| a.first().and_then(Value::as_str))))
                .unwrap_or_default()
                .to_ascii_lowercase();

            match type_str.as_str() {
                "article" | "newsarticle" | "blogposting" | "techarticle" => {
                    let mut article = ArticleEntity::default();
                    article.headline = map.get("headline").and_then(Value::as_str).map(ToString::to_string);
                    article.description = map.get("description").and_then(Value::as_str).map(ToString::to_string);
                    article.date_published = map.get("datePublished").and_then(Value::as_str).map(ToString::to_string);
                    article.date_modified = map.get("dateModified").and_then(Value::as_str).map(ToString::to_string);

                    if let Some(author) = map.get("author") {
                        article.author = if let Some(s) = author.as_str() {
                            Some(s.to_string())
                        } else if let Some(name) = author.get("name").and_then(Value::as_str) {
                            Some(name.to_string())
                        } else {
                            None
                        };
                    }

                    if let Some(pub_obj) = map.get("publisher") {
                        article.publisher = pub_obj.get("name").and_then(Value::as_str).map(ToString::to_string);
                    }

                    result.articles.push(article);
                }
                "product" => {
                    let mut product = ProductEntity::default();
                    product.name = map.get("name").and_then(Value::as_str).map(ToString::to_string);
                    product.description = map.get("description").and_then(Value::as_str).map(ToString::to_string);

                    if let Some(brand) = map.get("brand") {
                        product.brand = brand.as_str().or_else(|| brand.get("name").and_then(Value::as_str)).map(ToString::to_string);
                    }

                    if let Some(offers) = map.get("offers") {
                        let offer_obj = if let Some(arr) = offers.as_array() { arr.first() } else { Some(offers) };
                        if let Some(off) = offer_obj {
                            if let Some(price_num) = off.get("price").and_then(Value::as_f64) {
                                product.price = Some(price_num);
                            } else if let Some(price_str) = off.get("price").and_then(Value::as_str) {
                                product.price = price_str.parse().ok();
                            }
                            product.currency = off.get("priceCurrency").and_then(Value::as_str).map(ToString::to_string);
                            if let Some(avail) = off.get("availability").and_then(Value::as_str) {
                                product.in_stock = Some(avail.to_ascii_lowercase().contains("instock"));
                            }
                        }
                    }

                    result.products.push(product);
                }
                "faqpage" => {
                    if let Some(main_entity) = map.get("mainEntity").and_then(Value::as_array) {
                        for q_item in main_entity {
                            let question = q_item.get("name").and_then(Value::as_str).unwrap_or_default();
                            let answer = q_item
                                .get("acceptedAnswer")
                                .and_then(|a| a.get("text").and_then(Value::as_str))
                                .unwrap_or_default();

                            if !question.is_empty() && !answer.is_empty() {
                                result.faq_items.push(FaqItem {
                                    question: question.to_string(),
                                    answer: answer.to_string(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obscura_dom::parse_html;

    #[test]
    fn test_extract_json_ld_article_and_faq() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "NewsArticle",
                    "headline": "Quantum Computing Breakthrough",
                    "datePublished": "2026-11-18T10:00:00Z",
                    "author": { "@type": "Person", "name": "Dr. Sarah Connor" },
                    "publisher": { "@type": "Organization", "name": "TechChronicle" }
                }
                </script>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "FAQPage",
                    "mainEntity": [
                        {
                            "@type": "Question",
                            "name": "What is Obscura?",
                            "acceptedAnswer": {
                                "@type": "Answer",
                                "text": "Obscura is high-performance web intelligence infrastructure for AI agents."
                            }
                        }
                    ]
                }
                </script>
            </head>
            <body><h1>Content</h1></body>
            </html>
        "#;
        let dom = parse_html(html);
        let structured = extract_structured_data(&dom);

        assert_eq!(structured.raw_json_ld.len(), 2);
        assert_eq!(structured.articles.len(), 1);
        assert_eq!(structured.articles[0].headline.as_deref(), Some("Quantum Computing Breakthrough"));
        assert_eq!(structured.articles[0].author.as_deref(), Some("Dr. Sarah Connor"));
        assert_eq!(structured.articles[0].publisher.as_deref(), Some("TechChronicle"));

        assert_eq!(structured.faq_items.len(), 1);
        assert_eq!(structured.faq_items[0].question, "What is Obscura?");
        assert_eq!(structured.faq_items[0].answer, "Obscura is high-performance web intelligence infrastructure for AI agents.");
    }
}
