use std::sync::Arc;

use super::discord_markdown::{
    discord_bold, discord_code_inline, escape_discord, escape_discord_code,
    format_status as discord_format_status, render_markdown_to_discord,
};
use super::telegram_markdown::{
    escape_md_v2, escape_md_v2_code, format_status as telegram_format_status, mdv2_bold,
    mdv2_code_inline, render_markdown_to_mdv2,
};

/// Rendering context with platform-specific formatting functions.
#[derive(Clone)]
pub struct RenderCtx {
    pub bold: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub code_inline: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub escape_code: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub escape_text: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub render_markdown: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub thinking_tail_max: Option<usize>,
    pub tool_input_tail_max: Option<usize>,
    pub tool_output_tail_max: Option<usize>,
    pub text_tail_max: Option<usize>,
}

impl Default for RenderCtx {
    fn default() -> Self {
        Self {
            bold: Arc::new(|s| s.to_string()),
            code_inline: Arc::new(|s| format!("`{s}`")),
            escape_code: Arc::new(|s| s.to_string()),
            escape_text: Arc::new(|s| s.to_string()),
            render_markdown: Arc::new(|s| s.to_string()),
            thinking_tail_max: Some(1000),
            tool_input_tail_max: Some(1200),
            tool_output_tail_max: Some(1600),
            text_tail_max: Some(2000),
        }
    }
}

impl RenderCtx {
    pub fn bold(&self, text: &str) -> String {
        (self.bold)(text)
    }

    pub fn code_inline(&self, text: &str) -> String {
        (self.code_inline)(text)
    }

    pub fn escape_code(&self, text: &str) -> String {
        (self.escape_code)(text)
    }

    pub fn escape_text(&self, text: &str) -> String {
        (self.escape_text)(text)
    }

    pub fn render_markdown(&self, text: &str) -> String {
        (self.render_markdown)(text)
    }
}

/// Platform rendering profile.
pub struct RenderingProfile {
    pub format_status: Arc<dyn Fn(&str, &str, Option<&str>) -> String + Send + Sync>,
    pub parse_mode: Option<String>,
    pub render_ctx: RenderCtx,
    pub limit_chars: usize,
}

/// Build a rendering profile for a messaging platform.
pub fn build_rendering_profile(platform_name: &str) -> Arc<RenderingProfile> {
    let is_discord = platform_name == "discord";

    let format_status: Arc<dyn Fn(&str, &str, Option<&str>) -> String + Send + Sync> = if is_discord
    {
        Arc::new(|emoji, label, suffix| discord_format_status(emoji, label, suffix))
    } else {
        Arc::new(|emoji, label, suffix| telegram_format_status(emoji, label, suffix))
    };

    let render_ctx = if is_discord {
        RenderCtx {
            bold: Arc::new(|s| discord_bold(s)),
            code_inline: Arc::new(|s| discord_code_inline(s)),
            escape_code: Arc::new(|s| escape_discord_code(s)),
            escape_text: Arc::new(|s| escape_discord(s)),
            render_markdown: Arc::new(|s| render_markdown_to_discord(s)),
            ..Default::default()
        }
    } else {
        RenderCtx {
            bold: Arc::new(|s| mdv2_bold(s)),
            code_inline: Arc::new(|s| mdv2_code_inline(s)),
            escape_code: Arc::new(|s| escape_md_v2_code(s)),
            escape_text: Arc::new(|s| escape_md_v2(s)),
            render_markdown: Arc::new(|s| render_markdown_to_mdv2(s)),
            ..Default::default()
        }
    };

    Arc::new(RenderingProfile {
        format_status,
        parse_mode: if is_discord {
            None
        } else {
            Some("MarkdownV2".into())
        },
        render_ctx,
        limit_chars: if is_discord { 1900 } else { 3900 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_telegram_profile() {
        let p = build_rendering_profile("telegram");
        assert_eq!(p.parse_mode.as_deref(), Some("MarkdownV2"));
        assert_eq!(p.limit_chars, 3900);
        let s = (p.format_status)("\u{23f3}", "Test", None);
        assert!(s.contains("Test"));
    }

    #[test]
    fn test_build_discord_profile() {
        let p = build_rendering_profile("discord");
        assert_eq!(p.parse_mode, None);
        assert_eq!(p.limit_chars, 1900);
        let s = (p.format_status)("\u{23f3}", "Test", None);
        assert!(s.contains("Test"));
    }
}
