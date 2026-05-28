use serde::{Deserialize, Serialize};
use std::collections::HashMap;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextContent),
    #[serde(rename = "image")]
    Image(ImageContent),
    #[serde(rename = "document")]
    Document(DocumentContent),
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseContent),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultContent),
    #[serde(rename = "thinking")]
    Thinking(ThinkingContent),
    #[serde(rename = "redacted_thinking")]
    RedactedThinking(RedactedThinkingContent),
    #[serde(rename = "server_tool_use")]
    ServerToolUse(ServerToolUseContent),
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult(WebSearchToolResultContent),
    #[serde(rename = "web_fetch_tool_result")]
    WebFetchToolResult(WebFetchToolResultContent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "text")]
    pub text: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageContent {
    #[serde(rename = "source")]
    pub source: HashMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentContent {
    #[serde(rename = "source")]
    pub source: HashMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseContent {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "input")]
    pub input: HashMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultContent {
    #[serde(rename = "tool_use_id")]
    pub tool_use_id: String,
    #[serde(rename = "content")]
    pub content: serde_json::Value,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinkingContent {
    #[serde(rename = "thinking")]
    pub thinking: String,
    #[serde(rename = "signature")]
    pub signature: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactedThinkingContent {
    #[serde(rename = "data")]
    pub data: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerToolUseContent {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "input")]
    pub input: HashMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebSearchToolResultContent {
    #[serde(rename = "tool_use_id")]
    pub tool_use_id: String,
    #[serde(rename = "content")]
    pub content: serde_json::Value,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebFetchToolResultContent {
    #[serde(rename = "tool_use_id")]
    pub tool_use_id: String,
    #[serde(rename = "content")]
    pub content: serde_json::Value,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "role")]
    pub role: Role,
    #[serde(rename = "content")]
    pub content: ContentOrBlocks,
    #[serde(rename = "reasoning_content")]
    pub reasoning_content: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentOrBlocks {
    String(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemContent {
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "text")]
    pub text: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "input_schema")]
    pub input_schema: Option<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "enabled")]
    pub enabled: Option<bool>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    #[serde(rename = "budget_tokens")]
    pub budget_tokens: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemContentOrString {
    System(SystemContent),
    String(String),
}

