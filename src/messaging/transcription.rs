use std::path::Path;

/// Max file size in bytes (25 MB).
pub const MAX_AUDIO_SIZE_BYTES: u64 = 25 * 1024 * 1024;

/// Short model names -> full Hugging Face model IDs.
pub fn resolve_model_id(whisper_model: &str) -> &str {
    match whisper_model {
        "tiny" => "openai/whisper-tiny",
        "base" => "openai/whisper-base",
        "small" => "openai/whisper-small",
        "medium" => "openai/whisper-medium",
        "large-v2" => "openai/whisper-large-v2",
        "large-v3" => "openai/whisper-large-v3",
        "large-v3-turbo" => "openai/whisper-large-v3-turbo",
        _ => whisper_model,
    }
}

/// Transcribe audio file to text.
pub fn transcribe_audio(
    file_path: &Path,
    _mime_type: &str,
    whisper_model: &str,
    _whisper_device: &str,
    _hf_token: &str,
    _nvidia_nim_api_key: &str,
) -> Result<String, String> {
    if !file_path.exists() {
        return Err(format!("Audio file not found: {}", file_path.display()));
    }

    let size = std::fs::metadata(file_path)
        .map(|m| m.len())
        .map_err(|e| format!("Failed to read file metadata: {e}"))?;

    if size > MAX_AUDIO_SIZE_BYTES {
        return Err(format!(
            "Audio file too large ({size} bytes). Max {MAX_AUDIO_SIZE_BYTES} bytes."
        ));
    }

    let _model_id = resolve_model_id(whisper_model);
    // Placeholder - in production, integrate with whisper or NVIDIA NIM
    Err("Local transcription not yet implemented in Rust".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_id() {
        assert_eq!(resolve_model_id("tiny"), "openai/whisper-tiny");
        assert_eq!(resolve_model_id("base"), "openai/whisper-base");
        assert_eq!(resolve_model_id("custom"), "custom");
    }

    #[test]
    fn test_transcribe_nonexistent() {
        let result = transcribe_audio(
            Path::new("/nonexistent.wav"),
            "audio/wav",
            "base",
            "cpu",
            "",
            "",
        );
        assert!(result.is_err());
    }
}
