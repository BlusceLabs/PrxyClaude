"""Heuristic tool call parser for detecting tool calls in text content."""

from __future__ import annotations

import json
import re
import uuid
from typing import Any


class HeuristicToolParser:
    """Parser that detects tool calls in text using heuristics.

    Looks for patterns like:
    - JSON with "name" and "input" fields
    - XML-style tool invocations
    """

    def __init__(self) -> None:
        self._buffer = ""

    def feed(self, text: str) -> tuple[str, list[dict[str, Any]]]:
        """Process text and return (filtered_text, detected_tools)."""
        self._buffer += text
        tools: list[dict[str, Any]] = []
        filtered = text

        # Look for JSON tool calls
        json_pattern = (
            r'\{[^{}]*"name"\s*:\s*"[^"]+"\s*,\s*"input"\s*:\s*\{[^{}]*\}[^{}]*\}'
        )
        matches = list(re.finditer(json_pattern, self._buffer))

        for match in matches:
            try:
                tool_data = json.loads(match.group())
                if "name" in tool_data and "input" in tool_data:
                    tools.append(
                        {
                            "id": f"tool_{uuid.uuid4().hex[:8]}",
                            "name": tool_data["name"],
                            "input": tool_data["input"],
                        }
                    )
                    filtered = filtered.replace(match.group(), "")
            except json.JSONDecodeError:
                continue

        # Look for XML-style tool calls
        xml_pattern = r'<tool\s+name="([^"]+)">(.*?)</tool>'
        for match in re.finditer(xml_pattern, self._buffer, re.DOTALL):
            tool_name = match.group(1)
            tool_input_str = match.group(2)
            try:
                tool_input = json.loads(tool_input_str)
            except json.JSONDecodeError:
                tool_input = {"raw": tool_input_str}
            tools.append(
                {
                    "id": f"tool_{uuid.uuid4().hex[:8]}",
                    "name": tool_name,
                    "input": tool_input,
                }
            )
            filtered = filtered.replace(match.group(), "")

        return filtered.strip(), tools

    def flush(self) -> list[dict[str, Any]]:
        """Return any remaining tool calls in the buffer."""
        if not self._buffer:
            return []

        tools: list[dict[str, Any]] = []
        json_pattern = (
            r'\{[^{}]*"name"\s*:\s*"[^"]+"\s*,\s*"input"\s*:\s*\{[^{}]*\}[^{}]*\}'
        )

        for match in re.finditer(json_pattern, self._buffer):
            try:
                tool_data = json.loads(match.group())
                if "name" in tool_data and "input" in tool_data:
                    tools.append(
                        {
                            "id": f"tool_{uuid.uuid4().hex[:8]}",
                            "name": tool_data["name"],
                            "input": tool_data["input"],
                        }
                    )
            except json.JSONDecodeError:
                continue

        self._buffer = ""
        return tools
