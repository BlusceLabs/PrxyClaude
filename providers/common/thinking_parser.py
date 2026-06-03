"""PrxyClaude · Thinking Token Parser

Parses thinking/reasoning tokens from provider responses and converts them
to native Claude thinking blocks. Handles:
- <think> ...  tags
- reasoning_content fields in OpenAI responses
- thinking content blocks
"""

from __future__ import annotations

import re
from typing import Any

from providers.common.text import ContentType


class ThinkTagParser:
    """Streaming parser that detects <think>...</think> blocks in content chunks.

    Yields parsed content parts with their type (THINKING or TEXT).
    """

    def __init__(self) -> None:
        self._buffer = ""
        self._in_thinking = False

    def feed(self, chunk: str) -> list[Any]:
        """Process a chunk and return any complete content parts."""
        from dataclasses import dataclass

        @dataclass
        class ContentPart:
            type: ContentType
            content: str

        parts: list[ContentPart] = []
        self._buffer += chunk

        while True:
            if self._in_thinking:
                end_idx = self._buffer.find("</think>")
                if end_idx == -1:
                    break
                thinking_content = self._buffer[:end_idx]
                self._buffer = self._buffer[end_idx + 8 :]
                self._in_thinking = False
                parts.append(ContentPart(ContentType.THINKING, thinking_content))
            else:
                start_idx = self._buffer.find("<think>")
                if start_idx == -1:
                    if self._buffer:
                        parts.append(ContentPart(ContentType.TEXT, self._buffer))
                        self._buffer = ""
                    break
                if start_idx > 0:
                    parts.append(
                        ContentPart(ContentType.TEXT, self._buffer[:start_idx])
                    )
                self._buffer = self._buffer[start_idx + 7 :]
                self._in_thinking = True

        return parts

    def flush(self) -> Any:
        """Return any remaining buffered content."""
        from dataclasses import dataclass

        @dataclass
        class ContentPart:
            type: ContentType
            content: str

        if self._buffer:
            content = self._buffer
            self._buffer = ""
            if self._in_thinking:
                return ContentPart(ContentType.THINKING, content)
            return ContentPart(ContentType.TEXT, content)
        return None


def extract_thinking_from_text(text: str) -> tuple[str, str]:
    """
    Extract thinking content from text containing <think> tags.
    Returns (thinking_content, remaining_text).
    """
    thinking_pattern = r"<think>(.*?)</think>"
    match = re.search(thinking_pattern, text, re.DOTALL)

    if match:
        thinking = match.group(1).strip()
        remaining = text[: match.start()] + text[match.end() :]
        return thinking, remaining.strip()

    return "", text


def parse_reasoning_content(message: dict[str, Any]) -> tuple[str, str]:
    """
    Parse reasoning_content from an OpenAI-style message delta.
    Returns (thinking_text, content_text).
    """
    thinking = ""
    content = message.get("content", "") or ""

    # Check for reasoning_content field
    reasoning = message.get("reasoning_content")
    if reasoning:
        thinking = reasoning if isinstance(reasoning, str) else str(reasoning)

    # Also check for <think> tags in content
    if "<think>" in content:
        extracted, content = extract_thinking_from_text(content)
        if extracted:
            thinking = extracted

    return thinking, content


def build_thinking_block(
    thinking_text: str, cache_control: dict | None = None
) -> dict[str, Any]:
    """Build a Claude-style thinking content block."""
    block: dict[str, Any] = {
        "type": "thinking",
        "thinking": thinking_text,
    }
    if cache_control:
        block["cache_control"] = cache_control
    return block


def merge_thinking_into_content(
    content_blocks: list[dict[str, Any]],
    thinking_text: str,
) -> list[dict[str, Any]]:
    """
    Insert thinking block at the beginning of content blocks.
    Follows Claude's format: thinking block comes before text block.
    """
    if not thinking_text:
        return content_blocks

    thinking_block = build_thinking_block(thinking_text)

    # Find the first text block and insert thinking before it
    result = []
    inserted = False
    for block in content_blocks:
        if not inserted and block.get("type") == "text":
            result.append(thinking_block)
            inserted = True
        result.append(block)

    # If no text block found, just prepend
    if not inserted:
        result.insert(0, thinking_block)

    return result


def has_thinking_content(message: dict[str, Any]) -> bool:
    """Check if a message has thinking content."""
    if message.get("reasoning_content"):
        return True
    content = message.get("content", "")
    return bool(isinstance(content, str) and "<think>" in content)
