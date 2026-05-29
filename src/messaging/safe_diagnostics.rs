/// Format exception for logging. If `log_full_message` is true, includes the message.
pub fn format_exception_for_log(exc: &dyn std::fmt::Display, log_full_message: bool) -> String {
    if log_full_message {
        format!("{}: {}", std::any::type_name_of_val(exc), exc)
    } else {
        std::any::type_name_of_val(exc).to_string()
    }
}

/// Length of text for metadata-only logging (0 when missing).
pub fn text_len_hint(text: Option<&str>) -> usize {
    text.map(|t| t.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_exception() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let short = format_exception_for_log(&err, false);
        // Type name contains "io::Error" or similar
        assert!(!short.is_empty());
        let full = format_exception_for_log(&err, true);
        assert!(full.contains("file missing"));
    }

    #[test]
    fn test_text_len_hint() {
        assert_eq!(text_len_hint(None), 0);
        assert_eq!(text_len_hint(Some("hello")), 5);
    }
}
