"""Z.ai provider implementation."""

from __future__ import annotations

from typing import Any

from providers.base import ProviderConfig
from providers.defaults import ZAI_DEFAULT_BASE
from providers.model_listing import ProviderModelInfo, model_infos_from_ids
from providers.openai_compat import OpenAIChatTransport

from .request import build_request_body

# z.ai accepts model names in /chat/completions that are not listed in /models.
# Known -flash variants accepted at inference time:
_KNOWN_FLASH_VARIANTS = frozenset({"glm-4.5-flash", "glm-4.7-flash"})


class ZAIProvider(OpenAIChatTransport):
    """Z.ai provider using the OpenAI-compatible chat completions API."""

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
        """Return advertised model ids plus known -flash inference variants."""
        try:
            base = await super().list_model_ids()
        except Exception:
            base = frozenset()
        return base | _KNOWN_FLASH_VARIANTS

    async def list_model_infos(self) -> frozenset[ProviderModelInfo]:
        """Return model infos including known -flash variants."""
        try:
            base = await super().list_model_infos()
        except Exception:
            base = frozenset()
        return base | model_infos_from_ids(_KNOWN_FLASH_VARIANTS)
