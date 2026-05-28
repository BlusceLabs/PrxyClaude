"""Cloudflare AI Gateway provider implementation."""

from __future__ import annotations

from collections.abc import Iterator
from typing import Any

from providers.anthropic_messages import AnthropicMessagesTransport, StreamChunkMode
from providers.base import ProviderConfig
from providers.defaults import CF_GATEWAY_V1_DEFAULT_BASE
from providers.exceptions import ModelListResponseError
from providers.model_listing import ProviderModelInfo

from .request import build_request_body

_ANTHROPIC_VERSION = "2023-06-01"


class CloudflareGatewayProvider(AnthropicMessagesTransport):
    """Cloudflare AI Gateway using the native Anthropic Messages API endpoint.

    The gateway proxies Anthropic-format requests through Cloudflare. Configure
    your gateway's account ID and gateway name in the base URL, and use
    BYOK (Bring Your Own Key) in the Cloudflare dashboard to store your
    Anthropic API key. The ``CF_AIG_TOKEN`` environment variable should contain
    a Cloudflare API token with AI Gateway read/edit permissions.
    """

    stream_chunk_mode: StreamChunkMode = "event"

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="CF_GATEWAY",
            default_base_url=CF_GATEWAY_V1_DEFAULT_BASE,
        )

    def _build_request_body(
        self, request: Any, thinking_enabled: bool | None = None
    ) -> dict:
        """Build a native Anthropic request body for the gateway."""
        return build_request_body(
            request,
            thinking_enabled=self._is_thinking_enabled(request, thinking_enabled),
        )

    def _request_headers(self) -> dict[str, str]:
        """Return headers for the Cloudflare AI Gateway Anthropic endpoint."""
        return {
            "Accept": "text/event-stream",
            "Content-Type": "application/json",
            "anthropic-version": _ANTHROPIC_VERSION,
            "cf-aig-authorization": f"Bearer {self._api_key}",
        }

    def _model_list_headers(self) -> dict[str, str]:
        """Return auth header for model-list requests."""
        return {"cf-aig-authorization": f"Bearer {self._api_key}"}

    def _send_model_list_request(self) -> Any:
        """Model listing is not supported through the Anthropic gateway endpoint."""
        raise ModelListResponseError(
            "Cloudflare AI Gateway does not support model listing through "
            "the Anthropic endpoint"
        )

    async def list_model_ids(self) -> frozenset[str]:
        """Return empty — model listing is not supported on this endpoint."""
        return frozenset()

    async def list_model_infos(self) -> frozenset[ProviderModelInfo]:
        """Return empty — model listing is not supported on this endpoint."""
        return frozenset()

    def _format_error_message(self, base_message: str, request_id: str | None) -> str:
        """Append request id when available."""
        if request_id:
            return f"{base_message}\nRequest ID: {request_id}"
        return base_message

    def _emit_error_events(
        self,
        *,
        request: Any,
        input_tokens: int,
        error_message: str,
        sent_any_event: bool,
    ) -> Iterator[str]:
        """Emit the required Anthropic SSE error events."""
        from core.anthropic import iter_provider_stream_error_sse_events

        yield from iter_provider_stream_error_sse_events(
            request=request,
            input_tokens=input_tokens,
            error_message=error_message,
            sent_any_event=sent_any_event,
            log_raw_sse_events=self._config.log_raw_sse_events,
        )
