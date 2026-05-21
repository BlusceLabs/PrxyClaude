"""Anthropic Direct provider implementation (native Anthropic Messages API)."""

from __future__ import annotations

from typing import Any

from config.constants import ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS
from providers.anthropic_messages import AnthropicMessagesTransport
from providers.base import ProviderConfig
from providers.defaults import ANTHROPIC_DEFAULT_BASE

from .request import build_request_body


class AnthropicDirectProvider(AnthropicMessagesTransport):
    """Anthropic provider using the native Messages API directly.

    https://docs.anthropic.com/en/api/messages
    """

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="ANTHROPIC",
            default_base_url=ANTHROPIC_DEFAULT_BASE,
        )

    def _build_request_body(
        self, request: Any, thinking_enabled: bool | None = None
    ) -> dict:
        return build_request_body(
            request,
            thinking_enabled=self._is_thinking_enabled(request, thinking_enabled),
            default_max_tokens=ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS,
        )

    def _request_headers(self) -> dict[str, str]:
        return {
            "Accept": "text/event-stream",
            "Content-Type": "application/json",
            "x-api-key": self._api_key,
            "anthropic-version": "2023-06-01",
        }

    def _model_list_headers(self) -> dict[str, str]:
        return {"x-api-key": self._api_key, "anthropic-version": "2023-06-01"}
