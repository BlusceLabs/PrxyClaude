"""NVIDIA NIM specific configuration."""

from pydantic import BaseModel


class NimSettings(BaseModel):
    """Settings specific to NVIDIA NIM provider."""

    temperature: float | None = 0.7
    top_p: float | None = 0.9
    top_k: int | None = 50
    max_tokens: int | None = 4096
    stop: list[str] | None = None
    presence_penalty: float = 0.0
    frequency_penalty: float = 0.0
    seed: int | None = None
    parallel_tool_calls: bool = True
    min_p: float = 0.0
    repetition_penalty: float = 1.0
    min_tokens: int = 0
    chat_template: str | None = None
    request_id: str | None = None
    return_tokens_as_token_ids: bool = False
    include_stop_str_in_output: bool = False
    ignore_eos: bool = False
    reasoning_effort: str | None = None
    include_reasoning: bool = True
