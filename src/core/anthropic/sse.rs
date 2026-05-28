use serde_json::Value;

/// SSE (Server-Sent Events) format utilities
pub fn format_sse_event(event_type: &str, data: &Value) -> String {
    format!("event: {}\ndata: {}\n\n", event_type, data)
}

/// SSE builder for constructing SSE responses
pub struct SSEBuilder {
    events: Vec<String>,
}

impl SSEBuilder {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
    
    pub fn add_event(mut self, event_type: &str, data: Value) -> Self {
        self.events.push(format_sse_event(event_type, &data));
        self
    }
    
    pub fn build(self) -> String {
        self.events.join("")
    }
}

/// Content block manager for SSE content blocks
pub struct ContentBlockManager {
    blocks: Vec<Value>,
    current_index: usize,
}

impl ContentBlockManager {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_index: 0,
        }
    }
    
    pub fn add_block(&mut self, block: Value) {
        self.blocks.push(block);
    }
    
    pub fn get_block(&mut self, index: usize) -> Option<&Value> {
        self.blocks.get(index)
    }
    
    pub fn next_block(&mut self) -> Option<&Value> {
        let block = self.blocks.get(self.current_index);
        if block.is_some() {
            self.current_index += 1;
        }
        block
    }
}

/// Map API stop reason to appropriate value
pub fn map_stop_reason(stop_reason: &str) -> Option<String> {
    match stop_reason {
        "end_turn" => Some("end_turn".to_string()),
        "max_tokens" => Some("max_tokens".to_string()),
        "stop_sequence" => Some("stop_sequence".to_string()),
        "tool_use" => Some("tool_use".to_string()),
        _ => None,
    }
}