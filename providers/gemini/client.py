"""Gemini provider implementation (OpenAI-compatible endpoint)."""

from __future__ import annotations

from typing import Any

from providers.base import ProviderConfig
from providers.defaults import GEMINI_DEFAULT_BASE
from providers.model_listing import ProviderModelInfo, model_infos_from_ids
from providers.openai_compat import OpenAIChatTransport

from .request import build_request_body

_GEMINI_MODEL_PREFIX = "models/"


class GeminiProvider(OpenAIChatTransport):
    """Gemini provider using the OpenAI-compatible chat completions API.

    https://ai.google.dev/gemini-api/docs/openai
    """

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="GEMINI",
            base_url=config.base_url or GEMINI_DEFAULT_BASE,
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
        """Return model ids with the ``models/`` prefix stripped."""
        try:
            raw = await super().list_model_ids()
        except Exception:
            return frozenset()
        return frozenset(
            _strip_model_prefix(mid) for mid in raw
        )

    async def list_model_infos(self) -> frozenset[ProviderModelInfo]:
        """Return model infos with the ``models/`` prefix stripped."""
        try:
            raw = await super().list_model_infos()
        except Exception:
            return frozenset()
        return frozenset(
            ProviderModelInfo(
                model_id=_strip_model_prefix(info.model_id),
                supports_thinking=info.supports_thinking,
            )
            for info in raw
        )


def _strip_model_prefix(model_id: str) -> str:
    if model_id.startswith(_GEMINI_MODEL_PREFIX):
        return model_id[len(_GEMINI_MODEL_PREFIX):]
    return model_id
