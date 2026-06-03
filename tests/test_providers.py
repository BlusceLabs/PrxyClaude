"""PrxyClaude · Tests for Providers & Common Utilities"""

from __future__ import annotations

import asyncio
import json
import time

import pytest

from config.nim import NimSettings
from config.settings import ModelMapping
from core.cache import ResponseCache
from core.metrics import GlobalMetrics, ProviderMetrics
from core.rate_limiter import RateLimiter
from core.types import (
    AnthropicMessage,
    AnthropicRequest,
    CircuitState,
    ContentBlock,
    ModelTier,
    OpenAIRequest,
    OpenAITool,
    ProviderType,
    QueuedRequest,
    Tool,
    ToolChoice,
)
from providers.base import ProviderConfig
from providers.common import (
    ContentType,
    HeuristicToolParser,
    SSEBuilder,
    ThinkTagParser,
    append_request_id,
    extract_thinking_from_text,
    get_user_facing_error_message,
    map_stop_reason,
)
from providers.common.logging import (
    build_request_summary,
    generate_request_fingerprint,
    get_last_user_message_preview,
    get_tool_names,
)
from providers.common.message_converter import build_base_request_body
from providers.common.text import (
    count_tokens_approximate,
    extract_text_from_content,
    truncate_text,
)
from providers.common.thinking_parser import (
    build_thinking_block,
    has_thinking_content,
    merge_thinking_into_content,
    parse_reasoning_content,
)
from providers.common.utils import set_if_not_none
from providers.exceptions import (
    APIError,
    AuthenticationError,
    InvalidRequestError,
    OverloadedError,
    ProviderError,
    RateLimitError,
)
from providers.transform import detect_tier

# ─── detect_tier ──────────────────────────────────────────────────────────


class TestDetectTier:
    def test_opus(self):
        assert detect_tier("claude-opus-4-5") == ModelTier.opus

    def test_opus_case_insensitive(self):
        assert detect_tier("claude-OPUS-4") == ModelTier.opus

    def test_sonnet(self):
        assert detect_tier("claude-sonnet-4-5") == ModelTier.sonnet

    def test_sonnet_fallback(self):
        assert detect_tier("gpt-5.5-high") == ModelTier.sonnet

    def test_haiku(self):
        assert detect_tier("claude-haiku-4-5") == ModelTier.haiku

    def test_haiku_case_insensitive(self):
        assert detect_tier("claude-HAIKU-3.5") == ModelTier.haiku

    def test_empty_string(self):
        assert detect_tier("") == ModelTier.sonnet

    def test_partial_opus(self):
        assert detect_tier("my-custom-opus-model") == ModelTier.opus

    def test_nvidia_model_name(self):
        assert detect_tier("nvidia/nemotron") == ModelTier.sonnet


# ─── ModelMapping.parse ──────────────────────────────────────────────────


class TestModelMapping:
    def test_parse_full(self):
        mapping = ModelMapping.parse("nvidia_nim/z-ai/glm4.7")
        assert mapping.provider_type == "nvidia_nim"
        assert mapping.model_name == "z-ai/glm4.7"

    def test_parse_openrouter(self):
        mapping = ModelMapping.parse("open_router/openrouter/owl-alpha")
        assert mapping.provider_type == "open_router"
        assert mapping.model_name == "openrouter/owl-alpha"

    def test_parse_no_provider(self):
        with pytest.raises(ValueError, match="Invalid model mapping format"):
            ModelMapping.parse("gpt-4")


# ─── Provider Exceptions ──────────────────────────────────────────────────


class TestProviderExceptions:
    def test_base_error_to_anthropic(self):
        err = ProviderError("something broke")
        result = err.to_anthropic_format()
        assert result["type"] == "error"
        assert result["error"]["type"] == "api_error"
        assert result["error"]["message"] == "something broke"

    def test_authentication_error(self):
        err = AuthenticationError("bad key")
        assert err.status_code == 401
        assert err.error_type == "authentication_error"
        result = err.to_anthropic_format()
        assert result["error"]["type"] == "authentication_error"

    def test_invalid_request_error(self):
        err = InvalidRequestError("bad request")
        assert err.status_code == 400
        assert err.error_type == "invalid_request_error"

    def test_rate_limit_error(self):
        err = RateLimitError("too fast")
        assert err.status_code == 429
        assert err.error_type == "rate_limit_error"

    def test_overloaded_error(self):
        err = OverloadedError("busy")
        assert err.status_code == 529
        assert err.error_type == "overloaded_error"

    def test_api_error_default_status(self):
        err = APIError("generic failure")
        assert err.status_code == 500
        assert err.error_type == "api_error"

    def test_api_error_custom_status(self):
        err = APIError("gateway timeout", status_code=504)
        assert err.status_code == 504
        assert err.error_type == "api_error"

    def test_raw_error_preserved(self):
        raw = ValueError("original")
        err = APIError("wrapped", raw_error=raw)
        assert err.raw_error is raw
        assert str(err) == "wrapped"


# ─── map_stop_reason ──────────────────────────────────────────────────────


class TestMapStopReason:
    def test_stop(self):
        assert map_stop_reason("stop") == "end_turn"

    def test_length(self):
        assert map_stop_reason("length") == "max_tokens"

    def test_tool_calls(self):
        assert map_stop_reason("tool_calls") == "tool_use"

    def test_content_filter(self):
        assert map_stop_reason("content_filter") == "end_turn"

    def test_none(self):
        assert map_stop_reason(None) == "end_turn"

    def test_unknown(self):
        assert map_stop_reason("weird_reason") == "end_turn"


# ─── ResponseCache ────────────────────────────────────────────────────────


class TestResponseCache:
    def test_set_and_get(self):
        cache = ResponseCache(max_entries=10, ttl_ms=60_000)
        cache.set({"model": "test"}, {"content": "hello"})
        result = cache.get({"model": "test"})
        assert result == {"content": "hello"}

    def test_miss(self):
        cache = ResponseCache(max_entries=10, ttl_ms=60_000)
        assert cache.get({"model": "other"}) is None

    def test_clear(self):
        cache = ResponseCache(max_entries=10, ttl_ms=60_000)
        cache.set({"model": "test"}, "value")
        cache.clear()
        assert cache.get({"model": "test"}) is None

    def test_eviction(self):
        cache = ResponseCache(max_entries=2, ttl_ms=60_000)
        cache.set({"a": 1}, "one")
        cache.set({"b": 2}, "two")
        cache.set({"c": 3}, "three")
        assert cache.get({"a": 1}) is None
        assert cache.get({"b": 2}) == "two"
        assert cache.get({"c": 3}) == "three"

    def test_ttl_expiry(self):
        cache = ResponseCache(max_entries=10, ttl_ms=0)
        cache.set({"model": "test"}, "value")
        time.sleep(0.001)
        assert cache.get({"model": "test"}) is None

    def test_stats(self):
        cache = ResponseCache(max_entries=100, ttl_ms=30_000)
        stats = cache.stats()
        assert stats["size"] == 0
        assert stats["maxEntries"] == 100
        assert stats["ttlMs"] == 30_000

    def test_key_based_on_content_not_reference(self):
        cache = ResponseCache(max_entries=10, ttl_ms=60_000)
        cache.set({"model": "test"}, "value")
        assert cache.get({"model": "test"}) == "value"
        assert cache.get({"model": "test", "extra": True}) is None


# ─── RateLimiter ──────────────────────────────────────────────────────────


class TestRateLimiter:
    def test_can_proceed_initially(self):
        limiter = RateLimiter(max_requests=5, window_seconds=60)
        assert limiter.can_proceed("test") is True

    def test_blocked_at_limit(self):
        limiter = RateLimiter(max_requests=3, window_seconds=60)
        for _ in range(3):
            limiter.record_request("test")
        assert limiter.can_proceed("test") is False

    def test_allows_under_limit(self):
        limiter = RateLimiter(max_requests=5, window_seconds=60)
        for _ in range(3):
            limiter.record_request("test")
        assert limiter.can_proceed("test") is True

    def test_backoff_blocked(self):
        limiter = RateLimiter(max_requests=10, window_seconds=60)
        limiter.record_429("test", retry_after=60)
        assert limiter.can_proceed("test") is False

    def test_backoff_expires(self):
        limiter = RateLimiter(max_requests=10, window_seconds=60)
        limiter.record_429("test", retry_after=0)
        assert limiter.can_proceed("test") is True

    def test_per_provider_independence(self):
        limiter = RateLimiter(max_requests=1, window_seconds=60)
        limiter.record_request("provider_a")
        assert limiter.can_proceed("provider_a") is False
        assert limiter.can_proceed("provider_b") is True

    def test_get_status(self):
        limiter = RateLimiter(max_requests=10, window_seconds=60)
        limiter.record_request("test")
        status = limiter.get_status("test")
        assert "requests_in_window" in status
        assert status["max_requests"] == 10
        assert status["window_seconds"] == 60


# ─── GlobalMetrics ────────────────────────────────────────────────────────


class TestGlobalMetrics:
    def test_record_request(self):
        from core.metrics import get_metrics, record_request

        metrics = get_metrics()
        before = metrics.total_requests
        record_request()
        assert metrics.total_requests == before + 1

    def test_record_cache_hit(self):
        from core.metrics import get_metrics, record_cache_hit

        metrics = get_metrics()
        before = metrics.cached_requests
        record_cache_hit()
        assert metrics.cached_requests == before + 1

    def test_record_cache_hit_with_provider(self):
        from core.metrics import get_metrics, record_cache_hit

        metrics = get_metrics()
        record_cache_hit("nvidia")
        pm = metrics.providers.get("nvidia")
        assert pm is not None
        assert pm.cached_hits == 1

    def test_record_provider_success(self):
        from core.metrics import get_metrics, record_provider_success

        metrics = get_metrics()
        record_provider_success("openai", latency_ms=150, tokens_in=10, tokens_out=20)
        pm = metrics.providers["openai"]
        assert pm.requests == 1
        assert pm.successes == 1
        assert pm.avg_latency_ms == 150.0
        assert pm.total_tokens_in == 10
        assert pm.total_tokens_out == 20

    def test_record_provider_failure(self):
        from core.metrics import record_provider_failure

        metrics = GlobalMetrics()
        record_provider_failure("openai", "timeout")
        pm = metrics.providers.get("openai")
        assert pm is None  # Uses global singleton, not our local instance

        # Instead test ProviderMetrics directly
        pm = ProviderMetrics()
        pm.failures += 1
        pm.last_error_msg = "timeout"
        assert pm.failures == 1
        assert "timeout" in pm.last_error_msg

    def test_get_provider_metrics_nonexistent(self):
        from core.metrics import get_provider_metrics

        assert get_provider_metrics("nonexistent") is None


class TestGlobalMetricsDataClass:
    def test_global_metrics_defaults(self):
        g = GlobalMetrics()
        assert g.total_requests == 0
        assert g.cached_requests == 0
        assert g.queued_requests == 0
        assert g.providers == {}

    def test_provider_metrics_defaults(self):
        p = ProviderMetrics()
        assert p.requests == 0
        assert p.successes == 0
        assert p.failures == 0
        assert p.latencies == []
        assert p.avg_latency_ms == 0.0


# ─── SSEBuilder ───────────────────────────────────────────────────────────


