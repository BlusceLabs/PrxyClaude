"""Conversion utilities between OpenAI and Anthropic formats."""

from typing import Any

from .models.anthropic import Message, MessagesRequest
from .models.openai import OpenAIChatCompletionRequest, OpenAIMessage, OpenAITool


def openai_to_anthropic_messages(
    messages: list[OpenAIMessage],
) -> tuple[str | None, list[Message]]:
    """Convert OpenAI messages to Anthropic format.

    Returns:
        Tuple of (system_prompt, messages)
    """
    system_prompt = None
    anthropic_messages: list[Message] = []

    for msg in messages:
        if msg.role == "system":
            # Extract system prompt
            if isinstance(msg.content, str):
                system_prompt = msg.content
            elif isinstance(msg.content, list):
                # Handle list of content blocks
                text_parts = [
                    block.get("text", "")
                    for block in msg.content
                    if isinstance(block, dict) and block.get("type") == "text"
                ]
                system_prompt = " ".join(text_parts) if text_parts else None
        elif msg.role in ("user", "assistant"):
            # Convert to Anthropic message format
            content: str | list[dict[str, Any]]

            if msg.content is None:
                content = ""
            elif isinstance(msg.content, str):
                content = msg.content
            elif isinstance(msg.content, list):
                # Convert content blocks
                content = msg.content
            else:
                content = str(msg.content)

            anthropic_messages.append(Message(role=msg.role, content=content))
        elif msg.role == "tool":
            # Convert tool results to user message with tool_result content
            tool_result_block = {
                "type": "tool_result",
                "tool_use_id": msg.tool_call_id or "",
                "content": msg.content or "",
            }
            # Find or create the last user message
            if anthropic_messages and anthropic_messages[-1].role == "user":
                # Append to existing user message
                existing = anthropic_messages[-1].content
                if isinstance(existing, list):
                    existing.append(tool_result_block)
                else:
                    anthropic_messages[-1].content = [tool_result_block]
            else:
                anthropic_messages.append(
                    Message(role="user", content=[tool_result_block])
                )

    return system_prompt, anthropic_messages


def openai_tools_to_anthropic(
    tools: list[OpenAITool] | None,
) -> list[dict[str, Any]] | None:
    """Convert OpenAI tools to Anthropic format."""
    if not tools:
        return None

    anthropic_tools = []
    for tool in tools:
        if tool.type == "function":
            func = tool.function
            anthropic_tools.append(
                {
                    "name": func.get("name", ""),
                    "description": func.get("description", ""),
                    "input_schema": func.get("parameters", {}),
                }
            )

    return anthropic_tools if anthropic_tools else None


def openai_request_to_anthropic(
    request: OpenAIChatCompletionRequest,
) -> MessagesRequest:
    """Convert OpenAI chat completion request to Anthropic format."""
    system_prompt, messages = openai_to_anthropic_messages(request.messages)

    # Map stop sequences
    if request.stop:
        [request.stop] if isinstance(request.stop, str) else request.stop

    return MessagesRequest(
        model=request.model,
        messages=messages,
        max_tokens=request.max_tokens or 4096,
        temperature=request.temperature,
        top_p=request.top_p,
        stream=request.stream,
        system=system_prompt,
        tools=openai_tools_to_anthropic(request.tools),
    )
