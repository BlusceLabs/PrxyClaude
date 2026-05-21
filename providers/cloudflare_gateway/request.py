"""Request builder for Cloudflare AI Gateway provider."""

from typing import Any

from loguru import logger

from config.constants import ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS
from core.anthropic.native_messages_request import (
    build_base_native_anthropic_request_body,
)


def build_request_body(request_data: Any, *, thinking_enabled: bool) -> dict:
    """Build a native Anthropic request body.

    The gateway proxies Anthropic-format requests as-is, so we pass through
    the native body without conversion.
    """
    logger.debug(
        "CF_GATEWAY_REQUEST: start model={} msgs={}",
        getattr(request_data, "model", "?"),
        len(getattr(request_data, "messages", [])),
    )
    body = build_base_native_anthropic_request_body(
        request_data,
        default_max_tokens=ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS,
        thinking_enabled=thinking_enabled,
    )
    logger.debug(
        "CF_GATEWAY_REQUEST: done model={} msgs={} tools={}",
        body.get("model"),
        len(body.get("messages", [])),
        len(body.get("tools", [])),
    )
    return body
