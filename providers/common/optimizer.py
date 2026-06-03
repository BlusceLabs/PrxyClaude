"""PrxyClaude · Request Optimization Interceptors

Intercepts trivial API calls and responds locally without hitting the provider:
1. Quota probes - responds with mock quota info
2. Title generation - responds with placeholder
3. Prefix detection - responds with empty match
4. Suggestion mode - responds with no suggestions
5. Filepath extraction - responds with empty paths
"""

from __future__ import annotations

from typing import Any

from loguru import logger


def is_quota_probe(messages: list[dict]) -> bool:
    """Detect quota/probe requests (short messages asking about limits)."""
    if len(messages) > 2:
        return False
    text = " ".join(
        m.get("content", "") if isinstance(m.get("content"), str) else ""
        for m in messages
    ).lower()
    markers = [
        "quota",
        "rate limit",
        "remaining",
        "credits",
        "usage",
        "how many requests",
    ]
    return any(m in text for m in markers) and len(text) < 200


def is_title_generation(messages: list[dict]) -> bool:
    """Detect title/summary generation requests."""
    if len(messages) > 2:
        return False
    text = " ".join(
        m.get("content", "") if isinstance(m.get("content"), str) else ""
        for m in messages
    ).lower()
    markers = [
        "generate a title",
        "create a title",
        "summarize this conversation",
        "title for",
    ]
    return any(m in text for m in markers) and len(text) < 200


def is_prefix_detection(messages: list[dict]) -> bool:
    """Detect autocomplete/prefix detection requests."""
    if len(messages) != 1:
        return False
    content = messages[0].get("content", "")
    if not isinstance(content, str):
        return False
    # Very short messages that look like partial inputs
    return len(content) < 10 and not content.endswith((".", "!", "?"))


def is_suggestion_mode(messages: list[dict]) -> bool:
    """Detect suggestion/continuation mode requests."""
    if len(messages) > 3:
        return False
    text = " ".join(
        m.get("content", "") if isinstance(m.get("content"), str) else ""
        for m in messages
    ).lower()
    markers = ["suggest", "continue", "what should i", "next steps", "recommend"]
    return any(m in text for m in markers) and len(text) < 150


def is_filepath_extraction(messages: list[dict]) -> bool:
    """Detect filepath extraction/mock requests."""
    if len(messages) > 2:
        return False
    text = " ".join(
        m.get("content", "") if isinstance(m.get("content"), str) else ""
        for m in messages
    ).lower()
    markers = ["extract file", "find file", "file path", "filepath", "list files"]
    return any(m in text for m in markers) and len(text) < 200


def intercept_request(
    messages: list[dict],
    settings: Any = None,
) -> dict[str, Any] | None:
    """
    Check if the request can be handled locally.
    Returns a mock Anthropic response dict if intercepted, None otherwise.
    """
    # Check each optimization category
    if (
        settings
        and getattr(settings, "enable_quota_probe_mock", True)
        and is_quota_probe(messages)
    ):
        logger.debug("[optimize] intercepted quota probe")
        return _mock_quota_response(messages)

    if (
        settings
        and getattr(settings, "enable_title_generation_skip", True)
        and is_title_generation(messages)
    ):
        logger.debug("[optimize] intercepted title generation")
        return _mock_title_response(messages)

    if (
        settings
        and getattr(settings, "enable_prefix_detection", True)
        and is_prefix_detection(messages)
    ):
        logger.debug("[optimize] intercepted prefix detection")
        return _mock_prefix_response(messages)

    if (
        settings
        and getattr(settings, "enable_suggestion_mode_skip", True)
        and is_suggestion_mode(messages)
    ):
        logger.debug("[optimize] intercepted suggestion mode")
        return _mock_suggestion_response(messages)

    if (
        settings
        and getattr(settings, "enable_filepath_extraction_mock", True)
        and is_filepath_extraction(messages)
    ):
        logger.debug("[optimize] intercepted filepath extraction")
        return _mock_filepath_response(messages)

    return None


# ─── Mock Responses ──────────────────────────────────────────────────────────


def _mock_quota_response(messages: list[dict]) -> dict:
    return {
        "id": "msg_optimized_quota",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "Quota is unlimited through the proxy."}],
        "model": "optimized",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }


def _mock_title_response(messages: list[dict]) -> dict:
    text = messages[0].get("content", "Conversation") if messages else "Conversation"
    if isinstance(text, list):
        text = "Conversation"
    title = text[:60].strip()
    return {
        "id": "msg_optimized_title",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": title}],
        "model": "optimized",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }


def _mock_prefix_response(messages: list[dict]) -> dict:
    return {
        "id": "msg_optimized_prefix",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": ""}],
        "model": "optimized",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }


def _mock_suggestion_response(messages: list[dict]) -> dict:
    return {
        "id": "msg_optimized_suggest",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": ""}],
        "model": "optimized",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }


def _mock_filepath_response(messages: list[dict]) -> dict:
    return {
        "id": "msg_optimized_filepath",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "No file paths found."}],
        "model": "optimized",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }
