"""Voice transcription factory."""

from __future__ import annotations

from config.settings import Settings
from voice.base import TranscriptionBackend


def create_transcription_backend(settings: Settings) -> TranscriptionBackend | None:
    """Create the appropriate transcription backend based on settings."""
    if not settings.voice_note_enabled:
        return None

    device = settings.whisper_device

    if device == "nvidia_nim":
        from voice.whisper_nim import NvidiaNimTranscriber

        return NvidiaNimTranscriber(
            api_key=settings.nvidia_nim_api_key,
            model_name=settings.whisper_model,
        )

    from voice.whisper_local import LocalWhisperTranscriber

    return LocalWhisperTranscriber(
        model_name=settings.whisper_model,
        device=device,
    )
