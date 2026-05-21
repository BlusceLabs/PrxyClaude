"""Request builder for Anthropic Direct provider."""

from typing import Any

from loguru import logger

from core.anthropic.native_messages_request import (
    build_base_native_anthropic_request_body,
)


def build_request_body(
    request_data: Any,
    *,
    thinking_enabled: bool,
    default_max_tokens: int,
) -> dict:
    """Build native Anthropic Messages API request body."""
    logger.debug(
        "ANTHROPIC_REQUEST: build start model={} msgs={}",
        getattr(request_data, "model", "?"),
        len(getattr(request_data, "messages", [])),
    )
    body = build_base_native_anthropic_request_body(
        request_data,
        default_max_tokens=default_max_tokens,
        thinking_enabled=thinking_enabled,
    )
    body["stream"] = True

    logger.debug(
        "ANTHROPIC_REQUEST: build done model={} msgs={} tools={}",
        body.get("model"),
        len(body.get("messages", [])),
        len(body.get("tools", [])),
    )
    return body
