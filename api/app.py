"""PrxyClaude · FastAPI Application"""

from __future__ import annotations

import json
import time
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from uuid import uuid4

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse
from loguru import logger

from api.dependencies import (
    get_provider_for_model,
    resolve_target_model,
    stream_to_anthropic_response,
)
from api.request_utils import get_token_count
from config.settings import get_config, get_settings
from core.cache import cache_clear, cache_get, cache_set, cache_stats
from core.circuit_breaker import get_circuit_states, reset_circuit
from core.key_manager import all_key_pool_stats
from core.metrics import get_metrics, record_cache_hit, record_request
from core.queue import get_queue, queue_stats
from core.rate_limiter import configure_rate_limiter, get_rate_limiter
from core.subagent_control import intercept_subagent_calls
from core.types import AnthropicMessage, AnthropicRequest
from providers.common.optimizer import intercept_request
from providers.transform import detect_tier


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Lifespan context manager for startup/shutdown events."""
    # Startup
    settings = get_settings()
    configure_rate_limiter(settings.provider_rate_limit, settings.provider_rate_window)
    logger.info("[app] PrxyClaude started")
    yield
    # Shutdown (if needed)
    logger.info("[app] PrxyClaude shutting down")


def create_app() -> FastAPI:
    app = FastAPI(title="PrxyClaude", docs_url=None, redoc_url=None, lifespan=lifespan)

    # ─── CORS ────────────────────────────────────────────────────────────────
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # ─── Auth Middleware ──────────────────────────────────────────────────────
    @app.middleware("http")
    async def auth_middleware(request: Request, call_next):
        settings = get_settings()
        path = request.url.path

        # Skip auth for health, admin UI, and OPTIONS
        if path == "/health" or path == "/admin" or request.method == "OPTIONS":
            return await call_next(request)

        # Admin API auth
        if path.startswith("/admin/api/"):
            admin_token = request.headers.get(
                "x-admin-token"
            ) or request.query_params.get("token")
            if admin_token != settings.admin_token:
                return JSONResponse(status_code=403, content={"error": "Forbidden"})
            return await call_next(request)

        # Proxy auth for /v1/* endpoints
        if (
            path.startswith("/v1/")
            and settings.proxy_auth_token
            and settings.proxy_auth_token != "any"
        ):
            auth_header = request.headers.get("authorization", "")
            bearer_token = (
                auth_header[7:] if auth_header.startswith("Bearer ") else None
            )
            api_key = request.headers.get("x-api-key")
            provided = bearer_token or api_key

            if not provided or provided != settings.proxy_auth_token:
                return JSONResponse(
                    status_code=401,
                    content={
                        "type": "error",
                        "error": {
                            "type": "authentication_error",
                            "message": "Invalid or missing proxy auth token",
                        },
                    },
                )

        return await call_next(request)

    # ─── Health ──────────────────────────────────────────────────────────────
    @app.get("/health")
    async def health():
        return {
            "ok": True,
            "version": "2.0.0",
            "ts": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        }

    # ─── Models ──────────────────────────────────────────────────────────────
    @app.get("/v1/models")
    async def models():
        return {
            "data": [
                {"id": "claude-opus-4-5", "object": "model", "created": 1700000000},
                {"id": "claude-sonnet-4-5", "object": "model", "created": 1700000000},
                {
                    "id": "claude-haiku-4-5-20251001",
                    "object": "model",
                    "created": 1700000000,
                },
                {"id": "claude-opus-4-6", "object": "model", "created": 1700000000},
                {"id": "claude-sonnet-4-6", "object": "model", "created": 1700000000},
            ]
        }

    # ─── Messages (main proxy endpoint) ──────────────────────────────────────
    @app.post("/v1/messages")
    async def messages(request: Request):
        body = await request.json()
        req = AnthropicRequest(**body)

        if not req.model or not req.messages:
            return JSONResponse(
                status_code=400,
                content={
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Missing required fields: model, messages",
                    },
                },
            )

        record_request()

        # Convert messages to dict format for optimization checks
        messages_dicts = [
            {
                "role": m.role,
                "content": m.content if isinstance(m.content, str) else str(m.content),
            }
            for m in req.messages
        ]

        # ── Request Optimization ──
        settings = get_settings()
        optimized = intercept_request(messages_dicts, settings)
        if optimized:
            return optimized

        # ── Subagent Control ──
        messages_dicts = intercept_subagent_calls(messages_dicts)

        tier = detect_tier(req.model).value
        is_stream = req.stream is True

        # ── Rate Limiting ──
        config = get_config()
        model_mapping = config.model_mappings.get(
            tier, config.model_mappings["default"]
        )
        provider_type = model_mapping.provider_type

        rate_limiter = get_rate_limiter()
        if not rate_limiter.can_proceed(provider_type):
            return JSONResponse(
                status_code=429,
                content={
                    "type": "error",
                    "error": {
                        "type": "rate_limit_error",
                        "message": f"Rate limit exceeded for {provider_type}",
                    },
                },
            )
        rate_limiter.record_request(provider_type)

        # ── Resolve Provider ──
        provider = get_provider_for_model(req.model)
        target_model = resolve_target_model(req.model)
        req_mapped = (
            req.model_copy(update={"model": target_model})
            if target_model != req.model
            else req
        )
        input_tokens = get_token_count(req.messages, req.system, req.tools)
        request_id = f"req_{uuid4().hex[:12]}"

        # ── Streaming ──
        if is_stream:

            async def stream_generator() -> AsyncIterator[str]:
                try:
                    queue = get_queue()

                    async def execute_stream():
                        async for chunk in provider.stream_response(
                            req_mapped, input_tokens=input_tokens, request_id=request_id
                        ):
                            yield chunk

                    async for chunk in queue.enqueue_stream(execute_stream, tier):
                        yield chunk
                except Exception as e:
                    msg = str(e)
                    logger.error(f"[stream] {msg}")
                    yield f"event: error\ndata: {json.dumps({'type': 'error', 'error': {'type': 'api_error', 'message': msg}})}\n\n"
                yield "data: [DONE]\n\n"

            return StreamingResponse(
                stream_generator(),
                media_type="text/event-stream",
                headers={
                    "Cache-Control": "no-cache",
                    "Connection": "keep-alive",
                    "X-Accel-Buffering": "no",
                },
            )

        # ── Non-streaming ──
        # Check cache first
        cached = cache_get(body)
        if cached:
            record_cache_hit()
            return cached

        try:
            queue = get_queue()
            result = await queue.enqueue(
                lambda: stream_to_anthropic_response(
                    provider, req, input_tokens, request_id, target_model=target_model
                ),
                tier,
            )
            cache_set(body, result)
            return result
        except Exception as e:
            msg = str(e)
            logger.error(f"[messages] {msg}")
            return JSONResponse(
                status_code=503,
                content={
                    "type": "error",
                    "error": {"type": "overloaded_error", "message": msg},
                },
            )

    # ─── OpenAI-compatible Chat Completions (for Codex, etc.) ─────────────
    @app.post("/v1/chat/completions")
    async def chat_completions(request: Request):
        body = await request.json()

        # Convert OpenAI format to Anthropic format
        messages = body.get("messages", [])
        model = body.get("model", "gpt-4")
        max_tokens = body.get("max_tokens", 4096)
        stream = body.get("stream", False)

        # Extract system message
        system_prompt = None
        anthropic_messages = []
        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            if role == "system":
                system_prompt = content
            elif role in ("user", "assistant"):
                anthropic_messages.append({"role": role, "content": content or ""})

        if not anthropic_messages:
            return JSONResponse(
                status_code=400,
                content={"error": {"message": "No messages provided"}},
            )

        # Create Anthropic request
        anthropic_messages_typed = [
            AnthropicMessage(role=m["role"], content=m.get("content") or "")
            for m in anthropic_messages
        ]
        req = AnthropicRequest(
            model=model,
            messages=anthropic_messages_typed,
            max_tokens=max_tokens,
            stream=stream,
            system=system_prompt if system_prompt else None,
        )

        record_request()

        # ── Request Optimization ──
        settings = get_settings()
        optimized = intercept_request(
            [
                {
                    "role": m.role,
                    "content": m.content
                    if isinstance(m.content, str)
                    else str(m.content),
                }
                for m in req.messages
            ],
            settings,
        )
        if optimized:
            # Convert Anthropic response to OpenAI format
            return JSONResponse(
                content={
                    "id": f"chatcmpl-{int(time.time() * 1000)}",
                    "object": "chat.completion",
                    "created": int(time.time()),
                    "model": model,
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": optimized.get("content", [{}])[0].get(
                                    "text", ""
                                ),
                            },
                            "finish_reason": "stop",
                        }
                    ],
                    "usage": {
                        "prompt_tokens": 0,
                        "completion_tokens": 0,
                        "total_tokens": 0,
                    },
                }
            )

        tier = detect_tier(req.model).value

        # ── Rate Limiting ──
        config = get_config()
        model_mapping = config.model_mappings.get(
            tier, config.model_mappings["default"]
        )
        provider_type = model_mapping.provider_type

        rate_limiter = get_rate_limiter()
        if not rate_limiter.can_proceed(provider_type):
            return JSONResponse(
                status_code=429,
                content={
                    "error": {"message": f"Rate limit exceeded for {provider_type}"}
                },
            )
        rate_limiter.record_request(provider_type)

        # ── Resolve Provider ──
        provider = get_provider_for_model(req.model)
        target_model = resolve_target_model(req.model)
        req_mapped = (
            req.model_copy(update={"model": target_model})
            if target_model != req.model
            else req
        )
        input_tokens = get_token_count(req.messages, req.system, req.tools)
        request_id = f"req_{uuid4().hex[:12]}"

        # ── Streaming ──
        if stream:

            async def stream_openai():
                response_id = f"chatcmpl-{int(time.time() * 1000)}"
                created = int(time.time())

                # Send initial role chunk
                yield f"data: {json.dumps({'id': response_id, 'object': 'chat.completion.chunk', 'created': created, 'model': model, 'choices': [{'index': 0, 'delta': {'role': 'assistant', 'content': ''}, 'finish_reason': None}]})}\n\n"

                try:
                    queue = get_queue()

                    async def execute_stream():
                        async for chunk in provider.stream_response(
                            req_mapped, input_tokens=input_tokens, request_id=request_id
                        ):
                            yield chunk

                    async for raw_event in queue.enqueue_stream(execute_stream, tier):
                        for event in raw_event.split("\n"):
                            event = event.strip()
                            if not event:
                                continue
                            if event.startswith("event: "):
                                continue
                            if event.startswith("data: "):
                                try:
                                    data = json.loads(event[6:])
                                    if data.get("type") == "content_block_delta":
                                        delta = data.get("delta", {})
                                        if delta.get("type") == "text_delta":
                                            text = delta.get("text", "")
                                            if text:
                                                yield f"data: {json.dumps({'id': response_id, 'object': 'chat.completion.chunk', 'created': created, 'model': model, 'choices': [{'index': 0, 'delta': {'content': text}, 'finish_reason': None}]})}\n\n"
                                    elif data.get("type") == "message_delta":
                                        stop_reason = data.get("delta", {}).get(
                                            "stop_reason", "stop"
                                        )
                                        if stop_reason == "end_turn":
                                            stop_reason = "stop"
                                        yield f"data: {json.dumps({'id': response_id, 'object': 'chat.completion.chunk', 'created': created, 'model': model, 'choices': [{'index': 0, 'delta': {}, 'finish_reason': stop_reason}]})}\n\n"
                                except json.JSONDecodeError:
                                    continue
                except Exception as e:
                    logger.error(f"[chat_completions] {e}")

                yield "data: [DONE]\n\n"

            return StreamingResponse(
                stream_openai(),
                media_type="text/event-stream",
                headers={
                    "Cache-Control": "no-cache",
                    "Connection": "keep-alive",
                    "X-Accel-Buffering": "no",
                },
            )

        # ── Non-streaming ──
        try:
            queue = get_queue()
            result = await queue.enqueue(
                lambda: stream_to_anthropic_response(
                    provider, req, input_tokens, request_id, target_model=target_model
                ),
                tier,
            )

            # Convert Anthropic response to OpenAI format
            content_parts = [
                block.get("text", "")
                for block in result.get("content", [])
                if block.get("type") == "text"
            ]

            return JSONResponse(
                content={
                    "id": f"chatcmpl-{int(time.time() * 1000)}",
                    "object": "chat.completion",
                    "created": int(time.time()),
                    "model": model,
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": "".join(content_parts),
                            },
                            "finish_reason": result.get("stop_reason", "stop"),
                        }
                    ],
                    "usage": result.get(
                        "usage",
                        {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
                    ),
                }
            )
        except Exception as e:
            msg = str(e)
            logger.error(f"[chat_completions] {msg}")
            return JSONResponse(
                status_code=503,
                content={"error": {"message": msg}},
            )

    # ─── Admin UI ────────────────────────────────────────────────────────────
    @app.get("/admin", response_class=HTMLResponse)
    async def admin_ui():
        admin_html_path = Path(__file__).parent.parent / "admin" / "ui.html"
        if admin_html_path.exists():
            return HTMLResponse(admin_html_path.read_text())
        return HTMLResponse("<h1>Admin UI not found</h1>")

    # ─── Admin API ───────────────────────────────────────────────────────────
    @app.get("/admin/api/status")
    async def admin_status():
        cfg = get_config()
        metrics = get_metrics()
        return {
            "ok": True,
            "version": "2.0.0",
            "uptime": int((time.time() * 1000 - metrics.started_at) / 1000),
            "port": cfg.settings.port,
            "providers": [
                {
                    "id": p,
                    "label": p,
                    "type": p,
                    "enabled": True,
                    "priority": i,
                    "keyCount": len(cfg.api_keys.get(p, [])),
                }
                for i, p in enumerate(cfg.providers)
            ],
            "circuits": get_circuit_states(),
            "keys": all_key_pool_stats(),
            "metrics": metrics.model_dump(),
            "cache": cache_stats(),
            "queue": queue_stats(),
            "rateLimit": get_rate_limiter().get_status(cfg.providers[0])
            if cfg.providers
            else {},
        }

    @app.get("/admin/api/config")
    async def admin_config():
        cfg = get_config()
        safe_keys = {
            k: [f"...{key[-6:]}" for key in v] for k, v in cfg.api_keys.items()
        }
        return {
            "providers": cfg.providers,
            "api_keys": safe_keys,
            "model_mappings": {
                k: {"provider_type": v.provider_type, "model_name": v.model_name}
                for k, v in cfg.model_mappings.items()
            },
        }

    @app.post("/admin/api/provider/{provider_id}/enable")
    async def enable_provider(provider_id: str):
        return {"ok": True, "message": f"Provider {provider_id} enabled"}

    @app.post("/admin/api/provider/{provider_id}/disable")
    async def disable_provider(provider_id: str):
        return {"ok": True, "message": f"Provider {provider_id} disabled"}

    @app.post("/admin/api/provider/{provider_id}/reset-circuit")
    async def reset_circuit_endpoint(provider_id: str):
        reset_circuit(provider_id)
        return {"ok": True}

    @app.post("/admin/api/cache/clear")
    async def clear_cache_endpoint():
        cache_clear()
        return {"ok": True}

    @app.get("/admin/api/metrics")
    async def get_metrics_endpoint():
        return get_metrics().model_dump()

    return app


app = create_app()
