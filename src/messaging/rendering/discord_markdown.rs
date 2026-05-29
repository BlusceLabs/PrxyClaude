use std::collections::HashSet;
use std::sync::LazyLock;

use super::markdown_tables::normalize_gfm_tables;

static DISCORD_SPECIAL: LazyLock<HashSet<char>> = LazyLock::new(|| {
    r"\*_`~|>".chars().collect()
});

/// Escape text for Discord markdown.
pub fn escape_discord(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if DISCORD_SPECIAL.contains(&ch) {
                format!("\\{ch}")
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Escape text for Discord code spans/blocks.
pub fn escape_discord_code(text: &str) -> String {
    text.replace('\\', "\\\\").replace('`', "\\`")
}

/// Format text as bold in Discord (uses **).
pub fn discord_bold(text: &str) -> String {
    format!("**{}**", escape_discord(text))
}

/// Format text as inline code in Discord.
pub fn discord_code_inline(text: &str) -> String {
    format!("`{}`", escape_discord_code(text))
}

/// Format a status message for Discord (label in bold, optional suffix).
pub fn format_status_discord(label: &str, suffix: Option<&str>) -> String {
    let base = discord_bold(label);
    if let Some(s) = suffix {
        format!("{base} {}", escape_discord(s))
    } else {
        base
    }
}

/// Format a status message with emoji for Discord (matches Telegram API).
pub fn format_status(emoji: &str, label: &str, suffix: Option<&str>) -> String {
    let base = format!("{emoji} {}", discord_bold(label));
    if let Some(s) = suffix {
        format!("{base} {}", escape_discord(s))
    } else {
        base
    }
}

/// Render common Markdown into Discord-compatible format.
/// Simplified renderer for common cases.
pub fn render_markdown_to_discord(text: &str) -> String {
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
            output.push_str(line);
            output.push('\n');
            i += 1;
            continue;
        }

        if line.starts_with("# ") {
            let content = &line[2..];
            output.push_str(&format!("**{}**\n\n", escape_discord(content)));
            i += 1;
            continue;
        }
        if line.starts_with("## ") {
            let content = &line[3..];
            output.push_str(&format!("**{}**\n\n", escape_discord(content)));
            i += 1;
            continue;
        }

        if line.starts_with("- ") || line.starts_with("* ") {
            let content = &line[2..];
            output.push_str(&format!("- {}\n", escape_discord(content)));
            i += 1;
            continue;
        }

        if line.starts_with("> ") {
            let content = &line[2..];
            output.push_str(&format!("> {}\n", escape_discord(content)));
            i += 1;
            continue;
        }

        output.push_str(&escape_discord(line));
        output.push('\n');
        i += 1;
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_discord() {
        assert_eq!(escape_discord("hello"), "hello");
        assert!(escape_discord("a*b").contains("\\*"));
    }

    #[test]
    fn test_discord_bold() {
        assert_eq!(discord_bold("test"), "**test**");
    }

    #[test]
    fn test_discord_code_inline() {
        assert_eq!(discord_code_inline("code"), "`code`");
    }

    #[test]
    fn test_format_status_discord() {
        let s = format_status_discord("Processing", None);
        assert_eq!(s, "**Processing**");
    }

    #[test]
    fn test_format_status_with_emoji() {
        let s = format_status("\u{23f3}", "Queued", Some("(pos 1)"));
        assert!(s.contains("Queued"));
        assert!(s.contains("pos"));
    }
}
