"""Common utility functions for providers."""

from typing import Any


def set_if_not_none(d: dict[str, Any], key: str, value: Any) -> None:
    """Set a key in dict only if value is not None."""
    if value is not None:
        d[key] = value
