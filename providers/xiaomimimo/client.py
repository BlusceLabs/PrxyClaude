"""XiaoMiMiMo provider implementation (OpenAI-compatible chat completions).

API at ``https://token-plan-sgp.xiaomimimo.com/v1/chat/completions``.
Supports reasoning (thinking) content and tool calls.
"""

from __future__ import annotations

from typing import Any

from providers.base import ProviderConfig
from providers.defaults import XIAOMIMIMO_DEFAULT_BASE
from providers.transports.openai_chat import OpenAIChatTransport

from .request import build_request_body


class XiaoMiMiMoProvider(OpenAIChatTransport):
    """XiaoMiMiMo API using ``https://token-plan-sgp.xiaomimimo.com/v1/chat/completions``."""

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="XIAOMIMIMO",
            base_url=config.base_url or XIAOMIMIMO_DEFAULT_BASE,
            api_key=config.api_key,
        )

    def _build_request_body(
        self, request: Any, thinking_enabled: bool | None = None
    ) -> dict:
        return build_request_body(
            request,
            thinking_enabled=self._is_thinking_enabled(request, thinking_enabled),
        )
