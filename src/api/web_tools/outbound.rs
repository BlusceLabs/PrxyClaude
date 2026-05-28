use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use url::Url;

use crate::api::web_tools::constants;
use crate::api::web_tools::egress::{
    get_validated_addrs_for_egress, WebFetchEgressPolicy, WebFetchEgressViolation,
};
use crate::api::web_tools::parsers::{parse_search_results, HtmlTextExtractor};

pub fn web_tool_client_error_summary(tool_name: &str, verbose: bool) -> String {
    if verbose {
        format!("{tool_name} failed")
    } else {
        "Web tool request failed.".to_string()
    }
}

pub async fn run_web_search(query: &str) -> Result<Vec<HashMap<String, String>>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs_f64(constants::REQUEST_TIMEOUT_S))
        .user_agent("Mozilla/5.0 compatible; PxyClaude/2.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get("https://lite.duckduckgo.com/lite/")
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|e| format!("Search request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Search returned status {status}"));
    }

    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let text = String::from_utf8_lossy(&body_bytes).to_string();
    let results = parse_search_results(&text);
    let capped: Vec<_> = results
        .into_iter()
        .take(constants::MAX_SEARCH_RESULTS)
        .collect();
    Ok(capped)
}

pub async fn run_web_fetch(
    url: &str,
    egress: &WebFetchEgressPolicy,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut current_url = url.to_string();
    let mut redirect_hops = 0;

    loop {
        let _addrs = get_validated_addrs_for_egress(&current_url, egress)?;
        let parsed = Url::parse(&current_url)?;
        let host = parsed.host_str().unwrap_or("").to_string();

        let local_ip: Option<IpAddr> = None;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs_f64(constants::REQUEST_TIMEOUT_S))
            .user_agent("Mozilla/5.0 compatible; PxyClaude/2.0")
            .local_address(local_ip)
            .build()?;

        let response = client
            .get(&current_url)
            .header("Host", &host)
            .send()
            .await?;

        let status = response.status();

        if constants::web_fetch_redirect_statuses().contains(&status.as_u16()) {
            if redirect_hops >= constants::MAX_WEB_FETCH_REDIRECTS {
                return Err(Box::new(WebFetchEgressViolation::MaxRedirects(
                    constants::MAX_WEB_FETCH_REDIRECTS,
                )));
            }

            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            match location {
                Some(loc) if !loc.is_empty() => {
                    let base = Url::parse(&current_url)?;
                    current_url = base.join(&loc)?.to_string();
                    redirect_hops += 1;
                    continue;
                }
                _ => {
                    return Err(Box::new(WebFetchEgressViolation::MissingLocation));
                }
            }
        }

        response.error_for_status_ref()?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();

        let final_url = response.url().to_string();
        let body_bytes = response.bytes().await?;
        let body_len = body_bytes
            .len()
            .min(constants::MAX_WEB_FETCH_RESPONSE_BYTES);
        let text = String::from_utf8_lossy(&body_bytes[..body_len]).to_string();

        let (title, data) = if content_type.to_lowercase().contains("html") {
            let mut extractor = HtmlTextExtractor::new();
            extractor.extract(&text);
            (
                if extractor.title.is_empty() {
                    final_url.clone()
                } else {
                    extractor.title
                },
                extractor.text,
            )
        } else {
            (final_url.clone(), text.clone())
        };

        let data_capped: String = data.chars().take(constants::MAX_FETCH_CHARS).collect();

        return Ok(HashMap::from([
            ("url".to_string(), final_url),
            ("title".to_string(), title),
            ("media_type".to_string(), "text/plain".to_string()),
            ("data".to_string(), data_capped),
        ]));
    }
}
