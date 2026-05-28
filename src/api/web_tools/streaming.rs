use std::sync::Arc;

use crate::api::web_tools::constants::MAX_FETCH_CHARS;
use crate::api::web_tools::egress::WebFetchEgressPolicy;
use crate::api::web_tools::outbound::{run_web_fetch, run_web_search, web_tool_client_error_summary};
use crate::api::web_tools::parsers::{extract_query, extract_url};
use crate::api::web_tools::request::{forced_server_tool_name, forced_tool_turn_text, has_tool_named};
use crate::core::anthropic::sse::format_sse_event;
use crate::core::anthropic::server_tool_sse::{
    SERVER_TOOL_USE, WEB_FETCH_TOOL_ERROR, WEB_FETCH_TOOL_RESULT, WEB_SEARCH_TOOL_RESULT,
    WEB_SEARCH_TOOL_RESULT_ERROR,
};
use crate::models::MessagesRequest;
use serde_json::Value;

fn search_summary(query: &str, results: &[std::collections::HashMap<String, String>]) -> String {
    if results.is_empty() {
        return format!("No web search results found for: {query}");
    }
    let mut lines = Vec::new();
    lines.push(format!("Search results for: {query}"));
    for (index, result) in results.iter().enumerate() {
        let title = result.get("title").map(|s| s.as_str()).unwrap_or("");
        let url = result.get("url").map(|s| s.as_str()).unwrap_or("");
        lines.push(format!("{}. {}\n{}", index + 1, title, url));
    }
    lines.join("\n\n")
}

pub async fn stream_web_server_tool_response(
    request: &MessagesRequest,
    input_tokens: i32,
    web_fetch_egress: Arc<WebFetchEgressPolicy>,
    verbose_client_errors: bool,
) -> Vec<String> {
    let tool_name = match forced_server_tool_name(request) {
        Some(n) => n,
        None => return Vec::new(),
    };

    if !has_tool_named(request, &tool_name) {
        return Vec::new();
    }

    let text = forced_tool_turn_text(request);
    let message_id = format!("msg_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let tool_id = format!("srvtoolu_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let usage_key = if tool_name == "web_search" {
        "web_search_requests"
    } else {
        "web_fetch_requests"
    };

    let tool_input = if tool_name == "web_search" {
        serde_json::json!({"query": extract_query(&text)})
    } else {
        serde_json::json!({"url": extract_url(&text)})
    };

    let mut events = Vec::new();

    events.push(format_sse_event(
        "message_start",
        &serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": request.model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": input_tokens, "output_tokens": 1},
            },
        }),
    ));

    events.push(format_sse_event(
        "content_block_start",
        &serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": SERVER_TOOL_USE,
                "id": tool_id,
                "name": tool_name,
                "input": tool_input,
            },
        }),
    ));

    events.push(format_sse_event(
        "content_block_stop",
        &serde_json::json!({"type": "content_block_stop", "index": 0}),
    ));

    let (result_block_type, result_content, summary): (&str, Value, String) = match tool_name.as_str() {
        "web_search" => {
            let query = tool_input.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
            match run_web_search(&query).await {
                Ok(results) => {
                    let content: Vec<Value> = results
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "type": "web_search_result",
                                "title": r.get("title").map(|s| s.as_str()).unwrap_or(""),
                                "url": r.get("url").map(|s| s.as_str()).unwrap_or(""),
                            })
                        })
                        .collect();
                    let summary = search_summary(&query, &results);
                    (WEB_SEARCH_TOOL_RESULT, serde_json::Value::Array(content), summary)
                }
                Err(e) => {
                    log_web_tool_failure("web_search", &e);
                    (
                        WEB_SEARCH_TOOL_RESULT,
                        serde_json::json!({
                            "type": WEB_SEARCH_TOOL_RESULT_ERROR,
                            "error_code": "unavailable",
                        }),
                        web_tool_client_error_summary("web_search", verbose_client_errors),
                    )
                }
            }
        }
        "web_fetch" => {
            let url = tool_input.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let egress = web_fetch_egress.clone();
            match run_web_fetch(&url, &egress).await {
                Ok(fetched) => {
                    let result_content = serde_json::json!({
                        "type": "web_fetch_result",
                        "url": fetched.get("url").map(|s| s.as_str()).unwrap_or(""),
                        "content": {
                            "type": "document",
                            "source": {
                                "type": "text",
                                "media_type": fetched.get("media_type").map(|s| s.as_str()).unwrap_or("text/plain"),
                                "data": fetched.get("data").map(|s| s.as_str()).unwrap_or(""),
                            },
                            "title": fetched.get("title").map(|s| s.as_str()).unwrap_or(""),
                            "citations": {"enabled": true},
                        },
                        "retrieved_at": chrono::Utc::now().to_rfc3339(),
                    });
                    let summary = fetched
                        .get("data")
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .chars()
                        .take(MAX_FETCH_CHARS)
                        .collect::<String>();
                    (WEB_FETCH_TOOL_RESULT, result_content, summary)
                }
                Err(e) => {
                    log_web_tool_failure("web_fetch", &e);
                    (
                        WEB_FETCH_TOOL_RESULT,
                        serde_json::json!({
                            "type": WEB_FETCH_TOOL_ERROR,
                            "error_code": "unavailable",
                        }),
                        web_tool_client_error_summary("web_fetch", verbose_client_errors),
                    )
                }
            }
        }
        _ => return Vec::new(),
    };

    let output_tokens = std::cmp::max(1, summary.chars().count() / 4) as i32;

    events.push(format_sse_event(
        "content_block_start",
        &serde_json::json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": {
                "type": result_block_type,
                "tool_use_id": tool_id,
                "content": result_content,
            },
        }),
    ));

    events.push(format_sse_event(
        "content_block_stop",
        &serde_json::json!({"type": "content_block_stop", "index": 1}),
    ));

    events.push(format_sse_event(
        "content_block_start",
        &serde_json::json!({
            "type": "content_block_start",
            "index": 2,
            "content_block": {"type": "text", "text": ""},
        }),
    ));

    events.push(format_sse_event(
        "content_block_delta",
        &serde_json::json!({
            "type": "content_block_delta",
            "index": 2,
            "delta": {"type": "text_delta", "text": summary},
        }),
    ));

    events.push(format_sse_event(
        "content_block_stop",
        &serde_json::json!({"type": "content_block_stop", "index": 2}),
    ));

    events.push(format_sse_event(
        "message_delta",
        &serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn", "stop_sequence": null},
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "server_tool_use": {usage_key: 1},
            },
        }),
    ));

    events.push(format_sse_event("message_stop", &serde_json::json!({"type": "message_stop"})));

    events
}

fn log_web_tool_failure(tool_name: &str, error: &dyn std::fmt::Display) {
    let exc_type = std::any::type_name_of_val(error);
    tracing::warn!("web_tool_failure tool={} exc_type={}", tool_name, exc_type);
}
