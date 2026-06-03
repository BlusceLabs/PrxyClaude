"""PrxyClaude · Tests for Core Modules"""

from __future__ import annotations

from config.settings import ModelMapping, get_settings
from core.types import AnthropicMessage, AnthropicRequest


def test_model_mapping_parse():
    """Test that ModelMapping correctly parses provider_type/model_name format."""
    mapping = ModelMapping.parse("nvidia_nim/z-ai/glm4.7")
    assert mapping.provider_type == "nvidia_nim"
    assert mapping.model_name == "z-ai/glm4.7"


def test_anthropic_request_creation():
    """Test that AnthropicRequest can be created with minimal fields."""
    messages = [AnthropicMessage(role="user", content="Hello")]
    req = AnthropicRequest(model="test-model", messages=messages)
    assert req.model == "test-model"
    assert len(req.messages) == 1
    assert req.messages[0].role == "user"


def test_settings_load():
    """Test that settings can be loaded."""
    settings = get_settings()
    assert settings is not None
    assert settings.port == 8082