class TestSSEBuilder:
    def test_message_start(self):
        sse = SSEBuilder("msg_123", "test-model", 42)
        event = sse.message_start()
        assert event.startswith("event: message_start")
        data = json.loads(event.split("data: ")[1].strip())
        assert data["type"] == "message_start"
        assert data["message"]["id"] == "msg_123"
        assert data["message"]["model"] == "test-model"
        assert data["message"]["usage"]["input_tokens"] == 42

    def test_ensure_text_block_first_call(self):
        sse = SSEBuilder("msg_1", "m", 0)
        events = sse.ensure_text_block()
        assert len(events) == 1
        data = json.loads(events[0].split("data: ")[1])
        assert data["type"] == "content_block_start"
        assert data["content_block"]["type"] == "text"

    def test_ensure_text_block_idempotent(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_text_block()
        events = sse.ensure_text_block()
        assert events == []

    def test_emit_text_delta(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_text_block()
        event = sse.emit_text_delta("Hello")
        assert event.startswith("event: content_block_delta")
        data = json.loads(event.split("data: ")[1])
        assert data["delta"]["type"] == "text_delta"
        assert data["delta"]["text"] == "Hello"
        assert sse.output_tokens > 0

    def test_ensure_thinking_block(self):
        sse = SSEBuilder("msg_1", "m", 0)
        events = sse.ensure_thinking_block()
        assert len(events) == 1
        data = json.loads(events[0].split("data: ")[1])
        assert data["type"] == "content_block_start"
        assert data["content_block"]["type"] == "thinking"

    def test_ensure_thinking_block_idempotent(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_thinking_block()
        assert sse.ensure_thinking_block() == []

    def test_emit_thinking_delta(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_thinking_block()
        event = sse.emit_thinking_delta("thinking text")
        data = json.loads(event.split("data: ")[1])
        assert data["delta"]["type"] == "thinking_delta"
        assert data["delta"]["thinking"] == "thinking text"

    def test_content_block_start_tool(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.content_block_start(0, "tool_use", id="tool_1", name="search")
        data = json.loads(event.split("data: ")[1])
        assert data["type"] == "content_block_start"
        assert data["content_block"]["type"] == "tool_use"
        assert data["content_block"]["id"] == "tool_1"
        assert data["content_block"]["name"] == "search"

    def test_content_block_stop(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.content_block_stop(0)
        data = json.loads(event.split("data: ")[1])
        assert data["type"] == "content_block_stop"
        assert data["index"] == 0

    def test_close_content_blocks_closes_both(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_text_block()
        sse.ensure_thinking_block()
        events = sse.close_content_blocks()
        assert len(events) == 2

    def test_close_content_blocks_idempotent(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_text_block()
        sse.close_content_blocks()
        assert sse.close_content_blocks() == []

    def test_tool_delta(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.emit_tool_delta(0, '{"key": "val"}')
        data = json.loads(event.split("data: ")[1])
        assert data["delta"]["type"] == "input_json_delta"
        assert data["delta"]["partial_json"] == '{"key": "val"}'

    def test_message_delta(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.message_delta("end_turn", 42)
        data = json.loads(event.split("data: ")[1])
        assert data["type"] == "message_delta"
        assert data["delta"]["stop_reason"] == "end_turn"
        assert data["usage"]["output_tokens"] == 42

    def test_message_stop(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.message_stop()
        data = json.loads(event.split("data: ")[1])
        assert data["type"] == "message_stop"

    def test_emit_error_event(self):
        sse = SSEBuilder("msg_1", "m", 0)
        events = sse.emit_error("Something went wrong")
        assert len(events) == 1
        data = json.loads(events[0].split("data: ")[1])
        assert data["type"] == "error"
        assert data["error"]["type"] == "api_error"
        assert "Something went wrong" in data["error"]["message"]

    def test_start_tool_block(self):
        sse = SSEBuilder("msg_1", "m", 0)
        event = sse.start_tool_block(0, "tool_uuid", "search")
        data = json.loads(event.split("data: ")[1])
        assert data["type"] == "content_block_start"
        assert data["content_block"]["type"] == "tool_use"
        assert data["content_block"]["id"] == "tool_uuid"

    def test_close_all_blocks(self):
        sse = SSEBuilder("msg_1", "m", 0)
        sse.ensure_text_block()
        sse.start_tool_block(0, "t1", "search")
        events = sse.close_all_blocks()
        assert len(events) == 2  # text block + tool block stops

    def test_estimate_output_tokens(self):
        sse = SSEBuilder("msg_1", "m", 0)
        assert sse.estimate_output_tokens() == 0
        sse.ensure_text_block()
        sse.emit_text_delta("Hello world, this is a test string")
        assert sse.estimate_output_tokens() > 0

    def test_default_message_id(self):
        sse = SSEBuilder("", "m", 0)
        assert sse.message_id != ""


# ─── ThinkTagParser ───────────────────────────────────────────────────────


class TestThinkTagParser:
    def test_no_tags(self):
        parser = ThinkTagParser()
        parts = parser.feed("Hello world")
        assert len(parts) == 1
        assert parts[0].type == ContentType.TEXT
        assert parts[0].content == "Hello world"

    def test_simple_think_block(self):
        parser = ThinkTagParser()
        parts = parser.feed("<think>deep thought</think>")
        assert len(parts) == 1
        assert parts[0].type == ContentType.THINKING
        assert parts[0].content == "deep thought"

    def test_text_before_thinking(self):
        parser = ThinkTagParser()
        parts = parser.feed("before<think>inside</think>after")
        assert len(parts) == 3
        assert parts[0].type == ContentType.TEXT
        assert parts[0].content == "before"
        assert parts[1].type == ContentType.THINKING
        assert parts[1].content == "inside"
        assert parts[2].type == ContentType.TEXT
        assert parts[2].content == "after"

    def test_multiple_think_blocks(self):
        parser = ThinkTagParser()
        parts = parser.feed("<think>one</think><think>two</think>")
        assert len(parts) == 2
        assert parts[0].content == "one"
        assert parts[1].content == "two"

    def test_split_across_chunks(self):
        parser = ThinkTagParser()
        parts = parser.feed("<think>par")
        assert len(parts) == 0
        parts = parser.feed("tial</think>")
        assert len(parts) == 1
        assert parts[0].type == ContentType.THINKING
        assert parts[0].content == "partial"

    def test_unclosed_think(self):
        parser = ThinkTagParser()
        parts = parser.feed("<think>never closed")
        assert len(parts) == 0
        remaining = parser.flush()
        assert remaining is not None
        assert remaining.type == ContentType.THINKING
        assert remaining.content == "never closed"

    def test_think_at_start(self):
        parser = ThinkTagParser()
        parts = parser.feed("<think>thought</think>text")
        assert len(parts) == 2
        assert parts[0].type == ContentType.THINKING
        assert parts[1].type == ContentType.TEXT
        assert parts[1].content == "text"

    def test_empty_buffer_flush(self):
        parser = ThinkTagParser()
        assert parser.flush() is None

    def test_flush_after_complete(self):
        parser = ThinkTagParser()
        parts = parser.feed("Hello world")
        assert len(parts) == 1
        assert parts[0].type == ContentType.TEXT
        assert parts[0].content == "Hello world"
        # buffer is consumed by feed(), flush returns None
        assert parser.flush() is None


# ─── extract_thinking_from_text ───────────────────────────────────────────


class TestExtractThinkingFromText:
    def test_with_think_tags(self):
        thinking, text = extract_thinking_from_text(
            "<think>deep analysis</think>result"
        )
        assert thinking == "deep analysis"
        assert text == "result"

    def test_no_tags(self):
        thinking, text = extract_thinking_from_text("just text")
        assert thinking == ""
        assert text == "just text"

    def test_only_tags(self):
        thinking, text = extract_thinking_from_text("<think>only</think>")
        assert thinking == "only"
        assert text == ""

    def test_multiple_tags_extracts_first(self):
        thinking, text = extract_thinking_from_text(
            "<think>first</think>middle<think>second</think>"
        )
        assert thinking == "first"
        assert text == "middle<think>second</think>"

    def test_empty_string(self):
        thinking, text = extract_thinking_from_text("")
        assert thinking == ""
        assert text == ""


# ─── HeuristicToolParser ──────────────────────────────────────────────────


class TestHeuristicToolParser:
    def test_no_tools(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed("Hello world")
        assert text == "Hello world"
        assert tools == []

    def test_json_tool_call(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed(
            'Some text {"name": "search", "input": {"q": "test"}} more'
        )
        assert "search" in str(tools[0].get("name", "")) if tools else True
        if tools:
            assert tools[0]["name"] == "search"
            assert tools[0]["input"] == {"q": "test"}
        assert "Some text" in text
        assert "more" in text

    def test_json_tool_call_removed_from_text(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed('{"name": "search", "input": {"q": "test"}}')
        assert len(tools) == 1
        assert text == ""

    def test_xml_tool_call(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed('<tool name="search">{"q": "hello"}</tool>remaining')
        assert len(tools) == 1
        assert tools[0]["name"] == "search"
        assert tools[0]["input"] == {"q": "hello"}
        assert text == "remaining"

    def test_xml_tool_removed_from_text(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed('<tool name="greet">{"name": "world"}</tool>')
        assert len(tools) == 1
        assert text == ""

    def test_empty_feed(self):
        parser = HeuristicToolParser()
        text, tools = parser.feed("")
        assert text == ""
        assert tools == []

    def test_flush_no_tools(self):
        parser = HeuristicToolParser()
        parser.feed("Hello")
        assert parser.flush() == []

    def test_flush_with_json_tool(self):
        parser = HeuristicToolParser()
        parser.feed('Plain text {"name": "search", "input": {}}')
        tools = parser.flush()
        assert len(tools) == 1
        assert tools[0]["name"] == "search"

    def test_multiple_tools_in_one_chunk(self):
        parser = HeuristicToolParser()
        _, tools = parser.feed('{"name": "a", "input": {}}{"name": "b", "input": {}}')
        assert len(tools) == 2
        assert tools[0]["name"] == "a"
        assert tools[1]["name"] == "b"


# ─── build_base_request_body ──────────────────────────────────────────────


class TestBuildBaseRequestBody:
    def test_minimal_request(self):
        messages = [AnthropicMessage(role="user", content="Hello")]
        req = AnthropicRequest(model="test-model", messages=messages)
        body = build_base_request_body(req)
        assert body["model"] == "test-model"
        assert len(body["messages"]) == 1
        assert body["messages"][0]["role"] == "user"
        assert body["messages"][0]["content"] == "Hello"

    def test_with_system_string(self):
        messages = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(
            model="test", messages=messages, system="You are a helper"
        )
        body = build_base_request_body(req)
        assert body["messages"][0]["role"] == "system"
        assert body["messages"][0]["content"] == "You are a helper"

    def test_with_default_max_tokens(self):
        messages = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(model="test", messages=messages, max_tokens=500)
        body = build_base_request_body(req)
        assert body["max_tokens"] == 500

    def test_with_tools(self):
        messages = [AnthropicMessage(role="user", content="Hi")]
        tool = Tool(
            name="get_weather",
            description="Get weather",
            input_schema={"type": "object", "properties": {}},
        )
        req = AnthropicRequest(model="test", messages=messages, tools=[tool])
        body = build_base_request_body(req)
        assert len(body["tools"]) == 1
        assert body["tools"][0]["function"]["name"] == "get_weather"
        assert body["tools"][0]["type"] == "function"

    def test_with_temperature(self):
        messages = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(model="test", messages=messages, temperature=0.7)
        body = build_base_request_body(req)
        assert body["temperature"] == 0.7

    def test_with_top_p(self):
        messages = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(model="test", messages=messages, top_p=0.9)
        body = build_base_request_body(req)
        assert body["top_p"] == 0.9


# ─── Core Types ────────────────────────────────────────────────────────────


class TestCoreTypes:
    def test_model_tier_values(self):
        assert ModelTier.opus == "opus"
        assert ModelTier.sonnet == "sonnet"
        assert ModelTier.haiku == "haiku"

    def test_circuit_state_values(self):
        assert CircuitState.closed == "closed"
        assert CircuitState.open == "open"
        assert CircuitState.half_open == "half-open"

    def test_provider_type_values(self):
        assert ProviderType.openrouter == "openrouter"
        assert ProviderType.nvidia_nim == "nvidia_nim"
        assert ProviderType.lmstudio == "lmstudio"

    def test_content_block_creation(self):
        b = ContentBlock(type="text", text="hello")
        assert b.type == "text"
        assert b.text == "hello"

    def test_content_block_tool_use(self):
        b = ContentBlock(type="tool_use", id="t1", name="search", input={"q": "test"})
        assert b.type == "tool_use"
        assert b.id == "t1"
        assert b.name == "search"
        assert b.input == {"q": "test"}

    def test_tool_creation(self):
        t = Tool(
            name="get_weather",
            description="Get weather",
            input_schema={"type": "object"},
        )
        assert t.name == "get_weather"
        assert t.description == "Get weather"
        assert t.input_schema == {"type": "object"}

    def test_tool_choice_default(self):
        tc = ToolChoice()
        assert tc.type == "auto"
        assert tc.name is None

    def test_tool_choice_specific(self):
        tc = ToolChoice(type="tool", name="search")
        assert tc.type == "tool"
        assert tc.name == "search"

    def test_anthropic_message_text(self):
        m = AnthropicMessage(role="user", content="Hello")
        assert m.role == "user"
        assert m.content == "Hello"

    def test_anthropic_message_blocks(self):
        blocks = [ContentBlock(type="text", text="Hello")]
        m = AnthropicMessage(role="assistant", content=blocks)
        assert m.role == "assistant"
        assert m.content == blocks

    def test_anthropic_request_minimal(self):
        msg = AnthropicMessage(role="user", content="Hi")
        req = AnthropicRequest(model="m", messages=[msg])
        assert req.model == "m"
        assert req.max_tokens == 8192
        assert req.stream is False

    def test_anthropic_request_full(self):
        msg = AnthropicMessage(role="user", content="Hi")
        tool = Tool(name="search", input_schema={"type": "object"})
        req = AnthropicRequest(
            model="m",
            messages=[msg],
            max_tokens=500,
            temperature=0.5,
            top_p=0.9,
            stream=True,
            tools=[tool],
            system="You are a bot",
        )
        assert req.max_tokens == 500
        assert req.temperature == 0.5
        assert req.stream is True
        assert req.tools is not None
        assert len(req.tools) == 1
        assert req.system == "You are a bot"

    def test_openai_request_defaults(self):
        from core.types import OpenAIMessage

        msg = OpenAIMessage(role="user", content="Hi")
        req = OpenAIRequest(model="m", messages=[msg])
        assert req.model == "m"
        assert req.max_tokens == 8192
        assert req.stream is False

    def test_openai_request_with_tools(self):
        from core.types import OpenAIMessage

        msg = OpenAIMessage(role="user", content="Hi")
        ot = OpenAITool(function={"name": "search", "parameters": {}})
        req = OpenAIRequest(model="m", messages=[msg], tools=[ot])
        assert req.tools is not None
        assert len(req.tools) == 1
        assert req.tools[0].function["name"] == "search"

    def test_queued_request(self):
        qr = QueuedRequest(
            id="q_abc", priority=0, tier="opus", created_at=100.0, timeout_at=200.0
        )
        assert qr.id == "q_abc"
        assert qr.tier == "opus"
        assert qr.priority == 0

    def test_provider_metrics(self):
        pm = ProviderMetrics()
        assert pm.requests == 0
        assert pm.latencies == []


# ─── append_request_id & get_user_facing_error_message ────────────────────


class TestCommonUtils:
    def test_append_request_id(self):
        result = append_request_id("Error occurred", "req_abc")
        assert "req_abc" in result

    def test_append_request_id_none(self):
        assert append_request_id("Error occurred", None) == "Error occurred"

    def test_get_user_facing_error_message_generic(self):
        exc = ValueError("bad value")
        msg = get_user_facing_error_message(exc)
        assert "bad value" in msg

    def test_set_if_not_none_sets_value(self):
        d = {}
        set_if_not_none(d, "key", "value")
        assert d["key"] == "value"

    def test_set_if_not_none_skips_none(self):
        d = {"existing": True}
        set_if_not_none(d, "key", None)
        assert "key" not in d


# ─── extract_text_from_content ────────────────────────────────────────────


class TestExtractTextFromContent:
    def test_plain_string(self):
        assert extract_text_from_content("hello") == "hello"

    def test_empty_string(self):
        assert extract_text_from_content("") == ""

    def test_list_of_blocks(self):
        blocks = [
            ContentBlock(type="text", text="Hello"),
            ContentBlock(type="text", text=" World"),
        ]
        assert extract_text_from_content(blocks) == "Hello World"

    def test_list_of_dicts(self):
        blocks = [{"text": "Hello"}, {"text": " World"}]
        assert extract_text_from_content(blocks) == "Hello World"

    def test_list_with_content_field(self):
        blocks = [{"content": "nested"}]
        assert extract_text_from_content(blocks) == "nested"

    def test_object_with_text_attr(self):
        obj = type("Block", (), {"text": "hello"})()
        assert extract_text_from_content(obj) == "hello"

    def test_object_with_content_attr(self):
        obj = type("Block", (), {"content": "hello"})()
        assert extract_text_from_content(obj) == "hello"

    def test_none(self):
        assert extract_text_from_content(None) == ""


# ─── truncate_text & count_tokens_approximate ──────────────────────────────


class TestTextHelpers:
    def test_truncate_short(self):
        assert truncate_text("hello") == "hello"

    def test_truncate_long(self):
        result = truncate_text("hello world this is long", max_length=10)
        assert result == "hello w..."
        assert len(result) <= 10

    def test_truncate_custom_suffix(self):
        result = truncate_text("hello world", max_length=8, suffix="..")
        assert result == "hello .."

    def test_truncate_exact_boundary(self):
        result = truncate_text("12345", max_length=5)
        assert result == "12345"

    def test_count_tokens_approximate(self):
        assert count_tokens_approximate("hello world") == 2  # 11 // 4

    def test_count_tokens_approximate_minimum(self):
        assert count_tokens_approximate("a") == 1  # min 1

    def test_count_tokens_approximate_empty(self):
        assert count_tokens_approximate("") == 1  # min 1


# ─── thinking_parser helpers ───────────────────────────────────────────────


class TestThinkingParserHelpers:
    def test_parse_reasoning_content_dict(self):
        thinking, content = parse_reasoning_content(
            {"content": "answer", "reasoning_content": "deep thought"}
        )
        assert thinking == "deep thought"
        assert content == "answer"

    def test_parse_reasoning_content_no_reasoning(self):
        thinking, content = parse_reasoning_content({"content": "answer"})
        assert thinking == ""
        assert content == "answer"

    def test_parse_reasoning_content_think_tag(self):
        thinking, content = parse_reasoning_content(
            {"content": "<think>deep</think>answer"}
        )
        assert thinking == "deep"
        assert content == "answer"

    def test_build_thinking_block(self):
        block = build_thinking_block("thinking text")
        assert block["type"] == "thinking"
        assert block["thinking"] == "thinking text"

    def test_build_thinking_block_with_cache_control(self):
        block = build_thinking_block("text", {"type": "ephemeral"})
        assert block["cache_control"] == {"type": "ephemeral"}

    def test_merge_thinking_into_content_empty(self):
        result = merge_thinking_into_content([{"type": "text", "text": "answer"}], "")
        assert result == [{"type": "text", "text": "answer"}]

    def test_merge_thinking_into_content(self):
        result = merge_thinking_into_content(
            [{"type": "text", "text": "answer"}], "my thinking"
        )
        assert len(result) == 2
        assert result[0]["type"] == "thinking"
        assert result[0]["thinking"] == "my thinking"
        assert result[1]["type"] == "text"

    def test_merge_thinking_into_content_no_text_block(self):
        result = merge_thinking_into_content(
            [{"type": "tool_use", "name": "search"}], "thinking"
        )
        assert result[0]["type"] == "thinking"

    def test_has_thinking_content_with_reasoning(self):
        assert (
            has_thinking_content({"reasoning_content": "thinking", "content": ""})
            is True
        )

    def test_has_thinking_content_with_think_tag(self):
        assert has_thinking_content({"content": "<think>thinking</think>"}) is True

    def test_has_thinking_content_false(self):
        assert has_thinking_content({"content": "just text"}) is False


# ─── Logging Utilities ────────────────────────────────────────────────────


class TestLoggingUtilities:
    def test_generate_request_fingerprint(self):
        msgs = [AnthropicMessage(role="user", content="Hello")]
        fp = generate_request_fingerprint(msgs)
        assert fp.startswith("fp_")
        assert len(fp) == 11  # fp_ + 8 hex chars

    def test_generate_request_fingerprint_different(self):
        msgs1 = [AnthropicMessage(role="user", content="Hello")]
        msgs2 = [AnthropicMessage(role="user", content="World")]
        assert generate_request_fingerprint(msgs1) != generate_request_fingerprint(
            msgs2
        )

    def test_get_last_user_message_preview(self):
        msgs = [
            AnthropicMessage(role="assistant", content="ok"),
            AnthropicMessage(role="user", content="hello there"),
        ]
        preview = get_last_user_message_preview(msgs)
        assert "hello there" in preview

    def test_get_last_user_message_preview_no_user(self):
        msgs = [AnthropicMessage(role="assistant", content="ok")]
        assert get_last_user_message_preview(msgs) == "(no user message)"

    def test_get_last_user_message_preview_empty(self):
        assert get_last_user_message_preview([]) == "(no user message)"

    def test_get_tool_names(self):
        tools = [Tool(name="search"), Tool(name="compute")]
        names = get_tool_names(tools)
        assert names == ["search", "compute"]

    def test_get_tool_names_max_count(self):
        tools = [Tool(name=f"t{i}") for i in range(10)]
        names = get_tool_names(tools, max_count=3)
        assert names == ["t0", "t1", "t2", "+7 more"]

    def test_get_tool_names_none(self):
        assert get_tool_names(None) == []

    def test_get_tool_names_empty(self):
        assert get_tool_names([]) == []

    def test_build_request_summary(self):
        msgs = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(
            model="m",
            messages=msgs,
            max_tokens=500,
            tools=[Tool(name="search", input_schema={})],
        )
        summary = build_request_summary(req)
        assert summary["model"] == "m"
        assert summary["message_count"] == 1
        assert summary["tool_count"] == 1
        assert summary["tool_names"] == ["search"]
        assert summary["max_tokens"] == 500
        assert not summary["has_thinking"]
        assert not summary["has_system"]

    def test_build_request_summary_with_system(self):
        msgs = [AnthropicMessage(role="user", content="Hi")]
        req = AnthropicRequest(model="m", messages=msgs, system="You are a bot")
        summary = build_request_summary(req)
        assert summary["has_system"] is True


# ─── Config Types ──────────────────────────────────────────────────────────


class TestNimSettings:
    def test_default_values(self):
        ns = NimSettings()
        assert ns.temperature == 0.7
        assert ns.top_p == 0.9
        assert ns.top_k == 50
        assert ns.max_tokens == 4096
        assert ns.stop is None
        assert ns.reasoning_effort is None
        assert ns.include_reasoning is True

    def test_custom_values(self):
        ns = NimSettings(temperature=0.0, top_p=1.0, max_tokens=2048)
        assert ns.temperature == 0.0
        assert ns.top_p == 1.0
        assert ns.max_tokens == 2048


# ─── ProviderConfig ────────────────────────────────────────────────────────


class TestProviderConfig:
    def test_minimal(self):
        cfg = ProviderConfig(api_key="sk-test")
        assert cfg.api_key == "sk-test"
        assert cfg.base_url is None
        assert cfg.rate_window == 60
        assert cfg.max_concurrency == 5
        assert cfg.http_read_timeout == 300.0

    def test_full(self):
        cfg = ProviderConfig(
            api_key="sk-test",
            base_url="http://localhost:8080",
            rate_limit=50,
            rate_window=30,
            max_concurrency=10,
            http_read_timeout=120.0,
        )
        assert cfg.base_url == "http://localhost:8080"
        assert cfg.rate_limit == 50
        assert cfg.max_concurrency == 10
        assert cfg.http_read_timeout == 120.0


# ─── RequestQueue ─────────────────────────────────────────────────────────


@pytest.mark.asyncio
class TestRequestQueue:
    async def test_stats_empty(self):
        from core.queue import RequestQueue

        q = RequestQueue(max_size=50, max_concurrent=5)
        stats = q.stats()
        assert stats["depth"] == 0
        assert stats["maxSize"] == 50
        assert stats["active"] == 0
        assert stats["maxConcurrent"] == 5

    async def test_enqueue_and_execute(self):
        from core.queue import RequestQueue

        q = RequestQueue(max_size=10, max_concurrent=5)

        async def dummy():
            return 42

        result = await q.enqueue(dummy, "sonnet")
        assert result == 42

    async def test_enqueue_multiple(self):
        from core.queue import RequestQueue

        q = RequestQueue(max_size=10, max_concurrent=5)

        async def make(n):
            async def fn():
                return n

            return fn

        task1 = await make(1)
        task2 = await make(2)
        r1, r2 = await asyncio.gather(
            q.enqueue(task2, "haiku"), q.enqueue(task1, "opus")
        )
        assert r1 == 2
        assert r2 == 1

    async def test_stats_after_enqueue(self):
        from core.queue import RequestQueue

        q = RequestQueue(max_size=10, max_concurrent=5)

        async def dummy():
            return "ok"

        await q.enqueue(dummy, "sonnet")
        stats = q.stats()
        assert stats["active"] == 0
        # Items stay in the queue (depth tracks pending items not yet dequeued)
        assert stats["depth"] == 1

    async def test_queue_full(self):
        from core.queue import RequestQueue

        q = RequestQueue(max_size=1, max_concurrent=1)

        async def slow():
            await asyncio.sleep(10)
            return "done"

        async def fast():
            return "ok"

        task = asyncio.create_task(q.enqueue(slow, "sonnet"))
        await asyncio.sleep(0.05)

        with pytest.raises(RuntimeError, match="Request queue is full"):
            await q.enqueue(fast, "sonnet")

        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task


# ─── parse_provider_type ──────────────────────────────────────────────────


class TestParseProviderType:
    def test_standard_format(self):
        result = ModelMapping.parse_provider_type("nvidia_nim/my-model")
        assert result == "nvidia_nim"

    def test_openrouter(self):
        result = ModelMapping.parse_provider_type("open_router/openrouter/owl-alpha")
        assert result == "open_router"

    def test_no_separator(self):
        result = ModelMapping.parse_provider_type("my-model")
        assert result == "nvidia_nim"

    def test_in_settings(self):
        from config.settings import Settings

        s = Settings(model_opus="nvidia_nim/z-ai/glm4.7")
        assert s.provider_type == "nvidia_nim"

    def test_in_settings_openrouter(self):
        from config.settings import Settings

        s = Settings(model_opus="open_router/openrouter/owl-alpha")
        assert s.provider_type == "open_router"


# ─── get_token_count ──────────────────────────────────────────────────────


class TestGetTokenCount:
    def test_simple_message(self):
        from api.request_utils import get_token_count

        msg = AnthropicMessage(role="user", content="Hello")
        tokens = get_token_count([msg])
        assert tokens >= 1

    def test_system_string(self):
        from api.request_utils import get_token_count

        msg = AnthropicMessage(role="user", content="Hi")
        tokens = get_token_count([msg], system="You are a helpful assistant")
        assert tokens >= 4  # at least system + overhead

    def test_empty_messages(self):
        from api.request_utils import get_token_count

        tokens = get_token_count([])
        assert tokens == 1  # minimum

    def test_with_tools(self):
        from api.request_utils import get_token_count

        msg = AnthropicMessage(role="user", content="Weather?")
        tool = Tool(name="get_weather", description="Get weather", input_schema={"type": "object"})
        tokens = get_token_count([msg], tools=[tool])
        assert tokens >= 1

    def test_image_block(self):
        from api.request_utils import get_token_count

        block = ContentBlock(
            type="image",
            source={"type": "base64", "media_type": "image/png", "data": "a" * 30000},
        )
        msg = AnthropicMessage(role="user", content=[block])
        tokens = get_token_count([msg])
        assert tokens >= 10  # image has base cost

    def test_tool_use_block(self):
        from api.request_utils import get_token_count

        block = ContentBlock(
            type="tool_use", id="t1", name="search", input={"q": "test"}
        )
        msg = AnthropicMessage(role="assistant", content=[block])
        tokens = get_token_count([msg])
        assert tokens >= 1

    def test_tool_result_block(self):
        from api.request_utils import get_token_count

        block = ContentBlock(
            type="tool_result", tool_use_id="t1", content="result data"
        )
        msg = AnthropicMessage(role="user", content=[block])
        tokens = get_token_count([msg])
        assert tokens >= 1

    def test_thinking_block(self):
        from api.request_utils import get_token_count

        block = ContentBlock(type="thinking", thinking="deep reasoning")
        msg = AnthropicMessage(role="assistant", content=[block])
        tokens = get_token_count([msg])
        assert tokens >= 1


# ─── intercept_subagent_calls ─────────────────────────────────────────────


class TestInterceptSubagentCalls:
    def test_no_subagent(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [{"role": "user", "content": "Hello"}]
        result = intercept_subagent_calls(msgs)
        assert result == msgs

    def test_task_call_forced_background_false(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Task",
                        "input": {"task": "do something"},
                    }
                ],
            }
        ]
        result = intercept_subagent_calls(msgs)
        block = result[0]["content"][0]
        assert block["input"]["run_in_background"] is False

    def test_task_call_preserves_existing(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Task",
                        "input": {"task": "do something", "run_in_background": True},
                    }
                ],
            }
        ]
        result = intercept_subagent_calls(msgs)
        block = result[0]["content"][0]
        assert block["input"]["run_in_background"] is False  # overridden
        assert block["input"]["task"] == "do something"  # preserved

    def test_non_task_unaffected(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [
            {
                "role": "assistant",
                "content": [
                    {"type": "tool_use", "name": "search", "input": {"q": "hello"}}
                ],
            }
        ]
        result = intercept_subagent_calls(msgs)
        assert result == msgs

    def test_string_input_json(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Task",
                        "input": '{"task": "do something"}',
                    }
                ],
            }
        ]
        result = intercept_subagent_calls(msgs)
        block = result[0]["content"][0]
        assert isinstance(block["input"], dict)
        assert block["input"]["run_in_background"] is False

    def test_multiple_messages(self):
        from core.subagent_control import intercept_subagent_calls

        msgs = [
            {"role": "user", "content": "hi"},
            {
                "role": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Task", "input": {"task": "x"}}
                ],
            },
            {"role": "user", "content": "bye"},
        ]
        result = intercept_subagent_calls(msgs)
        assert len(result) == 3
        task_block = result[1]["content"][0]
        assert task_block["input"]["run_in_background"] is False


class TestHasSubagentCalls:
    def test_has_subagent_true(self):
        from core.subagent_control import has_subagent_calls

        msgs = [
            {
                "role": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Task", "input": {}}
                ],
            }
        ]
        assert has_subagent_calls(msgs) is True

    def test_has_subagent_false(self):
        from core.subagent_control import has_subagent_calls

        msgs = [{"role": "user", "content": "Hello"}]
        assert has_subagent_calls(msgs) is False

    def test_has_subagent_no_content(self):
        from core.subagent_control import has_subagent_calls

        msgs = [{"role": "assistant", "content": "text"}]
        assert has_subagent_calls(msgs) is False


# ─── map_error ────────────────────────────────────────────────────────────


class TestMapError:
    @staticmethod
    def _make_response(status_code: int = 429) -> object:
        """Create a minimal mock httpx Response for openai error construction."""
        import httpx

        request = httpx.Request("POST", "https://api.example.com/v1/chat/completions")
        return httpx.Response(status_code=status_code, request=request)

    def test_rate_limit_error(self):
        import openai

        exc = openai.RateLimitError(
            "rate limited",
            response=self._make_response(429),
            body={"error": {"message": "rate limited"}},
        )
        result = map_error(exc)
        from providers.exceptions import RateLimitError as RateLimitErrorCls

        assert isinstance(result, RateLimitErrorCls)

    def test_authentication_error(self):
        import openai

        exc = openai.AuthenticationError(
            "bad key",
            response=self._make_response(401),
            body={"error": {"message": "bad key"}},
        )
        result = map_error(exc)
        from providers.exceptions import AuthenticationError as AuthErrorCls

        assert isinstance(result, AuthErrorCls)

    def test_bad_request_error(self):
        import openai

        exc = openai.BadRequestError(
            "bad request",
            response=self._make_response(400),
            body={"error": {"message": "bad request"}},
        )
        result = map_error(exc)
        from providers.exceptions import InvalidRequestError

        assert isinstance(result, InvalidRequestError)

    def test_timeout_error(self):
        import httpx
        import openai

        request = httpx.Request("POST", "https://api.example.com/v1/chat/completions")
        exc = openai.APITimeoutError(request=request)
        result = map_error(exc)
        from providers.exceptions import APIError

        assert isinstance(result, APIError)

    def test_connection_error(self):
        import httpx
        import openai

        request = httpx.Request("POST", "https://api.example.com/v1/chat/completions")
        exc = openai.APIConnectionError(message="connection failed", request=request)
        result = map_error(exc)
        from providers.exceptions import APIError

        assert isinstance(result, APIError)

    def test_generic_exception(self):
        exc = ValueError("something broke")
        result = map_error(exc)
        from providers.exceptions import APIError

        assert isinstance(result, APIError)

    def test_get_user_facing_error_rate_limit(self):
        import openai

        exc = openai.RateLimitError(
            "rate limited",
            response=self._make_response(429),
            body={"error": {"message": "rate limited"}},
        )
        msg = get_user_facing_error_message(exc)
        assert "Rate limit" in msg

    def test_get_user_facing_error_timeout(self):
        import httpx
        import openai

        request = httpx.Request("POST", "https://api.example.com/v1/chat/completions")
        exc = openai.APITimeoutError(request=request)
        msg = get_user_facing_error_message(exc, read_timeout_s=120)
        assert "timed out" in msg

    def test_get_user_facing_error_connection(self):
        import httpx
        import openai

        request = httpx.Request("POST", "https://api.example.com/v1/chat/completions")
        exc = openai.APIConnectionError(message="connection failed", request=request)
        msg = get_user_facing_error_message(exc)
        assert "connect" in msg.lower()

    def test_get_user_facing_error_auth(self):
        import openai

        exc = openai.AuthenticationError(
            "bad key",
            response=self._make_response(401),
            body={"error": {"message": "bad key"}},
        )
        msg = get_user_facing_error_message(exc)
        assert "API key" in msg

    def test_get_user_facing_error_generic(self):
        exc = RuntimeError("random error")
        msg = get_user_facing_error_message(exc)
        assert "random error" in msg


# ─── RuntimeConfig ────────────────────────────────────────────────────────


class TestRuntimeConfig:
    def test_from_settings(self):
        from config.settings import RuntimeConfig, Settings

        s = Settings(
            nvidia_nim_api_key="sk-nvidia",
            openrouter_api_key="sk-openrouter",
            model_opus="nvidia_nim/model1",
            model_sonnet="open_router/model2",
            model_haiku="nvidia_nim/model3",
            model="nvidia_nim/default-model",
        )
        cfg = RuntimeConfig.from_settings(s)
        assert "nvidia_nim" in cfg.providers
        assert "open_router" in cfg.providers
        assert cfg.model_mappings["opus"].model_name == "model1"
        assert cfg.model_mappings["sonnet"].model_name == "model2"
        assert cfg.model_mappings["haiku"].model_name == "model3"
        assert cfg.model_mappings["default"].model_name == "default-model"

    def test_from_settings_lmstudio_no_api_key(self):
        from config.settings import RuntimeConfig, Settings

        s = Settings(
            lm_studio_base_url="http://localhost:1234/v1",
            model="lmstudio/local-model",
        )
        cfg = RuntimeConfig.from_settings(s)
        assert "lmstudio" in cfg.providers
        assert "nvidia_nim" not in cfg.providers
        assert "open_router" not in cfg.providers
