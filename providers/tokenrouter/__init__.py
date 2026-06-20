"""TokenRouter (OpenAI-compatible) adapter."""

from providers.defaults import TOKENROUTER_DEFAULT_BASE

from .client import TokenrouterProvider

__all__ = ["TOKENROUTER_DEFAULT_BASE", "TokenrouterProvider"]
