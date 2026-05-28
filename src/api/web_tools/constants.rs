//! Constants for web tools

/// Request timeout for web tools
pub const REQUEST_TIMEOUT_S: f64 = 20.0;

/// Maximum search results for web_search
pub const MAX_SEARCH_RESULTS: usize = 10;

/// Maximum characters for web_fetch
pub const MAX_FETCH_CHARS: usize = 24_000;

/// Maximum bytes to read from HTTP responses before decode/HTML parse
pub const MAX_WEB_FETCH_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Maximum bytes to drain from redirect responses before following Location
pub const REDIRECT_RESPONSE_BODY_CAP_BYTES: usize = 65_536;

/// Maximum redirects for web_fetch
pub const MAX_WEB_FETCH_REDIRECTS: usize = 10;

/// HTTP status codes that trigger redirects
pub fn web_fetch_redirect_statuses() -> std::collections::HashSet<u16> {
    std::collections::HashSet::from([301, 302, 303, 307, 308])
}

/// Default HTTP headers for web tools
pub const WEB_TOOL_HTTP_HEADERS: &[(&str, &str)] = &[
    ("User-Agent", "Mozilla/5.0 compatible; PxyClaude/2.0"),
];