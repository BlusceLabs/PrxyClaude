use crate::models::{ContentOrBlocks, MessagesRequest, Role, Tool};

/// Check if a request is a web server tool request
pub fn is_web_server_tool_request(request: &MessagesRequest) -> bool {
    get_forced_server_tool_name(request).is_some() && has_tool_named(request, &get_forced_server_tool_name(request).unwrap())
}

/// Get forced server tool name from request (from text content)
pub fn get_forced_server_tool_name(request: &MessagesRequest) -> Option<String> {
    if let Some(tool_choice) = &request.tool_choice {
        if let Some(tool_choice_map) = tool_choice.as_object() {
            if let Some(tool_map) = tool_choice_map.get("type") {
                if tool_map == "tool" {
                    if let Some(name) = tool_choice_map.get("name") {
                        if let Some(name_str) = name.as_str() {
                            let name = name_str.to_string();
                            if name == "web_search" || name == "web_fetch" {
                                return Some(name);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Also check for tool-like patterns in the last user message
    if request.messages.is_empty() {
        return None;
    }
    
    let last_message = request.messages.last()?;
    if last_message.role != Role::User {
        return None;
    }
    
    let text = match &last_message.content {
        ContentOrBlocks::String(s) => s.clone(),
        ContentOrBlocks::Blocks(blocks) => {
            let mut text = String::new();
            for block in blocks {
                if let Some(text_block) = block.as_text() {
                    text.push_str(text_block);
                }
            }
            text
        }
    };
    
    if text.contains("web_search") {
        return Some("web_search".to_string());
    } else if text.contains("web_fetch") {
        return Some("web_fetch".to_string());
    }
    
    None
}

/// Check if request has a tool with the given name
pub fn has_tool_named(request: &MessagesRequest, name: &str) -> bool {
    request.tools.as_ref().map_or(false, |tools| tools.iter().any(|t| t.name == name))
}

/// Check if Tool is an Anthropic server tool definition
pub fn is_anthropic_server_tool_definition(tool: &Tool) -> bool {
    tool.extra.contains_key("server") || 
    tool.description.as_ref().map_or(false, |desc| desc.contains("web_search") || desc.contains("web_fetch")) ||
    tool.name.trim() == "web_search" || tool.name.trim() == "web_fetch"
}

/// Get text from request messages
pub fn request_text(request: &MessagesRequest) -> String {
    let mut text_parts = Vec::new();
    
    for message in &request.messages {
        match &message.content {
            ContentOrBlocks::String(s) => {
                text_parts.push(s.clone());
            },
            ContentOrBlocks::Blocks(blocks) => {
                for block in blocks {
                    if let Some(text) = block.as_text() {
                        text_parts.push(text.to_string());
                    }
                }
            },
        }
    }
    
    text_parts.join("\n")
}

/// Get text for forced tool turn (latest user turn only)
pub fn forced_tool_turn_text(request: &MessagesRequest) -> String {
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

/// Check if request has listed Anthropic server tools
pub fn has_listed_anthropic_server_tools(request: &MessagesRequest) -> bool {
    if let Some(tools) = &request.tools {
        for tool in tools {
            if is_anthropic_server_tool_definition(tool) {
                return true;
            }
        }
    }
    false
}

/// Get OpenAI chat upstream server tool error
pub fn openai_chat_upstream_server_tool_error(request: &MessagesRequest, web_tools_enabled: bool) -> Option<String> {
    let forced = get_forced_server_tool_name(request);
    
    if let Some(tool_name) = forced {
        if !web_tools_enabled {
            return Some(format!(
                "tool_choice forces Anthropic server tool '{}', but local web server tools are disabled (ENABLE_WEB_SERVER_TOOLS=false). Enable them or use a native Anthropic transport.",
                tool_name
            ));
        }
    } else if has_listed_anthropic_server_tools(request) {
        return Some(
            "OpenAI Chat upstreams (NVIDIA NIM) cannot use listed Anthropic server tools (web_search / web_fetch) without the local web server tool handler. Use a native Anthropic transport, set ENABLE_WEB_SERVER_TOOLS=true and force the tool with tool_choice, or remove these tools from the request."
                .to_string()
        );
    }
    
    None
}