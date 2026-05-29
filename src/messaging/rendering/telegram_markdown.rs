use std::collections::HashSet;
use std::sync::LazyLock;

use super::markdown_tables::normalize_gfm_tables;

static MDV2_SPECIAL_CHARS: LazyLock<HashSet<char>> = LazyLock::new(|| {
    r"\_*[]()~`>#+-=|{}.!".chars().collect()
});

static MDV2_LINK_ESCAPE: LazyLock<HashSet<char>> = LazyLock::new(|| {
    r"\)".chars().collect()
});

/// Escape text for Telegram MarkdownV2.
pub fn escape_md_v2(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if MDV2_SPECIAL_CHARS.contains(&ch) {
                format!("\\{ch}")
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Escape text for Telegram MarkdownV2 code spans/blocks.
pub fn escape_md_v2_code(text: &str) -> String {
    text.replace('\\', "\\\\").replace('`', "\\`")
}

/// Escape URL for Telegram MarkdownV2 link destination.
pub fn escape_md_v2_link_url(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if MDV2_LINK_ESCAPE.contains(&ch) {
                format!("\\{ch}")
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Format text as bold in MarkdownV2.
pub fn mdv2_bold(text: &str) -> String {
    format!("*{}*", escape_md_v2(text))
}

/// Format text as inline code in MarkdownV2.
pub fn mdv2_code_inline(text: &str) -> String {
    format!("`{}`", escape_md_v2_code(text))
}

/// Format a status message with emoji and optional suffix.
pub fn format_status(emoji: &str, label: &str, suffix: Option<&str>) -> String {
    let base = format!("{emoji} {}", mdv2_bold(label));
    if let Some(s) = suffix {
        format!("{base} {}", escape_md_v2(s))
    } else {
        base
    }
}

/// Render common Markdown into Telegram MarkdownV2.
/// This is a simplified renderer that handles the most common cases.
pub fn render_markdown_to_mdv2(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let text = normalize_gfm_tables(text);
    let mut output = String::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut in_code_block = false;

    while i < lines.len() {
        let line = lines[i];

        // Code blocks
        if line.trim_start().starts_with("```") {
            if in_code_block {
                output.push_str("```\n");
                in_code_block = false;
            } else {
                output.push_str("```\n");
                in_code_block = true;
            }
            i += 1;
            continue;
        }

        if in_code_block {
            output.push_str(&escape_md_v2_code(line));
            output.push('\n');
            i += 1;
            continue;
        }

        // Headers
        if line.starts_with("# ") {
            let content = &line[2..];
            output.push_str(&format!("*{}*\n\n", escape_md_v2(content)));
            i += 1;
            continue;
        }
        if line.starts_with("## ") {
            let content = &line[3..];
            output.push_str(&format!("*{}*\n\n", escape_md_v2(content)));
            i += 1;
            continue;
        }

        // Lists
        if line.starts_with("- ") || line.starts_with("* ") {
            let content = &line[2..];
            output.push_str(&format!("\\- {}\n", escape_md_v2(content)));
            i += 1;
            continue;
        }

        // Blockquotes
        if line.starts_with("> ") {
            let content = &line[2..];
            output.push_str(&format!("> {}\n", escape_md_v2(content)));
            i += 1;
            continue;
        }

        // Regular text
        output.push_str(&escape_md_v2(line));
        output.push('\n');
        i += 1;
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_md_v2() {
        assert_eq!(escape_md_v2("hello"), "hello");
        // Space is not a special char in MDV2
        assert_eq!(escape_md_v2("hello world"), "hello world");
        // But special chars are escaped
        assert!(escape_md_v2("hello*world").contains("\\*"));
    }

    #[test]
    fn test_mdv2_bold() {
        assert_eq!(mdv2_bold("test"), "*test*");
    }

    #[test]
    fn test_mdv2_code_inline() {
        assert_eq!(mdv2_code_inline("code"), "`code`");
        assert_eq!(mdv2_code_inline("a`b"), "`a\\`b`");
    }

    #[test]
    fn test_format_status() {
        let s = format_status("\u{23f3}", "Processing", None);
        assert!(s.contains("*Processing*"));
        let s = format_status("\u{23f3}", "Queued", Some("(position 2)"));
        assert!(s.contains("position"));
    }

    #[test]
    fn test_render_empty() {
        assert_eq!(render_markdown_to_mdv2(""), "");
    }
}
