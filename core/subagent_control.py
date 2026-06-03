"""PrxyClaude · Subagent Control

Intercepts Task tool calls and forces run_in_background=False
to prevent runaway subagents from consuming resources.
"""

from __future__ import annotations

import json
from typing import Any


def intercept_subagent_calls(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """
    Scan messages for Task tool calls and force run_in_background=False.
    Returns modified messages.
    """
    modified = []
    for msg in messages:
        if msg.get("role") != "assistant":
            modified.append(msg)
            continue

        content = msg.get("content")
        if not isinstance(content, list):
            modified.append(msg)
            continue

        new_content = []
        for block in content:
            if block.get("type") == "tool_use" and block.get("name") == "Task":
                # Force run_in_background to False
                input_data = block.get("input", {})
                if isinstance(input_data, str):
                    try:
                        input_data = json.loads(input_data)
                    except json.JSONDecodeError:
                        input_data = {}

                input_data["run_in_background"] = False

                new_block = dict(block)
                new_block["input"] = input_data
                new_content.append(new_block)
            else:
                new_content.append(block)

        new_msg = dict(msg)
        new_msg["content"] = new_content
        modified.append(new_msg)

    return modified


def has_subagent_calls(messages: list[dict[str, Any]]) -> bool:
    """Quick check if messages contain Task tool calls."""
    for msg in messages:
        if msg.get("role") != "assistant":
            continue
        content = msg.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") == "tool_use" and block.get("name") == "Task":
                return True
    return False
