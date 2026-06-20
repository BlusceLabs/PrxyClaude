"""ModelScope (OpenAI-compatible) adapter."""

from providers.defaults import MODELSCOPE_DEFAULT_BASE

from .client import ModelScopeProvider

__all__ = ["MODELSCOPE_DEFAULT_BASE", "ModelScopeProvider"]
