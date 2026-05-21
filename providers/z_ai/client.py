"""Z.ai provider implementation."""

from __future__ import annotations

from collections.abc import Iterable
from typing import Any

from providers.base import ProviderConfig
from providers.defaults import ZAI_DEFAULT_BASE
from providers.model_listing import ProviderModelInfo
from providers.openai_compat import OpenAIChatTransport

from .request import build_request_body


class ZAIProvider(OpenAIChatTransport):
    """Z.ai provider using the OpenAI-compatible chat completions API.

    Model listing is disabled — z.ai does not reliably expose all deployable
    models through the OpenAI ``/models`` endpoint. Set your model directly
    in ``MODEL=z_ai/<model-name>``.
    """

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="ZAI",
            base_url=config.base_url or ZAI_DEFAULT_BASE,
            api_key=config.api_key,
        )

    def _build_request_body(
        self, request: Any, thinking_enabled: bool | None = None
    ) -> dict:
        return build_request_body(
            request,
            thinking_enabled=self._is_thinking_enabled(request, thinking_enabled),
        )

    async def list_model_ids(self) -> frozenset[str]:
        return frozenset()

    async def list_model_infos(self) -> frozenset[ProviderModelInfo]:
        return frozenset()
