/// HTML parsing utilities for web tools

use std::collections::VecDeque;

/// HTML text parser for extracting visible text
pub struct HTMLTextParser {
    title: String,
    text_parts: VecDeque<String>,
    in_title: bool,
    skip_depth: usize,
}

impl HTMLTextParser {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text_parts: VecDeque::new(),
            in_title: false,
            skip_depth: 0,
        }
    }
    
    pub fn parse(&mut self, html: &str) {
        let mut chars = html.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '<' {
                // Handle tag
                let tag = self.parse_tag(&mut chars);
                self.handle_tag_start(&tag);
            } else {
                // Handle text content
                let mut text = String::new();
                text.push(c);
                
                while let Some(&next_char) = chars.peek() {
                    if next_char == '<' {
                        break;
                    }
                    text.push(chars.next().unwrap());
                }
                
                let text = text.trim();
                if !text.is_empty() && self.skip_depth == 0 {
                    if self.in_title {
                        self.title = format!("{} {}", self.title, text);
                    } else {
                        self.text_parts_back(text);
                    }
                }
            }
        }
    }
    
    pub fn title(&self) -> &str {
        &self.title
    }
    
    pub fn text_parts(&mut self) -> &mut [String] {
        self.text_parts.make_contiguous()
    }
    
    fn parse_tag(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut tag = String::new();
        
        while let Some(c) = chars.next() {
            if c == '>' {
                break;
            }
            tag.push(c);
        }
        
        tag
    }
    
    fn handle_tag_start(&mut self, tag: &str) {
        let tag_lower = tag.to_lowercase();
        
        // Handle end tags
        if tag_lower.starts_with("/script") || tag_lower.starts_with("/style") || tag_lower.starts_with("/noscript") {
            if self.skip_depth > 0 {
                self.skip_depth -= 1;
            }
            return;
        }
        
        // Handle start tags
        if tag_lower == "title" {
            self.in_title = true;
            return;
        }
        
        if tag_lower == "script" || tag_lower == "style" || tag_lower == "noscript" {
            self.skip_depth += 1;
            return;
        }
    }
    
    fn text_parts_back(&mut self, text: &str) {
        self.text_parts_back_from(text);
    }
    
    fn text_parts_back_from(&mut self, text: &str) {
        self.text_parts.push_back(text.to_string());
    }
}

/// Parse HTML and return title and text parts
pub fn parse_html(html: &str) -> (String, Vec<String>) {
    let mut parser = HTMLTextParser::new();
    parser.parse(html);
    (parser.title().trim().to_string(), parser.text_parts().to_vec())
}

/// Extract text content from various content types
pub fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text);
                }
            }
            parts.join("\n")
        }
        serde_json::Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                text.to_string()
            } else {
                serde_json::to_string(obj).unwrap_or_default()
            }
        }
        _ => "".to_string(),
    }
}