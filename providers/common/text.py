"""Text extraction utilities for content blocks."""

from enum import StrEnum
from typing import Any


class ContentType(StrEnum):
    """Content block types."""

    THINKING = "thinking"
    TEXT = "text"


def extract_text_from_content(content: Any) -> str:
    """Extract text from various content formats.

    Handles:
    - Plain strings
    - Lists of content blocks (Anthropic format)
    - Objects with 'text' attribute
    """
    if isinstance(content, str):
        return content

    if isinstance(content, list):
        parts: list[str] = []
        for block in content:
            if hasattr(block, "text"):
                parts.append(block.text)
            elif isinstance(block, dict) and "text" in block:
                parts.append(block["text"])
            elif isinstance(block, dict) and "content" in block:
                extracted = extract_text_from_content(block["content"])
                if extracted:
                    parts.append(extracted)
        return "".join(parts)

    if hasattr(content, "text"):
        return content.text

    if hasattr(content, "content"):
        return extract_text_from_content(content.content)

    return ""


def truncate_text(text: str, max_length: int = 100, suffix: str = "...") -> str:
    """Truncate text to max_length, adding suffix if truncated."""
    if len(text) <= max_length:
        return text
    return text[: max_length - len(suffix)] + suffix


def count_tokens_approximate(text: str) -> int:
    """Approximate token count (roughly 4 chars per token for English)."""
    return max(1, len(text) // 4)
