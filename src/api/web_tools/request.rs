use crate::models::{MessagesRequest, Tool};

pub fn request_text(request: &MessagesRequest) -> String {
    request
        .messages
        .iter()
        .map(|m| crate::core::anthropic::utils::extract_text_from_message_content(&m.content))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn forced_tool_turn_text(request: &MessagesRequest) -> String {
    for message in request.messages.iter().rev() {
        if message.role == crate::models::Role::User {
            return crate::core::anthropic::utils::extract_text_from_message_content(
                &message.content,
            );
        }
    }
    String::new()
}

pub fn forced_server_tool_name(request: &MessagesRequest) -> Option<String> {
    let tc = request.tool_choice.as_ref()?;
    let obj = tc.as_object()?;
    if obj.get("type")?.as_str()? != "tool" {
        return None;
    }
    let name = obj.get("name")?.as_str()?;
    if name == "web_search" || name == "web_fetch" {
        Some(name.to_string())
    } else {
        None
    }
}

pub fn has_tool_named(request: &MessagesRequest, name: &str) -> bool {
    request
        .tools
        .as_ref()
        .map(|tools| tools.iter().any(|t| t.name == name))
        .unwrap_or(false)
}

pub fn is_web_server_tool_request(request: &MessagesRequest) -> bool {
    let forced = match forced_server_tool_name(request) {
        Some(f) => f,
        None => return false,
    };
    has_tool_named(request, &forced)
}

pub fn is_anthropic_server_tool_definition(tool: &Tool) -> bool {
    let name = tool.name.trim();
    if name == "web_search" || name == "web_fetch" {
        return true;
    }
    if let Some(ref typ) = tool.type_field {
        let t = typ.trim();
        if t.starts_with("web_search") || t.starts_with("web_fetch") {
            return true;
        }
    }
    false
}

pub fn has_listed_anthropic_server_tools(request: &MessagesRequest) -> bool {
    request
        .tools
        .as_ref()
        .map(|tools| tools.iter().any(|t| is_anthropic_server_tool_definition(t)))
        .unwrap_or(false)
}

pub fn openai_chat_upstream_server_tool_error(
    request: &MessagesRequest,
    web_tools_enabled: bool,
) -> Option<String> {
    let forced = forced_server_tool_name(request);
    if let Some(ref name) = forced {
        if !web_tools_enabled {
            return Some(format!(
                "tool_choice forces Anthropic server tool {name:?}, but local web server tools are \
                 disabled (ENABLE_WEB_SERVER_TOOLS=false). Enable them or use a native Anthropic \
                 Messages transport (e.g. open_router, ollama, lmstudio)."
            ));
        }
    }
    if forced.is_none() && has_listed_anthropic_server_tools(request) {
        return Some(
            "OpenAI Chat upstreams (NVIDIA NIM) cannot use listed Anthropic server tools \
             (web_search / web_fetch) without the local web server tool handler. Use a native \
             Anthropic transport, set ENABLE_WEB_SERVER_TOOLS=true and force the tool with \
             tool_choice, or remove these tools from the request."
                .to_string(),
        );
    }
    None
}
