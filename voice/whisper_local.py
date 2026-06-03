"""Local Whisper transcription via Hugging Face transformers."""

from __future__ import annotations

from voice.base import TranscriptionBackend


class LocalWhisperTranscriber(TranscriptionBackend):
    """Transcribe audio locally using Hugging Face Whisper."""

    def __init__(self, model_name: str = "base", device: str = "cpu"):
        self._model_name = model_name
        self._device = device
        self._pipeline = None

    async def transcribe(self, audio_data: bytes) -> str:
        if self._pipeline is None:
            self._load_pipeline()

        import tempfile

        with tempfile.NamedTemporaryFile(suffix=".ogg", delete=False) as tmp:
            tmp.write(audio_data)
            tmp_path = tmp.name

        try:
            if self._pipeline is None:
                return ""
            result = self._pipeline(tmp_path)
            return result.get("text", "")
        finally:
            import os

            os.unlink(tmp_path)

    def _load_pipeline(self):
        try:
            from transformers import pipeline

            self._pipeline = pipeline(
                "automatic-speech-recognition",
                model=f"openai/whisper-{self._model_name}",
                device=self._device,
            )
        except ImportError as e:
            raise ImportError(
                "Local Whisper requires voice_local extras: "
                "`uv sync --extra voice_local`"
            ) from e
