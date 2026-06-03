"""Shared message converter for building OpenAI-format request bodies."""

from typing import Any


def build_base_request_body(
    request_data: Any,
    *,
    default_max_tokens: int = 8192,
    include_reasoning_for_openrouter: bool = False,
) -> dict:
    """Build base OpenAI-format request body from Anthropic request.

    Args:
        request_data: Anthropic-format request object
        default_max_tokens: Default max tokens if not specified
        include_reasoning_for_openrouter: Enable OpenRouter reasoning

    Returns:
        OpenAI-format request body dict
    """
    body: dict[str, Any] = {
        "model": request_data.model,
        "messages": [],
    }

    # Add system message if present
    system = getattr(request_data, "system", None)
    if system:
        if isinstance(system, str):
            body["messages"].append({"role": "system", "content": system})
        elif isinstance(system, list):
            system_text = " ".join(
                block.text if hasattr(block, "text") else str(block) for block in system
            )
            body["messages"].append({"role": "system", "content": system_text})

    # Convert Anthropic messages to OpenAI format
    messages = getattr(request_data, "messages", [])
    for msg in messages:
        role = getattr(msg, "role", "user")
        content = getattr(msg, "content", "")

        oai_msg: dict[str, Any] = {"role": role}

        if isinstance(content, str):
            oai_msg["content"] = content
        elif isinstance(content, list):
            text_parts = []
            tool_calls = []
            tool_results = []

            for block in content:
                block_type = getattr(block, "type", None)

                if block_type == "text":
                    text_parts.append(getattr(block, "text", ""))
                elif block_type == "tool_use":
                    tool_id = getattr(block, "id", "")
                    tool_name = getattr(block, "name", "")
                    tool_input = getattr(block, "input", {})
                    import json

                    tool_calls.append(
                        {
                            "id": tool_id,
                            "type": "function",
                            "function": {
                                "name": tool_name,
                                "arguments": json.dumps(tool_input)
                                if tool_input
                                else "{}",
                            },
                        }
                    )
                elif block_type == "tool_result":
                    tool_call_id = getattr(block, "tool_use_id", "")
                    result_content = getattr(block, "content", "") or ""
                    tool_results.append(
                        {
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": result_content,
                        }
                    )

            if text_parts:
                oai_msg["content"] = " ".join(text_parts)
            if tool_calls:
                oai_msg["tool_calls"] = tool_calls

            # Tool results go as separate messages
            for tr in tool_results:
                body["messages"].append(tr)

        body["messages"].append(oai_msg)

    # Add tools if present
    tools = getattr(request_data, "tools", None)
    if tools:
        body["tools"] = []
        for tool in tools:
            body["tools"].append(
                {
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": getattr(tool, "description", "") or "",
                        "parameters": getattr(tool, "input_schema", {}) or {},
                    },
                }
            )

    # Add parameters
    max_tokens = getattr(request_data, "max_tokens", None) or default_max_tokens
    body["max_tokens"] = max_tokens

    temperature = getattr(request_data, "temperature", None)
    if temperature is not None:
        body["temperature"] = temperature

    top_p = getattr(request_data, "top_p", None)
    if top_p is not None:
        body["top_p"] = top_p

    top_k = getattr(request_data, "top_k", None)
    if top_k is not None:
        body["top_k"] = top_k

    return body
