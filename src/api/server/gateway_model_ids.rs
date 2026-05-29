use serde::{Deserialize, Serialize};

pub const GATEWAY_MODEL_ID_PREFIX: &str = "anthropic";
pub const NO_THINKING_GATEWAY_MODEL_ID_PREFIX: &str = "claude-3-proxycc-no-thinking";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedGatewayModelId {
    pub provider_id: String,
    pub provider_model: String,
    pub force_thinking_enabled: Option<bool>,
}

pub fn gateway_model_id(provider_model_ref: &str) -> String {
    format!("{}/{}", GATEWAY_MODEL_ID_PREFIX, provider_model_ref)
}

pub fn no_thinking_gateway_model_id(provider_model_ref: &str) -> String {
    format!(
        "{}/{}",
        NO_THINKING_GATEWAY_MODEL_ID_PREFIX, provider_model_ref
    )
}

pub fn decode_gateway_model_id(model_name: &str) -> Option<DecodedGatewayModelId> {
    let (prefix, remainder) = model_name.split_once('/')?;

    let force_thinking_enabled = match prefix {
        GATEWAY_MODEL_ID_PREFIX => None,
        NO_THINKING_GATEWAY_MODEL_ID_PREFIX => Some(false),
        _ => return None,
    };

    let (provider_id, provider_model) = remainder.split_once('/')?;

    if provider_model.is_empty() {
        return None;
    }

    Some(DecodedGatewayModelId {
        provider_id: provider_id.to_string(),
        provider_model: provider_model.to_string(),
        force_thinking_enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_model_id() {
        let result = gateway_model_id("openai/gpt-4");
        assert_eq!(result, "anthropic/openai/gpt-4");
    }

    #[test]
    fn test_no_thinking_gateway_model_id() {
        let result = no_thinking_gateway_model_id("openai/gpt-4");
        assert_eq!(result, "claude-3-proxycc-no-thinking/openai/gpt-4");
    }

    #[test]
    fn test_decode_gateway_model_id_thinking() {
        let decoded = decode_gateway_model_id("anthropic/openai/gpt-4").unwrap();
        assert_eq!(decoded.provider_id, "openai");
        assert_eq!(decoded.provider_model, "gpt-4");
        assert_eq!(decoded.force_thinking_enabled, None);
    }

    #[test]
    fn test_decode_gateway_model_id_no_thinking() {
        let decoded = decode_gateway_model_id("claude-3-proxycc-no-thinking/openai/gpt-4").unwrap();
        assert_eq!(decoded.provider_id, "openai");
        assert_eq!(decoded.provider_model, "gpt-4");
        assert_eq!(decoded.force_thinking_enabled, Some(false));
    }

    #[test]
    fn test_decode_gateway_model_id_invalid_prefix() {
        let result = decode_gateway_model_id("unknown/openai/gpt-4");
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_gateway_model_id_no_slash() {
        let result = decode_gateway_model_id("anthropic");
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_gateway_model_id_only_provider() {
        let result = decode_gateway_model_id("anthropic/openai");
        assert!(result.is_none());
    }
}
