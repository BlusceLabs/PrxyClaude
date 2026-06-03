"""PrxyClaude · Model tier detection."""

from __future__ import annotations

import re

from core.types import ModelTier


def detect_tier(model_name: str) -> ModelTier:
    if re.search(r"opus", model_name, re.IGNORECASE):
        return ModelTier.opus
    if re.search(r"haiku", model_name, re.IGNORECASE):
        return ModelTier.haiku
    return ModelTier.sonnet
