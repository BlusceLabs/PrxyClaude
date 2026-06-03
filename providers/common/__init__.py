"""PrxyClaude · Providers Common utilities

Shared utilities for all provider implementations.
"""

from providers.common.sse_builder import SSEBuilder as SSEBuilder
from providers.common.text import ContentType as ContentType
from providers.common.text import extract_text_from_content as extract_text_from_content
from providers.common.thinking_parser import ThinkTagParser as ThinkTagParser
from providers.common.thinking_parser import (
    extract_thinking_from_text as extract_thinking_from_text,
)
from providers.common.tool_parser import HeuristicToolParser as HeuristicToolParser


def append_request_id(message: str, request_id: str | None) -> str:
    """Append request ID to error message for traceability."""
    if request_id:
        return f"{message} (request_id={request_id})"
    return message


def get_user_facing_error_message(exc: Exception, read_timeout_s: float = 300.0) -> str:
    """Extract user-facing error message from exception."""
    exc_name = type(exc).__name__

    # OpenAI-specific error handling
    if "openai" in str(type(exc).__module__).lower():
        import openai

        if isinstance(exc, openai.APITimeoutError):
            return f"Request timed out after {read_timeout_s}s. The provider may be slow or unreachable."
        if isinstance(exc, openai.APIConnectionError):
            return "Failed to connect to the API. Check your network connection."
        if isinstance(exc, openai.RateLimitError):
            return "Rate limit exceeded. Please try again later."
        if isinstance(exc, openai.AuthenticationError):
            return "Authentication failed. Check your API key."
        if isinstance(exc, openai.BadRequestError):
            return f"Invalid request: {exc}"
        if isinstance(exc, openai.APIStatusError):
            return f"API error (status {exc.status_code}): {exc.message}"

    return f"{exc_name}: {exc}"


def map_error(exc: Exception) -> Exception:
    """Map provider-specific exceptions to our exception types."""
    from providers.exceptions import (
        APIError,
        AuthenticationError,
        InvalidRequestError,
        OverloadedError,
        RateLimitError,
    )

    try:
        import openai

        if isinstance(exc, openai.RateLimitError):
            return RateLimitError(str(exc), raw_error=exc)
        if isinstance(exc, openai.AuthenticationError):
            return AuthenticationError(str(exc), raw_error=exc)
        if isinstance(exc, openai.BadRequestError):
            return InvalidRequestError(str(exc), raw_error=exc)
        if isinstance(exc, openai.APITimeoutError):
            return APIError(f"Timeout: {exc}", status_code=504, raw_error=exc)
        if isinstance(exc, openai.APIConnectionError):
            return APIError(f"Connection error: {exc}", raw_error=exc)
        if isinstance(exc, openai.APIStatusError):
            if exc.status_code == 429:
                return RateLimitError(str(exc), raw_error=exc)
            if exc.status_code == 529:
                return OverloadedError(str(exc), raw_error=exc)
            return APIError(str(exc), status_code=exc.status_code, raw_error=exc)
    except ImportError:
        pass

    return APIError(str(exc), raw_error=exc)


def map_stop_reason(finish_reason: str | None) -> str:
    """Map OpenAI stop reasons to Anthropic stop reasons."""
    if finish_reason is None:
        return "end_turn"
    mapping = {
        "stop": "end_turn",
        "length": "max_tokens",
        "tool_calls": "tool_use",
        "content_filter": "end_turn",
    }
    return mapping.get(finish_reason, "end_turn")
