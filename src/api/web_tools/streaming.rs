use crate::models::{ContentOrBlocks, MessagesRequest, Role};
use crate::web_tools::constants::*;
use crate::web_tools::egress::WebFetchEgressPolicy;
use crate::web_tools::outbound::{run_web_search, run_web_fetch, SearchResult};
use crate::web_tools::request::{get_forced_server_tool_name, has_tool_named};
use futures::stream::Stream;
use futures::StreamExt;
use std::pin::Pin;
use serde_json::Value;

/// Web search result content block
pub struct WebSearchResultContent {
    pub tool_use_id: String,
    pub results: Vec<SearchResult>,
}

/// Web fetch result content block
pub struct WebFetchResultContent {
    pub tool_use_id: String,
    pub url: String,
    pub title: String,
    pub data: String,
}

/// Stream web server tool response
pub async fn stream_web_server_tool_response(
    request: &MessagesRequest,
    input_tokens: i32,
    web_fetch_egress: &WebFetchEgressPolicy,
    verbose_client_errors: bool,
) -> Box<dyn futures::stream::Stream<Item = Value> + Send + Unpin> {
    let tool_name = get_forced_server_tool_name(request);
    
    if tool_name.is_none() || !has_tool_named(request, tool_name.as_ref().unwrap()) {
        return Box::new(futures::stream::empty());
    }
    
    let tool_name = tool_name.unwrap();
    let text = forced_tool_turn_text(request);
    let message_id = format!("msg_{}", uuid::Uuid::new_v4());
    let tool_id = format!("srvtoolu_{}", uuid::Uuid::new_v4().to_string());
    let usage_key = if tool_name == "web_search" { "web_search_requests" } else { "web_fetch_requests" };
    
    let tool_input = if tool_name == "web_search" {
        let query = extract_query(&text);
        serde_json::json!({"query": query})
    } else {
        let url = extract_url(&text);
        serde_json::json!({"url": url})
    };
    
    // Stream SSE events
    Box::new(
        futures::stream::iter([
            create_message_start_event(&message_id, &request.model, input_tokens),
            create_content_block_start_event(0, "tool_use", &tool_id, &tool_name, &tool_input),
            create_content_block_stop_event(0),
        ])
        .chain(stream_tool_result(&tool_name, &tool_id, &tool_input, web_fetch_egress, verbose_client_errors).await)
        .chain(futures::stream::iter([
            create_content_block_start_event(1, &format!("{}_result", tool_name), &tool_id, &tool_name, &serde_json::json!({})),
            create_content_block_stop_event(1),
            create_text_content_block_start_event(2),
            create_text_delta_event("Processing request..."),
            create_content_block_stop_event(2),
            create_message_delta_event("end_turn", input_tokens, 1, &usage_key),
            create_message_stop_event(),
        ]))
    )
}

fn forced_tool_turn_text(request: &MessagesRequest) -> String {
    for message in request.messages.iter().rev() {
        if message.role == Role::User {
            match &message.content {
                ContentOrBlocks::String(s) => return s.clone(),
                ContentOrBlocks::Blocks(blocks) => {
                    let mut text = String::new();
                    for block in blocks {
                        if let Some(text_part) = block.as_text() {
                            text.push_str(text_part);
                        }
                    }
                    return text;
                },
            }
        }
    }
    "".to_string()
}

/// Extract query from text
fn extract_query(text: &str) -> String {
    let re = regex::Regex::new(r"query:\s*(.+)").unwrap();
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().trim_matches('"').trim_matches('\''))
        .unwrap_or_else(|| text.trim())
        .to_string()
}

/// Extract URL from text
fn extract_url(text: &str) -> String {
    let re = regex::Regex::new(r"https?://\S+").unwrap();
    re.captures(text)
        .and_then(|c| c.get(0))
        .map(|m| {
            let url = m.as_str();
            // Remove trailing punctuation characters
            url.trim_end_matches([')', ',', '.'])
        })
        .unwrap_or_else(|| text.trim())
        .to_string()
}

/// Create message start event
fn create_message_start_event(message_id: &str, model: &str, input_tokens: i32) -> Value {
    serde_json::json!({
        "type": "message_start",
        "message": {
            "id": message_id,
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": 1
            }
        }
    })
}

