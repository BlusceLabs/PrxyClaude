use std::collections::HashSet;

fn is_env_assignment(part: &str) -> bool {
    if let Some(eq_pos) = part.find('=') {
        if eq_pos > 0 {
            let name = &part[..eq_pos];
            return name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
        }
    }
    false
}

fn strip_env_assignments(parts: &[String]) -> &[String] {
    let mut start = 0;
    for part in parts {
        if is_env_assignment(part) {
            start += 1;
        } else {
            break;
        }
    }
    &parts[start..]
}

/// Parse a shell command safely, returning the command prefix.
pub fn extract_command_prefix(command: &str) -> String {
    if command.contains('`') || command.contains("$(") {
        return "command_injection_detected".to_string();
    }
    let parts: Vec<String> = command
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if parts.is_empty() {
        return "none".to_string();
    }
    let cmd_parts = strip_env_assignments(&parts);
    if cmd_parts.is_empty() {
        return "none".to_string();
    }
    let first_word = &cmd_parts[0];
    let two_word_commands: HashSet<&str> = [
        "git", "npm", "docker", "kubectl", "cargo", "go", "pip", "yarn",
    ]
    .into();
    if two_word_commands.contains(first_word.as_str()) && cmd_parts.len() > 1 {
        let second = &cmd_parts[1];
        if !second.starts_with('-') {
            return format!("{} {}", first_word, second);
        }
        return first_word.clone();
    }
    first_word.clone()
}

/// Extract file paths from a command locally without API call.
pub fn extract_filepaths_from_command(command: &str, _output: &str) -> String {
    let listing_commands: HashSet<&str> =
        ["ls", "dir", "find", "tree", "pwd", "cd", "mkdir", "rmdir", "rm"].into();
    let reading_commands: HashSet<&str> =
        ["cat", "head", "tail", "less", "more", "bat", "type"].into();
    let parts: Vec<String> = command
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if parts.is_empty() {
        return "<filepaths>\n</filepaths>".to_string();
    }
    let cmd_parts = strip_env_assignments(&parts);
    if cmd_parts.is_empty() {
        return "<filepaths>\n</filepaths>".to_string();
    }
    let base_cmd = cmd_parts[0]
        .split('/')
        .last()
        .unwrap_or(&cmd_parts[0])
        .to_lowercase();
    if listing_commands.contains(base_cmd.as_str()) {
        return "<filepaths>\n</filepaths>".to_string();
    }
    if reading_commands.contains(base_cmd.as_str()) {
        let filepaths: Vec<&str> = cmd_parts[1..]
            .iter()
            .filter(|p| !p.starts_with('-'))
            .map(|s| s.as_str())
            .collect();
        if filepaths.is_empty() {
            return "<filepaths>\n</filepaths>".to_string();
        }
        let paths_str = filepaths.join("\n");
        return format!("<filepaths>\n{}\n</filepaths>", paths_str);
    }
    "<filepaths>\n</filepaths>".to_string()
}
