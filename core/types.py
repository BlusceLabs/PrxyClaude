"""PrxyClaude · Core Types (Pydantic models)"""

from __future__ import annotations

from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field

# ─── Enums ──────────────────────────────────────────────────────────────────


class ModelTier(StrEnum):
    opus = "opus"
    sonnet = "sonnet"
    haiku = "haiku"


class CircuitState(StrEnum):
    closed = "closed"
    open = "open"
    half_open = "half-open"


class ProviderType(StrEnum):
    openrouter = "openrouter"
    nvidia_nim = "nvidia_nim"
    groq = "groq"
    openai_compat = "openai_compat"
    lmstudio = "lmstudio"
    ollama = "ollama"
    mistral = "mistral"
    together = "together"
    anthropic = "anthropic"


# ─── Anthropic Request Models ───────────────────────────────────────────────


class ContentBlock(BaseModel):
    type: str
    text: str | None = None
    id: str | None = None
    name: str | None = None
    input: Any | None = None
    tool_use_id: str | None = None
    content: str | list[ContentBlock] | None = None
    source: dict[str, Any] | None = None
    thinking: str | None = None
    signature: str | None = None


class SystemBlock(BaseModel):
    type: str = "text"
    text: str
    cache_control: Any | None = None


class Tool(BaseModel):
    name: str
    description: str | None = None
    input_schema: dict[str, Any] = Field(default_factory=dict)


class ToolChoice(BaseModel):
    type: str = "auto"
    name: str | None = None


class AnthropicMessage(BaseModel):
    role: str
    content: str | list[ContentBlock]


class AnthropicRequest(BaseModel):
    model: str
    messages: list[AnthropicMessage]
    max_tokens: int | None = 8192
    temperature: float | None = None
    top_p: float | None = None
    top_k: int | None = None
    stream: bool | None = False
    system: str | list[SystemBlock] | None = None
    tools: list[Tool] | None = None
    tool_choice: ToolChoice | None = None
    thinking: Any | None = None
    metadata: Any | None = None


# ─── OpenAI Request Models ──────────────────────────────────────────────────


class OpenAIMessage(BaseModel):
    role: str
    content: str | None = None
    tool_calls: list[dict[str, Any]] | None = None
    tool_call_id: str | None = None


class OpenAITool(BaseModel):
    type: str = "function"
    function: dict[str, Any]


class OpenAIRequest(BaseModel):
    model: str
    messages: list[OpenAIMessage]
    max_tokens: int = 8192
    temperature: float | None = None
    top_p: float | None = None
    stream: bool = False
    tools: list[OpenAITool] | None = None
    tool_choice: str | None = None


# ─── Metrics Models ─────────────────────────────────────────────────────────


class ProviderMetrics(BaseModel):
    requests: int = 0
    successes: int = 0
    failures: int = 0
    cached_hits: int = 0
    total_tokens_in: int = 0
    total_tokens_out: int = 0
    avg_latency_ms: float = 0
    latencies: list[float] = []
    last_error_msg: str | None = None


class GlobalMetrics(BaseModel):
    total_requests: int = 0
    cached_requests: int = 0
    queued_requests: int = 0
    started_at: float = Field(default_factory=lambda: __import__("time").time() * 1000)
    providers: dict[str, ProviderMetrics] = {}


# ─── Queue Models ───────────────────────────────────────────────────────────


class QueuedRequest(BaseModel):
    id: str
    priority: int
    tier: str
    created_at: float
    timeout_at: float
