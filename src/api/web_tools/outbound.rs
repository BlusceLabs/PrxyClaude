use crate::web_tools::constants::*;
use crate::web_tools::egress::{validate_url_for_egress, WebFetchEgressPolicy};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

/// Web search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
}

/// Run web search using DuckDuckGo Lite
pub async fn run_web_search(query: &str) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_S as u64))
        .build()?;
    
    let params = [("q", query)];
    let response = client
        .get("https://lite.duckduckgo.com/lite/")
        .query(&params)
        .send()
        .await?;
    
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP error: {}", status).into());
    }
    
    let body = timeout(
        Duration::from_secs(REQUEST_TIMEOUT_S as u64),
        response.text()
    ).await??;
    
    let results = parse_search_results(&body)?;
    
    Ok(results.into_iter().take(MAX_SEARCH_RESULTS).collect())
}

/// Run web fetch with egress validation
pub async fn run_web_fetch(url: &str, egress_policy: &WebFetchEgressPolicy) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Validate URL before making request
    validate_url_for_egress(url, egress_policy)?;
    
    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_S as u64))
        .build()?;
    
    let response = client.get(url).send().await?;
    
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP error: {}", status).into());
    }
    
    let content_type = response.headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("text/plain")
        .to_string();
    
    let final_url = response.url().to_string();
    let body = timeout(
        Duration::from_secs_f64(REQUEST_TIMEOUT_S),
        response.text()
    ).await??;
    
    let title = if content_type.contains("html") {
        extract_html_title(&body)
    } else {
        final_url.clone()
    };
    
    let data = if content_type.contains("html") {
        extract_html_text(&body)
    } else {
        body
    };
    
    let data = data.chars().take(MAX_FETCH_CHARS).collect::<String>();
    
    Ok(serde_json::json!({
        "url": final_url,
        "title": title,
        "media_type": "text/plain",
        "data": data
    }))
}

/// Parse search results from DuckDuckGo Lite HTML
fn parse_search_results(html: &str) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    let mut results = Vec::new();
    
    // Simple HTML parsing for DuckDuckGo results
    let re = regex::Regex::new(r#"<a[^>]+href="([^"]*uddg=[^"]*)"[^>]*>(.*?)</a>"#)?;
    
    for cap in re.captures_iter(html) {
        let href_url = &cap[1];
        let title = &cap[2];
        
        // Extract the actual URL from uddg parameter
        if let Some(actual_url) = extract_url_from_uddg(href_url) {
            let title = clean_html(title);
            if !title.is_empty() && !results.iter().any(|r: &SearchResult| r.url == actual_url) {
                results.push(SearchResult {
                    title: title,
                    url: actual_url,
                });
            }
        }
    }
    
    Ok(results)
}

/// Extract URL from DuckDuckGo uddg parameter
fn extract_url_from_uddg(href: &str) -> Option<String> {
    let url = url::Url::parse(href).ok()?;
    let query = url.query_pairs().find(|(k, _)| k == "uddg")?.1;
    Some(urlencoding::decode(&query).ok()?.into_owned())
}

/// Extract title from HTML
fn extract_html_title(html: &str) -> String {
    let re = regex::Regex::new(r#"<title[^>]*>(.*?)</title>"#).unwrap();
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| clean_html(m.as_str()))
        .unwrap_or_default()
}

/// Extract text from HTML
fn extract_html_text(html: &str) -> String {
    let re = regex::Regex::new(r#"<[^>]+>"#).unwrap();
    let clean = re.replace_all(html, " ");
    clean.into_owned()
}

/// Clean HTML entities and whitespace
fn clean_html(text: &str) -> String {
    let re = regex::Regex::new(r#"&[^;]+;"#).unwrap();
    let decoded = re.replace_all(text, |caps: &regex::Captures| {
        html_escape::decode_html_entities(&caps[0]).to_string()
    });
    
    let re = regex::Regex::new(r"\s+").unwrap();
    re.replace_all(&decoded, " ").trim().to_string()
}

/// Log web tool failure
pub fn log_web_tool_failure(tool_name: &str, _error: &dyn std::error::Error, fetch_url: Option<&str>) {
    let exc_type = std::any::type_name::<dyn std::error::Error>();
    
    if let Some(url) = fetch_url {
        let host = extract_host_from_url(url);
        tracing::warn!(
            "web_tool_failure tool={} exc_type={} host={}",
            tool_name,
            exc_type,
            host
        );
    } else {
        tracing::warn!(
            "web_tool_failure tool={} exc_type={}",
            tool_name,
            exc_type
        );
    }
}

/// Extract host from URL
fn extract_host_from_url(url: &str) -> String {
    match url.parse::<url::Url>() {
        Ok(parsed) => parsed.host().map(|h| h.to_string()).unwrap_or_else(|| "".to_string()),
        Err(_) => "".to_string(),
    }
}

/// Get web tool error summary
pub fn get_web_tool_error_summary(tool_name: &str, error: &dyn std::error::Error, verbose: bool) -> String {
    if verbose {
        format!("{} failed: {}", tool_name, error)
    } else {
        "Web tool request failed.".to_string()
    }
}