impl Default for SystemContentOrString {
    fn default() -> Self {
        Self::String(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessagesRequest {
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "original_model")]
    #[serde(skip_serializing)]
    pub original_model: Option<String>,
    #[serde(rename = "resolved_provider_model")]
    #[serde(skip_serializing)]
    pub resolved_provider_model: Option<String>,
    #[serde(rename = "max_tokens")]
    pub max_tokens: Option<i32>,
    #[serde(rename = "messages")]
    pub messages: Vec<Message>,
    #[serde(rename = "system")]
    pub system: Option<SystemContentOrString>,
    #[serde(rename = "stop_sequences")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(rename = "stream")]
    pub stream: Option<bool>,
    #[serde(rename = "temperature")]
    pub temperature: Option<f64>,
    #[serde(rename = "top_p")]
    pub top_p: Option<f64>,
    #[serde(rename = "top_k")]
    pub top_k: Option<i32>,
    #[serde(rename = "metadata")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "tools")]
    pub tools: Option<Vec<Tool>>,
    #[serde(rename = "tool_choice")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(rename = "thinking")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(rename = "context_management")]
    pub context_management: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "output_config")]
    pub output_config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "mcp_servers")]
    pub mcp_servers: Option<Vec<HashMap<String, serde_json::Value>>>,
    #[serde(rename = "extra_body")]
    pub extra_body: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "betas")]
    #[serde(skip_serializing)]
    pub betas: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl MessagesRequest {
    pub fn new(model: String, messages: Vec<Message>) -> Self {
        Self {
            model,
            messages,
            original_model: None,
            resolved_provider_model: None,
            max_tokens: None,
            system: None,
            stop_sequences: None,
            stream: Some(true),
            temperature: None,
            top_p: None,
            top_k: None,
            metadata: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            context_management: None,
            output_config: None,
            mcp_servers: None,
            extra_body: None,
            betas: None,
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCountRequest {
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "original_model")]
    #[serde(skip_serializing)]
    pub original_model: Option<String>,
    #[serde(rename = "resolved_provider_model")]
    #[serde(skip_serializing)]
    pub resolved_provider_model: Option<String>,
    #[serde(rename = "messages")]
    pub messages: Vec<Message>,
    #[serde(rename = "system")]
    pub system: Option<SystemContentOrString>,
    #[serde(rename = "tools")]
    pub tools: Option<Vec<Tool>>,
    #[serde(rename = "thinking")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(rename = "tool_choice")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(rename = "context_management")]
    pub context_management: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "output_config")]
    pub output_config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "mcp_servers")]
    pub mcp_servers: Option<Vec<HashMap<String, serde_json::Value>>>,
    #[serde(rename = "betas")]
    #[serde(skip_serializing)]
    pub betas: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCountResponse {
    #[serde(rename = "input_tokens")]
    pub input_tokens: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelsListResponse {
    #[serde(rename = "data")]
    pub data: Vec<ModelResponse>,
    #[serde(rename = "first_id")]
    pub first_id: Option<String>,
    #[serde(rename = "has_more")]
    pub has_more: bool,
    #[serde(rename = "last_id")]
    pub last_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(rename = "input_tokens")]
    pub input_tokens: i32,
    #[serde(rename = "output_tokens")]
    pub output_tokens: i32,
    #[serde(rename = "cache_creation_input_tokens")]
    #[serde(default)]
    pub cache_creation_input_tokens: i32,
    #[serde(rename = "cache_read_input_tokens")]
    #[serde(default)]
    pub cache_read_input_tokens: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessagesResponse {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "role")]
    pub role: String,
    #[serde(rename = "content")]
    pub content: Vec<serde_json::Value>,
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "stop_reason")]
    pub stop_reason: Option<String>,
    #[serde(rename = "stop_sequence")]
    pub stop_sequence: Option<String>,
    #[serde(rename = "usage")]
    pub usage: Usage,
}

impl MessagesResponse {
    pub fn new(id: String, model: String, content: Vec<serde_json::Value>, usage: Usage) -> Self {
        Self {
            id,
            model,
            role: "assistant".to_string(),
            content,
            type_field: "message".to_string(),
            stop_reason: None,
            stop_sequence: None,
            usage,
        }
    }
}

impl MessagesRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.model.is_empty() {
            return Err("Model cannot be empty".to_string());
        }
        
        if self.messages.is_empty() {
            return Err("Messages cannot be empty".to_string());
        }
        
        for message in &self.messages {
            if message.role == Role::System && message.content == ContentOrBlocks::String(String::new()) {
                return Err("System message content cannot be empty".to_string());
            }
        }
        
        Ok(())
    }
}

impl Message {
    pub fn new(role: Role, content: ContentOrBlocks) -> Self {
        Self {
            role,
            content,
            reasoning_content: None,
            extra: HashMap::new(),
        }
    }
}

impl ContentBlock {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text(text) => Some(&text.text),
            _ => None,
        }
    }
    
    pub fn as_image(&self) -> Option<&HashMap<String, serde_json::Value>> {
        match self {
            ContentBlock::Image(img) => Some(&img.source),
            _ => None,
        }
    }
    
    pub fn as_tool_use(&self) -> Option<&ToolUseContent> {
        match self {
            ContentBlock::ToolUse(tool_use) => Some(tool_use),
            _ => None,
        }
    }
}

pub fn content_text(content: &ContentOrBlocks) -> String {
    match content {
        ContentOrBlocks::String(s) => s.clone(),
        ContentOrBlocks::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if let Some(text) = block.as_text() {
                    parts.push(text);
                }
            }
            parts.join("\n")
        }
    }
}