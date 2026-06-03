"""API Models for OpenAI-compatible requests."""

from typing import Any

from pydantic import BaseModel


class OpenAIMessage(BaseModel):
    """A single message in OpenAI format."""

    role: str
    content: str | list[dict[str, Any]] | None = None
    name: str | None = None
    tool_calls: list[dict[str, Any]] | None = None
    tool_call_id: str | None = None


class OpenAITool(BaseModel):
    """OpenAI tool definition."""

    type: str = "function"
    function: dict[str, Any]


class OpenAIChatCompletionRequest(BaseModel):
    """OpenAI Chat Completion API request format."""

    model: str
    messages: list[OpenAIMessage]
    temperature: float | None = None
    top_p: float | None = None
    n: int | None = None
    stream: bool | None = False
    stop: str | list[str] | None = None
    max_tokens: int | None = None
    presence_penalty: float | None = None
    frequency_penalty: float | None = None
    logit_bias: dict[str, float] | None = None
    user: str | None = None
    tools: list[OpenAITool] | None = None
    tool_choice: str | dict[str, Any] | None = None
    response_format: dict[str, Any] | None = None
    seed: int | None = None

    class Config:
        extra = "allow"
