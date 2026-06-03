"""PrxyClaude · Settings (canonical config loader)"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings

ENV_FILE = Path(os.getenv("PRXYCLAUDE_ENV", ".env"))


class Settings(BaseSettings):
    """All settings loaded from .env file."""

    # Server
    host: str = Field(default="0.0.0.0", alias="HOST")
    port: int = Field(default=8082, alias="PORT")
    proxy_auth_token: str = Field(default="prxyclaude", alias="PROXY_AUTH_TOKEN")
    admin_token: str = Field(default="admin-change-me", alias="ADMIN_TOKEN")
    log_level: str = Field(default="info", alias="LOG_LEVEL")

    # API Keys
    nvidia_nim_api_key: str = Field(default="", alias="NVIDIA_NIM_API_KEY")
    openrouter_api_key: str = Field(default="", alias="OPENROUTER_API_KEY")
    lm_studio_base_url: str = Field(default="", alias="LM_STUDIO_BASE_URL")

    # Model mappings: provider_type/model_name
    model_opus: str = Field(default="nvidia_nim/z-ai/glm4.7", alias="MODEL_OPUS")
    model_sonnet: str = Field(default="nvidia_nim/z-ai/glm4.7", alias="MODEL_SONNET")
    model_haiku: str = Field(default="nvidia_nim/z-ai/glm4.7", alias="MODEL_HAIKU")
    model: str = Field(default="nvidia_nim/z-ai/glm4.7", alias="MODEL")

    # Provider config
    provider_rate_limit: int = Field(default=40, alias="PROVIDER_RATE_LIMIT")
    provider_rate_window: int = Field(default=60, alias="PROVIDER_RATE_WINDOW")
    provider_max_concurrency: int = Field(default=5, alias="PROVIDER_MAX_CONCURRENCY")

    # HTTP timeouts (seconds)
    http_read_timeout: float = Field(default=120, alias="HTTP_READ_TIMEOUT")
    http_write_timeout: float = Field(default=10, alias="HTTP_WRITE_TIMEOUT")
    http_connect_timeout: float = Field(default=2, alias="HTTP_CONNECT_TIMEOUT")

    # Messaging
    messaging_platform: str = Field(default="discord", alias="MESSAGING_PLATFORM")
    messaging_rate_limit: int = Field(default=1, alias="MESSAGING_RATE_LIMIT")
    messaging_rate_window: int = Field(default=1, alias="MESSAGING_RATE_WINDOW")

    # Voice
    voice_note_enabled: bool = Field(default=False, alias="VOICE_NOTE_ENABLED")
    whisper_device: str = Field(default="nvidia_nim", alias="WHISPER_DEVICE")
    whisper_model: str = Field(default="openai/whisper-large-v3", alias="WHISPER_MODEL")
    hf_token: str = Field(default="", alias="HF_TOKEN")

    # Telegram
    telegram_bot_token: str = Field(default="", alias="TELEGRAM_BOT_TOKEN")
    allowed_telegram_user_id: str = Field(default="", alias="ALLOWED_TELEGRAM_USER_ID")

    # Discord
    discord_bot_token: str = Field(default="", alias="DISCORD_BOT_TOKEN")
    allowed_discord_channels: str = Field(default="", alias="ALLOWED_DISCORD_CHANNELS")

    # Agent
    claude_workspace: str = Field(default="./agent_workspace", alias="CLAUDE_WORKSPACE")
    allowed_dir: str = Field(default="", alias="ALLOWED_DIR")

    # Optimization settings
    fast_prefix_detection: bool = Field(default=True, alias="FAST_PREFIX_DETECTION")
    enable_network_probe_mock: bool = Field(
        default=True, alias="ENABLE_NETWORK_PROBE_MOCK"
    )
    enable_title_generation_skip: bool = Field(
        default=True, alias="ENABLE_TITLE_GENERATION_SKIP"
    )
    enable_suggestion_mode_skip: bool = Field(
        default=True, alias="ENABLE_SUGGESTION_MODE_SKIP"
    )
    enable_filepath_extraction_mock: bool = Field(
        default=True, alias="ENABLE_FILEPATH_EXTRACTION_MOCK"
    )

    @property
    def provider_type(self) -> str:
        """Extract provider type from model string."""
        return self.parse_provider_type(self.model)

    @staticmethod
    def parse_provider_type(model_str: str) -> str:
        """Parse provider type from model string (e.g., 'nvidia_nim/model' -> 'nvidia_nim')."""
        if "/" in model_str:
            return model_str.split("/")[0]
        return "nvidia_nim"

    model_config = {
        "env_file": str(ENV_FILE),
        "env_file_encoding": "utf-8",
        "extra": "ignore",
    }


# ─── Provider Base URLs ──────────────────────────────────────────────────────

PROVIDER_BASE_URLS: dict[str, str] = {
    "nvidia_nim": "https://integrate.api.nvidia.com/v1",
    "open_router": "https://openrouter.ai/api/v1",
    "lmstudio": "",  # set from LM_STUDIO_BASE_URL
    "groq": "https://api.groq.com/openai/v1",
    "together": "https://api.together.xyz/v1",
    "mistral": "https://api.mistral.ai/v1",
    "ollama": "http://localhost:11434/api",
    "anthropic": "https://api.anthropic.com",
}


# ─── Parsed Model Mapping ───────────────────────────────────────────────────


@dataclass
class ModelMapping:
    """Parsed model string: provider_type/model_name"""

    provider_type: str
    model_name: str

    @classmethod
    def parse(cls, raw: str) -> ModelMapping:
        parts = raw.split("/", 1)
        if len(parts) != 2 or not parts[0] or not parts[1]:
            raise ValueError(
                f"Invalid model mapping format: {raw!r} (expected provider_type/model_name)"
            )
        return cls(provider_type=parts[0], model_name=parts[1])


def parse_model_mappings(settings: Settings) -> dict[str, ModelMapping]:
    """Parse all MODEL_* env vars into ModelMapping objects."""
    return {
        "opus": ModelMapping.parse(settings.model_opus),
        "sonnet": ModelMapping.parse(settings.model_sonnet),
        "haiku": ModelMapping.parse(settings.model_haiku),
        "default": ModelMapping.parse(settings.model),
    }


# ─── Runtime Config ─────────────────────────────────────────────────────────


@dataclass
class RuntimeConfig:
    """Resolved runtime configuration"""

    settings: Settings
    model_mappings: dict[str, ModelMapping] = field(default_factory=dict)
    api_keys: dict[str, list[str]] = field(default_factory=dict)
    providers: list[str] = field(default_factory=list)

    @classmethod
    def from_settings(cls, settings: Settings) -> RuntimeConfig:
        model_mappings = parse_model_mappings(settings)
        api_keys: dict[str, list[str]] = {}
        providers: list[str] = []

        if settings.nvidia_nim_api_key:
            api_keys["nvidia_nim"] = [settings.nvidia_nim_api_key]
            providers.append("nvidia_nim")

        if settings.openrouter_api_key:
            api_keys["open_router"] = [settings.openrouter_api_key]
            providers.append("open_router")

        if settings.lm_studio_base_url:
            providers.append("lmstudio")

        return cls(
            settings=settings,
            model_mappings=model_mappings,
            api_keys=api_keys,
            providers=providers,
        )


# ─── Singletons ─────────────────────────────────────────────────────────────

_settings: Settings | None = None
_config: RuntimeConfig | None = None


def get_settings() -> Settings:
    global _settings
    if _settings is None:
        _settings = Settings()
    return _settings


def get_config() -> RuntimeConfig:
    global _config
    if _config is None:
        _config = RuntimeConfig.from_settings(get_settings())
    return _config


def reload_config() -> RuntimeConfig:
    global _settings, _config
    _settings = None
    _config = None
    return get_config()


def reload_settings() -> Settings:
    global _settings
    _settings = None
    return get_settings()
