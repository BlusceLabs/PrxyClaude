"""API Dependencies for dependency injection."""

import json
import time
from contextlib import suppress
from typing import Any

from loguru import logger

from config.nim import NimSettings
from config.settings import ModelMapping, Settings, get_config
from core.types import AnthropicRequest
from providers.base import BaseProvider, ProviderConfig
from providers.lmstudio import LMStudioProvider
from providers.nvidia_nim import NvidiaNimProvider
from providers.openrouter import OpenRouterProvider
from providers.transform import detect_tier


def get_settings() -> Settings:
    """Get application settings."""
    return Settings()


_provider_cache: dict[str, BaseProvider] = {}
_disabled_providers: set[str] = set()


def is_provider_disabled(provider_type: str) -> bool:
    return provider_type in _disabled_providers


def set_provider_enabled(provider_type: str, enabled: bool) -> None:
    if enabled:
        _disabled_providers.discard(provider_type)
    else:
        _disabled_providers.add(provider_type)


def get_disabled_providers() -> set[str]:
    return _disabled_providers.copy()


def get_provider_for_type(provider_type: str) -> BaseProvider:
    """Get provider instance for the given type."""
    if provider_type in _disabled_providers:
        raise ValueError(f"Provider '{provider_type}' is disabled via admin")

    if provider_type in _provider_cache:
        return _provider_cache[provider_type]

    settings = get_settings()

    if provider_type == "nvidia_nim":
        config = ProviderConfig(
            api_key=settings.nvidia_nim_api_key,
            base_url="https://integrate.api.nvidia.com/v1",
            rate_limit=settings.provider_rate_limit,
            rate_window=settings.provider_rate_window,
            max_concurrency=settings.provider_max_concurrency,
        )
        nim_settings = NimSettings()
        provider = NvidiaNimProvider(config=config, nim_settings=nim_settings)
    elif provider_type in ("openrouter", "open_router"):
        config = ProviderConfig(
            api_key=settings.openrouter_api_key,
            base_url="https://openrouter.ai/api/v1",
            rate_limit=settings.provider_rate_limit,
            rate_window=settings.provider_rate_window,
            max_concurrency=settings.provider_max_concurrency,
        )
        provider = OpenRouterProvider(config=config)
    elif provider_type == "lmstudio":
        config = ProviderConfig(
            api_key="lm-studio",
            base_url=settings.lm_studio_base_url or "http://localhost:1234/v1",
            rate_limit=settings.provider_rate_limit,
            rate_window=settings.provider_rate_window,
            max_concurrency=settings.provider_max_concurrency,
        )
        provider = LMStudioProvider(config=config)
    else:
        raise ValueError(f"Unknown provider type: {provider_type}")

    _provider_cache[provider_type] = provider
    logger.info(f"Created provider: {provider_type}")
    return provider


def resolve_model_mapping(model_name: str) -> tuple[str, ModelMapping]:
    """Resolve provider type and model mapping from a model name."""
    tier = detect_tier(model_name).value
    config = get_config()
    mapping = config.model_mappings.get(tier, config.model_mappings["default"])
    return mapping.provider_type, mapping


def get_provider_for_model(model_name: str) -> BaseProvider:
    """Get provider instance for the given model name (resolves via tier)."""
    provider_type, _ = resolve_model_mapping(model_name)
    return get_provider_for_type(provider_type)


def resolve_target_model(model_name: str) -> str:
    """Resolve the target provider model name from a Claude model name."""
    _, mapping = resolve_model_mapping(model_name)
    return mapping.model_name


async def stream_to_anthropic_response(
    provider: BaseProvider,
    req: AnthropicRequest,
    input_tokens: int,
    request_id: str,
    *,
    target_model: str | None = None,
) -> dict[str, Any]:
    """Collect provider stream events into an Anthropic response dict."""
    content_blocks: list[dict[str, Any]] = []
    stop_reason = "end_turn"
    usage = {"input_tokens": input_tokens, "output_tokens": 0}
    message_id = None
    model_name = target_model or req.model
    current_block: dict[str, Any] | None = None

    if target_model is not None:
        req = req.model_copy(update={"model": target_model})

    async for event_str in provider.stream_response(
        req, input_tokens=input_tokens, request_id=request_id
    ):
        for line in event_str.split("\n"):
            line = line.strip()
            if not line.startswith("data: "):
                continue
            try:
                data = json.loads(line[6:])
            except json.JSONDecodeError:
                continue

            t = data.get("type")

            if t == "message_start":
                msg = data.get("message", {})
                message_id = msg.get("id")
                model_name = msg.get("model", model_name)
            elif t == "content_block_start":
                block_info = data.get("content_block", {})
                bt = block_info.get("type")
                current_block = {"type": bt}
                if bt == "text":
                    current_block["text"] = ""
                elif bt == "thinking":
                    current_block["thinking"] = ""
                elif bt == "tool_use":
                    current_block["id"] = block_info.get("id", "")
                    current_block["name"] = block_info.get("name", "")
                    current_block["input"] = ""
            elif t == "content_block_delta":
                delta = data.get("delta", {})
                dt = delta.get("type")
                if current_block:
                    if dt == "text_delta" and current_block["type"] == "text":
                        current_block["text"] += delta.get("text", "")
                    elif dt == "thinking_delta" and current_block["type"] == "thinking":
                        current_block["thinking"] += delta.get("thinking", "")
                    elif (
                        dt == "input_json_delta" and current_block["type"] == "tool_use"
                    ):
                        current_block["input"] += delta.get("partial_json", "")
            elif t == "content_block_stop":
                if current_block is not None:
                    if current_block["type"] == "tool_use" and isinstance(
                        current_block.get("input"), str
                    ):
                        with suppress(json.JSONDecodeError, TypeError):
                            current_block["input"] = json.loads(current_block["input"])
                    content_blocks.append(current_block)
                    current_block = None
            elif t == "message_delta":
                stop_reason = data.get("delta", {}).get("stop_reason", "end_turn")
                u = data.get("usage", {})
                usage = {
                    "input_tokens": input_tokens,
                    "output_tokens": u.get("output_tokens", 0),
                }

    return {
        "id": message_id or f"msg_{int(time.time() * 1000)}",
        "type": "message",
        "role": "assistant",
        "content": content_blocks,
        "model": model_name,
        "stop_reason": stop_reason,
        "stop_sequence": None,
        "usage": usage,
    }
