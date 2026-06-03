"""API Models for Anthropic-compatible requests."""

from typing import Any

from pydantic import BaseModel


class Message(BaseModel):
    """A single message in the conversation."""

    role: str
    content: str | list[dict[str, Any]]


class MessagesRequest(BaseModel):
    """Anthropic Messages API request format."""

    model: str
    messages: list[Message]
    max_tokens: int | None = 8192
    temperature: float | None = None
    top_p: float | None = None
    top_k: int | None = None
    stream: bool | None = False
    system: str | list[dict[str, Any]] | None = None
    tools: list[dict[str, Any]] | None = None
    tool_choice: dict[str, Any] | None = None
    thinking: dict[str, Any] | None = None
    metadata: dict[str, Any] | None = None
    resolved_provider_model: str | None = None

    class Config:
        extra = "allow"


class TokenCountRequest(BaseModel):
    """Token count request format."""

    model: str
    messages: list[Message]
    system: str | list[dict[str, Any]] | None = None
    tools: list[dict[str, Any]] | None = None
