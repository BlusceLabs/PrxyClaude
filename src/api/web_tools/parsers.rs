use regex::Regex;
use std::collections::HashMap;

/// Extract the first URL from text.
pub fn extract_url(text: &str) -> String {
    let re = Regex::new(r"https?://\S+").unwrap();
    if let Some(m) = re.find(text) {
        let url = m.as_str();
        url.trim_end_matches(").,]").to_string()
    } else {
        text.trim().to_string()
    }
}

/// Extract the query after "query:" marker.
pub fn extract_query(text: &str) -> String {
    let re = Regex::new(r"(?i)query:\s*(.+)").unwrap();
    if let Some(caps) = re.captures(text) {
        caps.get(1)
            .map(|m| m.as_str().trim().trim_matches('\'').trim_matches('"').to_string())
            .unwrap_or_else(|| text.trim().to_string())
    } else {
        text.trim().to_string()
    }
}

/// Extract text from content value (string, list, or dict).
pub fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                }
            }
            parts.join("\n")
        }
        _ => content.to_string(),
    }
}

/// Parse DuckDuckGo lite HTML to extract search results.
pub fn parse_search_results(html: &str) -> Vec<HashMap<String, String>> {
    let mut results = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    let href_re = Regex::new(r#"<a[^>]*href="([^"]*)"[^>]*>"#).unwrap();
    let uddg_re = Regex::new(r"uddg=([^&]+)").unwrap();
    let end_tag_re = Regex::new(r"</a>").unwrap();

    let mut pos = 0;
    while let Some(href_cap) = href_re.captures(&html[pos..]) {
        let href = href_cap.get(1).unwrap().as_str();
        let full_match_end = pos + href_cap.get(0).unwrap().end();

        if let Some(uddg_cap) = uddg_re.captures(href) {
            let encoded_url = uddg_cap.get(1).unwrap().as_str();
            if let Ok(decoded) = urlencoding::decode(encoded_url) {
                let url = decoded.to_string();

                // Find </a> after the <a> tag to extract title text
                if let Some(end_pos) = end_tag_re.find(&html[full_match_end..]) {
                    let title_html = &html[full_match_end..full_match_end + end_pos.start()];
                    let title = strip_html_tags(title_html);
                    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");

                    if !title.is_empty() && !seen_urls.contains(&url) {
                        seen_urls.insert(url.clone());
                        results.push(HashMap::from([
                            ("title".to_string(), html_escape::decode_html_entities(&title).to_string()),
                            ("url".to_string(), url),
                        ]));
                    }
                }
            }
        }

        pos = full_match_end;
    }

    results
}

/// Strip HTML tags, extracting visible text.
pub fn strip_html_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            in_tag = true;
            let tag_start = i;
            // Find tag name
            let mut tag_name = String::new();
            let mut j = i + 1;
            while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '>' {
                tag_name.push(chars[j].to_ascii_lowercase());
                j += 1;
            }
            if tag_name == "script" || tag_name == "/script" {
                in_script = tag_name != "/script";
            }
            if tag_name == "style" || tag_name == "/style" {
                in_style = tag_name != "/style";
            }
            if tag_name == "title" {
                // Parse title content
                if let Some(end) = html[tag_start..].find("</title>") {
                    let content = &html[tag_start + tag_name.len() + 1..tag_start + end];
                    if !in_script && !in_style {
                        let clean = content.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !clean.is_empty() {
                            text.push_str(&clean);
                            text.push(' ');
                        }
                    }
                    i = tag_start + end + "</title>".len();
                    in_tag = false;
                    continue;
                }
            }
            i += 1;
            while i < chars.len() && chars[i] != '>' {
                if in_script || in_style {
                    // skip everything inside script/style
                }
                i += 1;
            }
            if i < chars.len() {
                in_tag = false;
                i += 1;
            }
            continue;
        }

        if !in_tag && !in_script && !in_style {
            text.push(chars[i]);
        }
        i += 1;
    }

    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Simplified HTML to text extraction (for web_fetch previews).
pub struct HtmlTextExtractor {
    pub title: String,
    pub text: String,
}

impl HtmlTextExtractor {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
        }
    }

    pub fn extract(&mut self, html: &str) {
        // Try to find <title> tag
        if let Some(title_start) = html.find("<title>") {
            let after_title = &html[title_start + "<title>".len()..];
            if let Some(title_end) = after_title.find("</title>") {
                self.title = after_title[..title_end]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
            }
        }

        self.text = strip_html_tags(html);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_url() {
        let result = extract_url("Check https://example.com/path?q=1 for details");
        assert_eq!(result, "https://example.com/path?q=1");
    }

    #[test]
    fn test_extract_query() {
        let result = extract_query("query: what is the weather");
        assert_eq!(result, "what is the weather");
    }

    #[test]
    fn test_extract_query_no_marker() {
        let result = extract_query("just some text");
        assert_eq!(result, "just some text");
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<html><body><p>Hello <b>world</b></p></body></html>";
        let text = strip_html_tags(html);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_html_text_extractor() {
        let mut extractor = HtmlTextExtractor::new();
        extractor.extract("<html><head><title>Test Page</title></head><body><p>Content</p></body></html>");
        assert_eq!(extractor.title, "Test Page");
        assert!(extractor.text.contains("Content"));
    }
}