/// Create content block start event
fn create_content_block_start_event(index: usize, content_type: &str, tool_id: &str, tool_name: &str, input: &Value) -> Value {
    serde_json::json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": content_type,
            "id": tool_id,
            "name": tool_name,
            "input": input
        }
    })
}

/// Create text content block start event
fn create_text_content_block_start_event(index: usize) -> Value {
    serde_json::json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "text",
            "text": ""
        }
    })
}

/// Create content block stop event
fn create_content_block_stop_event(index: usize) -> Value {
    serde_json::json!({
        "type": "content_block_stop",
        "index": index
    })
}

/// Create text delta event
fn create_text_delta_event(text: &str) -> Value {
    serde_json::json!({
        "type": "content_block_delta",
        "index": 2,
        "delta": {
            "type": "text_delta",
            "text": text
        }
    })
}

/// Create message delta event
fn create_message_delta_event(stop_reason: &str, input_tokens: i32, output_tokens: i32, usage_key: &str) -> Value {
    serde_json::json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": stop_reason,
            "stop_sequence": null
        },
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "server_tool_use": { usage_key: 1 }
        }
    })
}

/// Create message stop event
fn create_message_stop_event() -> Value {
    serde_json::json!({
        "type": "message_stop"
    })
}

/// Stream tool result
async fn stream_tool_result(
    tool_name: &str,
    tool_id: &str,
    tool_input: &Value,
    web_fetch_egress: &WebFetchEgressPolicy,
    verbose_client_errors: bool,
) -> Pin<Box<dyn Stream<Item = Value> + Send + 'static>> {
    let tool_id = tool_id.to_string();
    let web_fetch_egress = web_fetch_egress.clone();
    
    if tool_name == "web_search" {
        let query = tool_input.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Box::pin(async_stream::stream! {
            match run_web_search(&query).await {
                Ok(results) => {
                    let _result_content = results.iter().map(|r| {
                        serde_json::json!({
                            "type": "web_search_result",
                            "title": r.title,
                            "url": r.url
                        })
                    }).collect::<Vec<_>>();
                    
                    let summary = create_search_summary(&query, &results);
                    
                    yield create_content_block_start_event(1, "web_search_result", &tool_id, tool_id.as_str(), &serde_json::json!({}));
                    yield create_content_block_stop_event(1);
                    yield create_text_delta_event(&summary);
                    yield create_content_block_stop_event(2);
                },
                Err(e) => {
                    yield create_error_event(&tool_id, &e.to_string(), verbose_client_errors);
                }
            }
        })
    } else {
        let url = tool_input.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Box::pin(async_stream::stream! {
            match run_web_fetch(&url, &web_fetch_egress).await {
                Ok(fetched) => {
                    let _result_content = serde_json::json!({
                        "type": "web_fetch_result",
                        "tool_use_id": tool_id,
                        "content": fetched
                    });
                    
                    let data = fetched.get("data").and_then(|v| v.as_str()).unwrap_or("");
                    let summary = data.chars().take(MAX_FETCH_CHARS).collect::<String>();
                    
                    yield create_content_block_start_event(1, "web_fetch_result", &tool_id, tool_id.as_str(), &_result_content);
                    yield create_content_block_stop_event(1);
                    yield create_text_delta_event(&summary);
                    yield create_content_block_stop_event(2);
                },
                Err(e) => {
                    yield create_error_event(&tool_id, &e.to_string(), verbose_client_errors);
                }
            }
        })
    }
}

/// Create search summary
fn create_search_summary(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        format!("No web search results found for: {}", query)
    } else {
        let mut lines = vec![format!("Search results for: {}", query)];
        for (index, result) in results.iter().enumerate() {
            lines.push(format!("{}. {}\n{}", index + 1, result.title, result.url));
        }
        lines.join("\n\n")
    }
}

/// Create error event
fn create_error_event(tool_id: &str, error: &str, verbose: bool) -> Value {
    let summary = if verbose {
        format!("Web tool request failed: {}", error)
    } else {
        "Web tool request failed.".to_string()
    };
    
    serde_json::json!({
        "type": "web_fetch_tool_result",
        "tool_use_id": tool_id,
        "content": {
            "type": "web_fetch_tool_error",
            "error_code": "unavailable",
            "message": summary
        }
    })
}