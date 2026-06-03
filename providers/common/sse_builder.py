"""PrxyClaude · SSE Builder (Server-Sent Events)

Utility for building Anthropic SSE events from OpenAI-style responses.
"""

from __future__ import annotations

import json
import uuid
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ToolState:
    """Tracks state for a single tool call."""

    name: str = ""
    id: str = ""
    started: bool = False
    args_buffer: str = ""


@dataclass
class BlockTracker:
    """Tracks content block indices and states."""

    text_index: int = -1
    thinking_index: int = -1
    tool_states: dict[int, ToolState] = field(default_factory=dict)
    _next_index: int = 0
    _task_arg_buffers: dict[int, str] = field(default_factory=dict)

    def allocate_index(self) -> int:
        """Allocate the next available block index."""
        idx = self._next_index
        self._next_index += 1
        return idx

    def register_tool_name(self, index: int, name: str) -> None:
        """Register or update a tool name at given index."""
        if index not in self.tool_states:
            self.tool_states[index] = ToolState()
        self.tool_states[index].name = name

    def buffer_task_args(self, index: int, args: str) -> dict | None:
        """Buffer Task tool arguments and try to parse as JSON."""
        if index not in self._task_arg_buffers:
            self._task_arg_buffers[index] = ""
        self._task_arg_buffers[index] += args

        # Try to parse the accumulated args
        try:
            return json.loads(self._task_arg_buffers[index])
        except json.JSONDecodeError:
            return None

    def flush_task_arg_buffers(self) -> list[tuple[int, str]]:
        """Flush all Task arg buffers as JSON strings."""
        result: list[tuple[int, str]] = []
        for idx, buffer in self._task_arg_buffers.items():
            if buffer:
                result.append((idx, buffer))
        self._task_arg_buffers.clear()
        return result


class SSEBuilder:
    """Class-based SSE event builder for Anthropic format."""

    def __init__(self, message_id: str, model: str, input_tokens: int = 0):
        self.message_id = message_id or f"msg_{uuid.uuid4()}"
        self.model = model
        self.input_tokens = input_tokens
        self.output_tokens = 0
        self.blocks = BlockTracker()
        self._started = False

    def message_start(self) -> str:
        """Build message_start event."""
        self._started = True
        event = {
            "type": "message_start",
            "message": {
                "id": self.message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": self.model,
                "stop_reason": None,
                "usage": {
                    "input_tokens": self.input_tokens,
                    "output_tokens": 0,
                },
            },
        }
        return f"event: message_start\ndata: {json.dumps(event)}\n\n"

    def ensure_thinking_block(self) -> list[str]:
        """Ensure a thinking block is started, return events if created."""
        if self.blocks.thinking_index >= 0:
            return []
        idx = self.blocks.allocate_index()
        self.blocks.thinking_index = idx
        event = {
            "type": "content_block_start",
            "index": idx,
            "content_block": {
                "type": "thinking",
                "thinking": "",
            },
        }
        return [f"event: content_block_start\ndata: {json.dumps(event)}\n\n"]

    def ensure_text_block(self) -> list[str]:
        """Ensure a text block is started, return events if created."""
        if self.blocks.text_index >= 0:
            return []
        idx = self.blocks.allocate_index()
        self.blocks.text_index = idx
        event = {
            "type": "content_block_start",
            "index": idx,
            "content_block": {
                "type": "text",
                "text": "",
            },
        }
        return [f"event: content_block_start\ndata: {json.dumps(event)}\n\n"]

    def emit_thinking_delta(self, text: str) -> str:
        """Build thinking content_block_delta event."""
        event = {
            "type": "content_block_delta",
            "index": self.blocks.thinking_index,
            "delta": {
                "type": "thinking_delta",
                "thinking": text,
            },
        }
        return f"event: content_block_delta\ndata: {json.dumps(event)}\n\n"

    def emit_text_delta(self, text: str) -> str:
        """Build text content_block_delta event."""
        self.output_tokens += max(1, len(text) // 4)
        event = {
            "type": "content_block_delta",
            "index": self.blocks.text_index,
            "delta": {
                "type": "text_delta",
                "text": text,
            },
        }
        return f"event: content_block_delta\ndata: {json.dumps(event)}\n\n"

    def emit_tool_delta(self, index: int, args_json: str) -> str:
        """Build tool use input_json_delta event."""
        event = {
            "type": "content_block_delta",
            "index": index,
            "delta": {
                "type": "input_json_delta",
                "partial_json": args_json,
            },
        }
        return f"event: content_block_delta\ndata: {json.dumps(event)}\n\n"

    def content_block_start(self, index: int, block_type: str, **kwargs: Any) -> str:
        """Build content_block_start event."""
        content_block: dict[str, Any] = {"type": block_type}
        content_block.update(kwargs)
        event = {
            "type": "content_block_start",
            "index": index,
            "content_block": content_block,
        }
        return f"event: content_block_start\ndata: {json.dumps(event)}\n\n"

    def content_block_delta(self, index: int, delta_type: str, content: str) -> str:
        """Build content_block_delta event."""
        event = {
            "type": "content_block_delta",
            "index": index,
            "delta": {
                "type": delta_type,
            },
        }
        if delta_type == "text_delta":
            event["delta"]["text"] = content
            self.output_tokens += max(1, len(content) // 4)
        elif delta_type == "thinking_delta":
            event["delta"]["thinking"] = content
        elif delta_type == "input_json_delta":
            event["delta"]["partial_json"] = content
        return f"event: content_block_delta\ndata: {json.dumps(event)}\n\n"

    def content_block_stop(self, index: int) -> str:
        """Build content_block_stop event."""
        event = {
            "type": "content_block_stop",
            "index": index,
        }
        return f"event: content_block_stop\ndata: {json.dumps(event)}\n\n"

    def close_content_blocks(self) -> list[str]:
        """Close any open content blocks (thinking, text)."""
        events: list[str] = []
        if self.blocks.thinking_index >= 0:
            events.append(self.content_block_stop(self.blocks.thinking_index))
            self.blocks.thinking_index = -1
        if self.blocks.text_index >= 0:
            events.append(self.content_block_stop(self.blocks.text_index))
            self.blocks.text_index = -1
        return events

    def start_tool_block(self, index: int, tool_id: str, name: str) -> str:
        """Start a tool use block."""
        self.blocks.tool_states[index] = ToolState(name=name, id=tool_id, started=True)
        return self.content_block_start(
            index,
            "tool_use",
            id=tool_id,
            name=name,
        )

    def close_all_blocks(self) -> list[str]:
        """Close all open content blocks."""
        events = self.close_content_blocks()
        # Close any tool blocks
        for idx in list(self.blocks.tool_states.keys()):
            events.append(self.content_block_stop(idx))
        return events

    def message_delta(self, stop_reason: str, output_tokens: int) -> str:
        """Build message_delta event."""
        event = {
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": None,
            },
            "usage": {
                "output_tokens": output_tokens or self.output_tokens,
            },
        }
        return f"event: message_delta\ndata: {json.dumps(event)}\n\n"

    def message_stop(self) -> str:
        """Build message_stop event."""
        event = {"type": "message_stop"}
        return f"event: message_stop\ndata: {json.dumps(event)}\n\n"

    def emit_error(self, message: str) -> list[str]:
        """Build error event."""
        event = {
            "type": "error",
            "error": {
                "type": "api_error",
                "message": message,
            },
        }
        return [f"event: error\ndata: {json.dumps(event)}\n\n"]

    def estimate_output_tokens(self) -> int:
        """Estimate output tokens from accumulated output."""
        return self.output_tokens
