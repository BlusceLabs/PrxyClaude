"""BAI (OpenAI-compatible) adapter."""

from providers.defaults import BAI_DEFAULT_BASE

from .client import BaiProvider

__all__ = ["BAI_DEFAULT_BASE", "BaiProvider"]
