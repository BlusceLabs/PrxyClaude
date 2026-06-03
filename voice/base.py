"""Voice transcription abstract base class."""

from abc import ABC, abstractmethod


class TranscriptionBackend(ABC):
    """Abstract base for voice transcription backends."""

    @abstractmethod
    async def transcribe(self, audio_data: bytes) -> str:
        """Transcribe audio bytes to text."""
