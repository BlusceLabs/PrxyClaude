"""Fireworks provider implementation (Fireworks chat completions API)."""

from __future__ import annotations

from typing import Any

import httpx

from providers.base import ProviderConfig
from providers.defaults import FIREWORKS_DEFAULT_BASE
from providers.model_listing import ProviderModelInfo, model_infos_from_ids
from providers.openai_compat import OpenAIChatTransport

from .request import build_request_body

_FIREWORKS_NATIVE_BASE = "https://api.fireworks.ai"


class FireworksProvider(OpenAIChatTransport):
    """Fireworks provider using the OpenAI-compatible chat completions API.

    https://docs.fireworks.ai/getting-started/quickstart
    """

    def __init__(self, config: ProviderConfig):
        super().__init__(
            config,
            provider_name="FIREWORKS",
            base_url=config.base_url or FIREWORKS_DEFAULT_BASE,
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
        base_url = _FIREWORKS_NATIVE_BASE
        async with httpx.AsyncClient(base_url=base_url) as client:
            response = await client.get(
                "/v1/accounts/fireworks/models",
                params={"filter": "supports_serverless=true", "pageSize": 200},
                headers={"Authorization": f"Bearer {self._api_key}"},
            )
            response.raise_for_status()
            data = response.json()
            models = data.get("models", [])
            return frozenset(
                m["name"] for m in models if isinstance(m.get("name"), str)
            )

    async def list_model_infos(self) -> frozenset[ProviderModelInfo]:
        return model_infos_from_ids(await self.list_model_ids())
