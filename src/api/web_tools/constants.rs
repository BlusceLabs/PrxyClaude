use std::collections::HashMap;

pub const REQUEST_TIMEOUT_S: f64 = 20.0;
pub const MAX_SEARCH_RESULTS: usize = 10;
pub const MAX_FETCH_CHARS: usize = 24_000;
pub const MAX_WEB_FETCH_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const REDIRECT_RESPONSE_BODY_CAP_BYTES: usize = 65_536;
pub const MAX_WEB_FETCH_REDIRECTS: usize = 10;

pub fn web_fetch_redirect_statuses() -> Vec<u16> {
    vec![301, 302, 303, 307, 308]
}

pub fn web_tool_http_headers() -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 compatible; PxyClaude/2.0".to_string(),
    );
    headers
}
