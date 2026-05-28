use crate::core::anthropic::utils::{
    extract_text_from_message_content, extract_text_from_system_content,
};
use crate::models::MessagesRequest;

/// Check if this is a quota probe request.
pub fn is_quota_check_request(request: &MessagesRequest) -> bool {
    if request.max_tokens != Some(1) || request.messages.len() != 1 {
        return false;
    }
    if request.messages[0].role != crate::models::Role::User {
        return false;
    }
    let text = extract_text_from_message_content(&request.messages[0].content);
    text.to_lowercase().contains("quota")
}

/// Check if this is a conversation title generation request.
pub fn is_title_generation_request(request: &MessagesRequest) -> bool {
    let system = match &request.system {
        Some(s) => extract_text_from_system_content(s),
        None => return false,
    };
    if request.tools.is_some() {
        return false;
    }
    let system_lower = system.to_lowercase();
    if !system_lower.contains("title") {
        return false;
    }
    system_lower.contains("sentence-case title")
        || (system_lower.contains("return json")
            && system_lower.contains("field")
            && (system_lower.contains("coding session") || system_lower.contains("this session")))
}

/// Check if this is a fast prefix detection request. Returns (is_prefix, command).
pub fn is_prefix_detection_request(request: &MessagesRequest) -> (bool, String) {
    if request.messages.len() != 1 || request.messages[0].role != crate::models::Role::User {
        return (false, String::new());
    }
    let content = extract_text_from_message_content(&request.messages[0].content);
    if content.contains("<policy_spec>") && content.contains("Command:") {
        if let Some(pos) = content.rfind("Command:") {
            let cmd = content[pos + "Command:".len()..].to_string();
            return (true, cmd.trim().to_string());
        }
    }
    (false, String::new())
}

/// Check if this is a suggestion mode request.
pub fn is_suggestion_mode_request(request: &MessagesRequest) -> bool {
    for msg in &request.messages {
        if msg.role == crate::models::Role::User {
            let text = extract_text_from_message_content(&msg.content);
            if text.contains("[SUGGESTION MODE:") {
                return true;
            }
        }
    }
    false
}

/// Check if this is a filepath extraction request. Returns (is_fp, command, output).
pub fn is_filepath_extraction_request(
    request: &MessagesRequest,
) -> (bool, String, String) {
    if request.messages.len() != 1 || request.messages[0].role != crate::models::Role::User {
        return (false, String::new(), String::new());
    }
    if request.tools.is_some() {
        return (false, String::new(), String::new());
    }
    let content = extract_text_from_message_content(&request.messages[0].content);
    if !content.contains("Command:") || !content.contains("Output:") {
        return (false, String::new(), String::new());
    }
    let user_has_filepaths =
        content.to_lowercase().contains("filepaths") || content.contains("<filepaths>");
    let system_has_extract = match &request.system {
        Some(s) => {
            let t = extract_text_from_system_content(s).to_lowercase();
            t.contains("extract any file paths") || t.contains("file paths that this command")
        }
        None => false,
    };
    if !user_has_filepaths && !system_has_extract {
        return (false, String::new(), String::new());
    }
    let cmd_start = content.find("Command:").map(|p| p + "Command:".len());
    let cmd_start = match cmd_start {
        Some(p) => p,
        None => return (false, String::new(), String::new()),
    };
    let output_marker = content[cmd_start..].find("Output:").map(|p| cmd_start + p);
    let output_marker = match output_marker {
        Some(p) => p,
        None => return (false, String::new(), String::new()),
    };
    let command = content[cmd_start..output_marker].trim().to_string();
    let mut output = content[output_marker + "Output:".len()..].trim().to_string();
    for marker in &["<", "\n\n"] {
        if let Some(pos) = output.find(marker) {
            output = output[..pos].trim().to_string();
        }
    }
    (true, command, output)
}
