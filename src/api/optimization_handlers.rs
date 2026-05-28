use uuid::Uuid;

use crate::api::command_utils::{extract_command_prefix, extract_filepaths_from_command};
use crate::api::detection::{
    is_filepath_extraction_request, is_prefix_detection_request, is_quota_check_request,
    is_suggestion_mode_request, is_title_generation_request,
};
use crate::config::Config;
use crate::models::{MessagesRequest, MessagesResponse, Usage};

fn _text_response(
    request_data: &MessagesRequest,
    text: &str,
    input_tokens: i32,
    output_tokens: i32,
) -> MessagesResponse {
    let content: Vec<serde_json::Value> =
        vec![serde_json::json!({"type": "text", "text": text})];
    MessagesResponse {
        id: format!("msg_{}", Uuid::new_v4().to_string().replace('-', "")),
        model: request_data.model.clone(),
        role: "assistant".to_string(),
        content,
        type_field: "message".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        },
    }
}

pub fn try_prefix_detection(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    if !config.features.fast_prefix_detection {
        return None;
    }

    let (is_prefix_req, command) = is_prefix_detection_request(request_data);
    if !is_prefix_req {
        return None;
    }

    Some(_text_response(
        request_data,
        &extract_command_prefix(&command),
        100,
        5,
    ))
}

pub fn try_quota_mock(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    if !config.features.enable_network_probe_mock {
        return None;
    }
    if !is_quota_check_request(request_data) {
        return None;
    }

    Some(_text_response(request_data, "Quota check passed.", 10, 5))
}

pub fn try_title_skip(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    if !config.features.enable_title_generation_skip {
        return None;
    }
    if !is_title_generation_request(request_data) {
        return None;
    }

    Some(_text_response(request_data, "Conversation", 100, 5))
}

pub fn try_suggestion_skip(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    if !config.features.enable_suggestion_mode_skip {
        return None;
    }
    if !is_suggestion_mode_request(request_data) {
        return None;
    }

    Some(_text_response(request_data, "", 100, 1))
}

pub fn try_filepath_mock(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    if !config.features.enable_filepath_extraction_mock {
        return None;
    }

    let (is_fp, cmd, output) = is_filepath_extraction_request(request_data);
    if !is_fp {
        return None;
    }

    let filepaths = extract_filepaths_from_command(&cmd, &output);
    Some(_text_response(request_data, &filepaths, 100, 10))
}

static OPTIMIZATION_HANDLERS: &[fn(&MessagesRequest, &Config) -> Option<MessagesResponse>] = &[
    try_quota_mock,
    try_prefix_detection,
    try_title_skip,
    try_suggestion_skip,
    try_filepath_mock,
];

pub fn try_optimizations(
    request_data: &MessagesRequest,
    config: &Config,
) -> Option<MessagesResponse> {
    for handler in OPTIMIZATION_HANDLERS {
        if let Some(result) = handler(request_data, config) {
            return Some(result);
        }
    }
    None
}
