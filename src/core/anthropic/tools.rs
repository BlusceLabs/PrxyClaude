use std::collections::HashMap;

use regex::Regex;
use serde_json;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolUseDetection {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    Text,
    MatchingFunction,
    ParsingParameters,
}

pub struct HeuristicToolParser {
    state: ParserState,
    buffer: String,
    current_tool_id: String,
    current_function_name: String,
    current_parameters: HashMap<String, String>,
    control_token_re: Regex,
    func_start_re: Regex,
    param_re: Regex,
    param_re_partial: Regex,
    web_tool_json_re: Regex,
}

impl HeuristicToolParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Text,
            buffer: String::new(),
            current_tool_id: String::new(),
            current_function_name: String::new(),
            current_parameters: HashMap::new(),
            control_token_re: Regex::new(r"<\|[^|>]{1,80}\|>").unwrap(),
            func_start_re: Regex::new(r"\x{25cf}\s*<function=([^>]+)>").unwrap(),
            param_re: Regex::new(r"(?s)<parameter=([^>]+)>(.*?)(?:</parameter>|$)").unwrap(),
            param_re_partial: Regex::new(r"(?s)<parameter=([^>]+)>(.*)$").unwrap(),
            web_tool_json_re: Regex::new(
                r"(?is)\b(?:use\s+)?(?P<tool>WebFetch|WebSearch)\b.*?(?P<json>\{.*?\})",
            )
            .unwrap(),
        }
    }

    fn make_tool_id() -> String {
        let id = Uuid::new_v4();
        format!("toolu_heuristic_{}", &id.to_string()[..8])
    }

    fn strip_control_tokens(&self, text: &str) -> String {
        self.control_token_re.replace_all(text, "").to_string()
    }

    fn split_incomplete_control_token_tail(&mut self) -> String {
        let start = match self.buffer.rfind("<|") {
            Some(s) => s,
            None => return String::new(),
        };
        if self.buffer[start..].find("|>").is_some() {
            return String::new();
        }
        let prefix = self.buffer[..start].to_string();
        self.buffer = self.buffer[start..].to_string();
        prefix
    }

    fn flush_current_tool(&mut self) -> Option<ToolUseDetection> {
        if !self.current_function_name.is_empty() {
            let input = serde_json::to_value(&self.current_parameters).unwrap_or_default();
            let detection = ToolUseDetection {
                id: self.current_tool_id.clone(),
                name: self.current_function_name.clone(),
                input,
            };
            self.current_tool_id.clear();
            self.current_function_name.clear();
            self.current_parameters.clear();
            Some(detection)
        } else {
            None
        }
    }

    fn extract_web_tool_json_calls(&mut self) -> Vec<ToolUseDetection> {
        let mut detected_tools = Vec::new();
        let buffer_clone = self.buffer.clone();

        for cap in self.web_tool_json_re.captures_iter(&buffer_clone) {
            let tool_name = cap.name("tool").map(|m| m.as_str()).unwrap_or("");
            let json_str = cap.name("json").map(|m| m.as_str()).unwrap_or("");

            if let Ok(tool_input) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(obj) = tool_input.as_object() {
                    if tool_name == "WebFetch" && !obj.contains_key("url") {
                        continue;
                    }
                    if tool_name == "WebSearch" && !obj.contains_key("query") {
                        continue;
                    }
                    detected_tools.push(ToolUseDetection {
                        id: Self::make_tool_id(),
                        name: tool_name.to_string(),
                        input: tool_input,
                    });
                }
            }
        }

        if detected_tools.is_empty() {
            detected_tools
        } else {
            self.buffer.clear();
            detected_tools
        }
    }

    pub fn feed(&mut self, text: &str) -> (String, Vec<ToolUseDetection>) {
        self.buffer.push_str(text);
        self.buffer = self.strip_control_tokens(&self.buffer);

        let web_tools = self.extract_web_tool_json_calls();
        let mut detected_tools: Vec<ToolUseDetection> = Vec::new();
        let mut filtered_output_parts: Vec<String> = Vec::new();

        loop {
            if self.state == ParserState::Text {
                if let Some(idx) = self.buffer.find('\u{25cf}') {
                    filtered_output_parts.push(self.buffer[..idx].to_string());
                    self.buffer = self.buffer[idx..].to_string();
                    self.state = ParserState::MatchingFunction;
                } else {
                    let safe_prefix = self.split_incomplete_control_token_tail();
                    if !safe_prefix.is_empty() {
                        filtered_output_parts.push(safe_prefix);
                        break;
                    }
                    filtered_output_parts.push(self.buffer.clone());
                    self.buffer.clear();
                    break;
                }
            }

            if self.state == ParserState::MatchingFunction {
                if let Some(mat) = self.func_start_re.find(&self.buffer) {
                    let full_match = mat.as_str();
                    let name_start = full_match.find('=').unwrap_or(0) + 1;
                    let name_end = full_match.find('>').unwrap_or(full_match.len());
                    let name = full_match[name_start..name_end].trim().to_string();

                    self.current_function_name = name;
                    self.current_tool_id = Self::make_tool_id();
                    self.current_parameters.clear();
                    self.buffer = self.buffer[mat.end()..].to_string();
                    self.state = ParserState::ParsingParameters;
                } else if self.buffer.len() > 100 {
                    filtered_output_parts.push(self.buffer[..1].to_string());
                    self.buffer = self.buffer[1..].to_string();
                    self.state = ParserState::Text;
                } else {
                    break;
                }
            }

            if self.state == ParserState::ParsingParameters {
                let mut finished_tool_call = false;

                loop {
                    let buffer_clone = self.buffer.clone();
                    if let Some(cap) = self.param_re.captures(&buffer_clone) {
                        let full_match = cap.get(0).unwrap().as_str();
                        if full_match.contains("</parameter>") {
                            let pre_match = &self.buffer[..cap.get(0).unwrap().start()];
                            if !pre_match.is_empty() {
                                filtered_output_parts.push(pre_match.to_string());
                            }

                            let key = cap.get(1).unwrap().as_str().trim().to_string();
                            let val = cap.get(2).unwrap().as_str().trim().to_string();
                            self.current_parameters.insert(key, val);
                            self.buffer = self.buffer[cap.get(0).unwrap().end()..].to_string();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if self.buffer.contains("\u{25cf}") {
                    let idx = self.buffer.find("\u{25cf}").unwrap();
                    if idx > 0 {
                        filtered_output_parts.push(self.buffer[..idx].to_string());
                        self.buffer = self.buffer[idx..].to_string();
                    }
                    finished_tool_call = true;
                } else if !self.buffer.is_empty()
                    && !self.buffer.trim_start().starts_with('<')
                {
                    if !self.buffer.contains("<parameter=") {
                        filtered_output_parts.push(self.buffer.clone());
                        self.buffer.clear();
                        finished_tool_call = true;
                    }
                }

                if finished_tool_call {
                    if let Some(tool) = self.flush_current_tool() {
                        detected_tools.push(tool);
                    }
                    self.state = ParserState::Text;
                } else {
                    break;
                }
            }
        }

        detected_tools.extend(web_tools);
        (filtered_output_parts.join(""), detected_tools)
    }

    pub fn flush(&mut self) -> Vec<ToolUseDetection> {
        self.buffer = self.strip_control_tokens(&self.buffer.clone());
        let mut detected_tools = Vec::new();

        if self.state == ParserState::ParsingParameters {
            let buffer_clone = self.buffer.clone();
            for cap in self.param_re_partial.captures_iter(&buffer_clone) {
                let key = cap
                    .get(1)
                    .map(|m| m.as_str().trim())
                    .unwrap_or("")
                    .to_string();
                let val = cap
                    .get(2)
                    .map(|m| m.as_str().trim())
                    .unwrap_or("")
                    .to_string();
                self.current_parameters.insert(key, val);
            }

            detected_tools.push(ToolUseDetection {
                id: self.current_tool_id.clone(),
                name: self.current_function_name.clone(),
                input: serde_json::to_value(&self.current_parameters).unwrap_or_default(),
            });
            self.state = ParserState::Text;
            self.buffer.clear();
        }

        detected_tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_control_tokens() {
        let parser = HeuristicToolParser::new();
        assert_eq!(
            parser.strip_control_tokens("hello<|eos|>world"),
            "helloworld"
        );
        assert_eq!(parser.strip_control_tokens("<|start|>hello"), "hello");
        assert_eq!(parser.strip_control_tokens("hello"), "hello");
    }

    #[test]
    fn test_text_passthrough() {
        let mut parser = HeuristicToolParser::new();
        let (output, tools) = parser.feed("hello world");
        assert_eq!(output, "hello world");
        assert!(tools.is_empty());
    }

    #[test]
    fn test_function_detection_with_params() {
        let mut parser = HeuristicToolParser::new();
        let marker = '\u{25cf}';
        let input = format!(
            "some text {marker} <function=WebFetch>\n\
             <parameter=url>https://example.com</parameter>"
        );
        parser.feed(&input);
        let tools = parser.flush();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "WebFetch");
    }

    #[test]
    fn test_no_false_positive_non_dict_json() {
        let mut parser = HeuristicToolParser::new();
        let (_output, tools) = parser.feed(
            "WebSearch is cool. WebFetch [1, 2, 3]"
        );
        assert!(tools.is_empty());
    }
}
