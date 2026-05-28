use serde_json::Value;
use std::collections::HashMap;

/// Heuristic tool parser for parsing tool definitions
pub struct HeuristicToolParser;

impl HeuristicToolParser {
    /// Parse tool definitions from various formats
    pub fn parse_tools(tools: &Value) -> Vec<ToolDefinition> {
        let mut parsed_tools = Vec::new();
        
        match tools {
            Value::Array(tool_array) => {
                for tool in tool_array {
                    if let Ok(tool_def) = Self::parse_single_tool(tool) {
                        parsed_tools.push(tool_def);
                    }
                }
            }
            Value::Object(tool_obj) => {
                for (_name, tool_def) in tool_obj {
                    if let Ok(parsed) = Self::parse_single_tool(tool_def) {
                        parsed_tools.push(parsed);
                    }
                }
            }
            _ => {}
        }
        
        parsed_tools
    }
    
    fn parse_single_tool(tool: &Value) -> Result<ToolDefinition, String> {
        let name = tool.get("name")
            .and_then(|n| n.as_str())
            .ok_or("Tool name is required")?
            .to_string();
        
        let description = tool.get("description")
            .and_then(|d| d.as_str())
            .map(|d| d.to_string());
        
        let input_schema = tool.get("input_schema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        
        Ok(ToolDefinition {
            name,
            description,
            input_schema,
            extra: HashMap::new(),
        })
    }
}

/// Tool definition structure
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub extra: HashMap<String, Value>,
}