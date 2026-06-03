"""API Response models."""

from typing import Any

from pydantic import BaseModel, Field


class Usage(BaseModel):
    """Token usage information."""

    input_tokens: int = 0
    output_tokens: int = 0


class MessagesResponse(BaseModel):
    """Anthropic Messages API response format."""

    id: str
    model: str
    role: str = "assistant"
    content: list[dict[str, Any]]
    stop_reason: str = "end_turn"
    stop_sequence: str | None = None
    usage: Usage = Field(default_factory=Usage)


class TokenCountResponse(BaseModel):
    """Token count response."""

    input_tokens: int
