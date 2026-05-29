use regex::Regex;
use std::sync::LazyLock;

static TABLE_SEP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$"#).unwrap());

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*```"#).unwrap());

fn is_gfm_table_header_line(line: &str) -> bool {
    if !line.contains('|') {
        return false;
    }
    if TABLE_SEP_RE.is_match(line) {
        return false;
    }
    let parts: Vec<&str> = line.trim().trim_start_matches('|').trim_end_matches('|')
        .split('|')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    parts.len() >= 2
}

/// Insert blank lines before detected tables outside fenced code blocks.
pub fn normalize_gfm_tables(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return text.to_string();
    }

    let mut out_lines = Vec::new();
    let mut in_fence = false;

    for (idx, line) in lines.iter().enumerate() {
        if FENCE_RE.is_match(line) {
            in_fence = !in_fence;
            out_lines.push(line.to_string());
            continue;
        }

        if !in_fence
            && idx + 1 < lines.len()
            && is_gfm_table_header_line(line)
            && TABLE_SEP_RE.is_match(lines[idx + 1])
            && out_lines.last().map(|s: &String| !s.trim().is_empty()).unwrap_or(false)
        {
            let indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            out_lines.push(indent);
        }

        out_lines.push(line.to_string());
    }

    out_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_gfm_tables() {
        let input = "Some text\n| A | B |\n|---|---|\n| 1 | 2 |";
        let result = normalize_gfm_tables(input);
        assert!(result.contains("\n\n| A"));
    }

    #[test]
    fn test_is_gfm_table_header() {
        assert!(is_gfm_table_header_line("| A | B |"));
        assert!(!is_gfm_table_header_line("|---|---|"));
        assert!(!is_gfm_table_header_line("not a table"));
    }
}
