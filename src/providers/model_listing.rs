use serde_json::Value;
use std::collections::HashSet;

use super::exceptions::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderModelInfo {
    pub model_id: String,
    pub supports_thinking: Option<bool>,
}

pub fn model_infos_from_ids(
    model_ids: impl IntoIterator<Item = impl AsRef<str>>,
    supports_thinking: Option<bool>,
) -> Vec<ProviderModelInfo> {
    model_ids
        .into_iter()
        .filter(|id| !id.as_ref().trim().is_empty())
        .map(|id| ProviderModelInfo {
            model_id: id.as_ref().to_string(),
            supports_thinking,
        })
        .collect()
}

pub fn extract_openai_model_ids(payload: &Value, provider_name: &str) -> Result<Vec<String>, ProviderError> {
    let data = match payload.get("data").and_then(|v| v.as_array()) {
        Some(d) => d,
        None => {
            return Err(ProviderError::service_unavailable(&format!(
                "{} model-list response is malformed: expected top-level data array",
                provider_name
            )));
        }
    };

    let mut model_ids = Vec::new();
    for item in data {
        let model_id = match item.get("id").and_then(|v| v.as_str()) {
            Some(id) if !id.trim().is_empty() => id.to_string(),
            _ => {
                return Err(ProviderError::service_unavailable(&format!(
                    "{} model-list response is malformed: expected every data item to include id",
                    provider_name
                )));
            }
        };
        model_ids.push(model_id);
    }

    if model_ids.is_empty() {
        return Err(ProviderError::service_unavailable(&format!(
            "{} model-list response did not include any model ids",
            provider_name
        )));
    }

    Ok(model_ids)
}

pub fn extract_openrouter_tool_model_infos(
    payload: &Value,
    provider_name: &str,
) -> Result<Vec<ProviderModelInfo>, ProviderError> {
    let data = match payload.get("data").and_then(|v| v.as_array()) {
        Some(d) => d,
        None => {
            return Err(ProviderError::service_unavailable(&format!(
                "{} model-list response is malformed: expected top-level data array",
                provider_name
            )));
        }
    };

    let mut model_infos = Vec::new();
    for item in data {
        let model_id = match item.get("id").and_then(|v| v.as_str()) {
            Some(id) if !id.trim().is_empty() => id.to_string(),
            _ => continue,
        };

        let supported_parameters = match item
            .get("supported_parameters")
            .and_then(|v| v.as_array())
        {
            Some(p) => p,
            None => continue,
        };

        let param_names: HashSet<&str> = supported_parameters
            .iter()
            .filter_map(|p| p.as_str())
            .collect();

        let tool_params: HashSet<&str> = ["tools", "tool_choice"].into_iter().collect();
        if param_names.is_disjoint(&tool_params) {
            continue;
        }

        model_infos.push(ProviderModelInfo {
            supports_thinking: Some(param_names.contains("reasoning")),
            model_id,
        });
    }

    Ok(model_infos)
}

pub fn extract_ollama_model_ids(payload: &Value, provider_name: &str) -> Result<Vec<String>, ProviderError> {
    let models = match payload.get("models").and_then(|v| v.as_array()) {
        Some(m) => m,
        None => {
            return Err(ProviderError::service_unavailable(&format!(
                "{} model-list response is malformed: expected top-level models array",
                provider_name
            )));
        }
    };

    let mut model_ids = Vec::new();
    for item in models {
        let mut found = false;
        for key in &["model", "name"] {
            if let Some(val) = item.get(*key).and_then(|v| v.as_str()) {
                if !val.trim().is_empty() {
                    model_ids.push(val.to_string());
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return Err(ProviderError::service_unavailable(&format!(
                "{} model-list response is malformed: expected every models item to include model or name",
                provider_name
            )));
        }
    }

    if model_ids.is_empty() {
        return Err(ProviderError::service_unavailable(&format!(
            "{} model-list response did not include any model ids",
            provider_name
        )));
    }

    Ok(model_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_infos_from_ids() {
        let infos = model_infos_from_ids(vec!["model-a", "model-b"], Some(true));
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].model_id, "model-a");
        assert_eq!(infos[0].supports_thinking, Some(true));
    }

    #[test]
    fn test_model_infos_from_ids_filters_empty() {
        let infos = model_infos_from_ids(vec!["model-a", "", "model-b"], None);
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn test_extract_openai_model_ids() {
        let payload = serde_json::json!({
            "data": [
                {"id": "gpt-4"},
                {"id": "gpt-3.5-turbo"},
            ]
        });
        let ids = extract_openai_model_ids(&payload, "test").unwrap();
        assert_eq!(ids, vec!["gpt-4", "gpt-3.5-turbo"]);
    }

    #[test]
    fn test_extract_openai_model_ids_missing_data() {
        let payload = serde_json::json!({});
        let result = extract_openai_model_ids(&payload, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_openai_model_ids_empty() {
        let payload = serde_json::json!({"data": []});
        let result = extract_openai_model_ids(&payload, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_openrouter_tool_model_infos() {
        let payload = serde_json::json!({
            "data": [
                {
                    "id": "model-with-tools",
                    "supported_parameters": ["tools", "tool_choice", "reasoning"]
                },
                {
                    "id": "model-without-tools",
                    "supported_parameters": ["temperature", "max_tokens"]
                },
            ]
        });
        let infos = extract_openrouter_tool_model_infos(&payload, "test").unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].model_id, "model-with-tools");
        assert_eq!(infos[0].supports_thinking, Some(true));
    }

    #[test]
    fn test_extract_ollama_model_ids() {
        let payload = serde_json::json!({
            "models": [
                {"model": "llama3", "name": "llama3:latest"},
                {"model": "mistral"},
            ]
        });
        let ids = extract_ollama_model_ids(&payload, "test").unwrap();
        assert!(ids.contains(&"llama3".to_string()));
        assert!(ids.contains(&"mistral".to_string()));
    }

    #[test]
    fn test_extract_ollama_model_ids_missing_models() {
        let payload = serde_json::json!({});
        let result = extract_ollama_model_ids(&payload, "test");
        assert!(result.is_err());
    }
}
