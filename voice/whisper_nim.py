"""NVIDIA NIM Whisper transcription via gRPC."""

from __future__ import annotations

from loguru import logger

from voice.base import TranscriptionBackend


class NvidiaNimTranscriber(TranscriptionBackend):
    """Transcribe audio using NVIDIA NIM Whisper via gRPC."""

    def __init__(self, api_key: str, model_name: str = "openai/whisper-large-v3"):
        self._api_key = api_key
        self._model_name = model_name

    async def transcribe(self, audio_data: bytes) -> str:
        try:
            return await self._transcribe_riva(audio_data)
        except ImportError:
            logger.warning("riva.client not installed, falling back to HTTP API")
            return await self._transcribe_http(audio_data)

    async def _transcribe_riva(self, audio_data: bytes) -> str:
        import riva.client

        auth = riva.client.Auth(
            uri="grpc.nvidia.com:443",
            use_ssl=True,
            metadata=[
                ("authorization", f"Bearer {self._api_key}"),
            ],
        )
        service = riva.client.RecognizeService(auth)
        config = riva.client.RecognitionConfig(
            encoding=riva.client.AudioEncoding.LINEAR_PCM,
            sample_rate_hertz=16000,
            language_code="en-US",
            model=self._model_name,
        )
        try:
            response = service.riva_recognize(audio_data, config)
            return (
                response.results[0].alternatives[0].transcript
                if response.results
                else ""
            )
        except Exception:
            logger.warning("Riva gRPC failed, falling back to HTTP")
            return await self._transcribe_http(audio_data)

    async def _transcribe_http(self, audio_data: bytes) -> str:
        import base64

        import httpx

        encoded = base64.b64encode(audio_data).decode()
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                "https://integrate.api.nvidia.com/v1/audio/transcriptions",
                headers={
                    "Authorization": f"Bearer {self._api_key}",
                    "Content-Type": "application/json",
                },
                json={
                    "model": self._model_name,
                    "audio": encoded,
                },
                timeout=30,
            )
            resp.raise_for_status()
            data = resp.json()
            return data.get("text", "")
