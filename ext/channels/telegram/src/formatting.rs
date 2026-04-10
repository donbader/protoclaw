use regex::Regex;
use std::sync::LazyLock;

/// Escape `&`, `<`, `>` for Telegram HTML parse mode.
/// These are the only three characters that need escaping — Telegram HTML
/// is much simpler than MarkdownV2 (which requires 18+ escapes).
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

static FENCED_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```(\w*)\n?(.*?)```").expect("valid regex literal"));

static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([^`]+)`").expect("valid regex literal"));

static BLOCKQUOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(?:>\s?(.*)(?:\n|$))+").expect("valid regex literal"));

static HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+(.+)$").expect("valid regex literal"));

static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("valid regex literal"));

static BOLD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*").expect("valid regex literal"));

static STRIKE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~~(.+?)~~").expect("valid regex literal"));

static ITALIC_UNDERSCORE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b_([^_]+?)_\b").expect("valid regex literal"));

static ITALIC_STAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*([^*]+?)\*").expect("valid regex literal"));

/// Convert markdown-ish agent output to Telegram-safe HTML.
///
/// Uses a placeholder-extraction pattern to avoid double-escaping:
/// code blocks and inline code are extracted first (their content escaped),
/// then the remaining text is escaped, markdown transforms applied,
/// and placeholders restored.
pub fn format_telegram_html(text: &str) -> String {
    let mut code_blocks: Vec<String> = Vec::new();
    let mut inline_codes: Vec<String> = Vec::new();

    // Step 1: Extract fenced code blocks → placeholder
    let text = FENCED_CODE_RE.replace_all(text, |caps: &regex::Captures| {
        let lang = caps.get(1).map_or("", |m| m.as_str());
        let code = escape_html(caps.get(2).map_or("", |m| m.as_str()));
        let block = if lang.is_empty() {
            format!("<pre>{code}</pre>")
        } else {
            format!("<pre><code class=\"language-{lang}\">{code}</code></pre>")
        };
        let idx = code_blocks.len();
        code_blocks.push(block);
        format!("\x00CODEBLOCK_{idx}\x00")
    });

    // Step 2: Extract inline code → placeholder
    let text = INLINE_CODE_RE.replace_all(&text, |caps: &regex::Captures| {
        let code = escape_html(caps.get(1).map_or("", |m| m.as_str()));
        let idx = inline_codes.len();
        inline_codes.push(format!("<code>{code}</code>"));
        format!("\x00INLINE_{idx}\x00")
    });

    // Step 3: Extract blockquotes → placeholder
    let mut blockquotes: Vec<String> = Vec::new();
    let text = BLOCKQUOTE_RE.replace_all(&text, |caps: &regex::Captures| {
        let full = caps.get(0).map_or("", |m| m.as_str());
        let lines: Vec<&str> = full
            .lines()
            .map(|l| {
                l.strip_prefix("> ")
                    .or_else(|| l.strip_prefix(">"))
                    .unwrap_or(l)
            })
            .collect();
        let content = escape_html(&lines.join("\n"));
        let idx = blockquotes.len();
        blockquotes.push(format!("<blockquote>{content}</blockquote>"));
        format!("\x00BLOCKQUOTE_{idx}\x00")
    });

    // Step 4: Escape remaining text
    let mut text = escape_html(&text);

    // Step 5: Apply markdown transforms on escaped text
    text = HEADER_RE.replace_all(&text, "<b>$1</b>").into_owned();
    text = LINK_RE
        .replace_all(&text, "<a href=\"$2\">$1</a>")
        .into_owned();
    text = BOLD_RE.replace_all(&text, "<b>$1</b>").into_owned();
    text = STRIKE_RE.replace_all(&text, "<s>$1</s>").into_owned();
    text = ITALIC_UNDERSCORE_RE
        .replace_all(&text, "<i>$1</i>")
        .into_owned();
    text = ITALIC_STAR_RE.replace_all(&text, "<i>$1</i>").into_owned();

    // Step 6: Restore placeholders
    for (i, block) in blockquotes.iter().enumerate() {
        text = text.replace(&format!("\x00BLOCKQUOTE_{i}\x00"), block);
    }
    for (i, code) in inline_codes.iter().enumerate() {
        text = text.replace(&format!("\x00INLINE_{i}\x00"), code);
    }
    for (i, block) in code_blocks.iter().enumerate() {
        text = text.replace(&format!("\x00CODEBLOCK_{i}\x00"), block);
    }

    text
}

/// Close any unclosed HTML tags in a partial/streaming text fragment.
/// Needed when displaying incomplete streaming content — Telegram rejects
/// malformed HTML, so we must close any tags the agent hasn't finished yet.
pub fn close_open_tags(html: &str) -> String {
    let tags: &[&str] = &["b", "i", "s", "u", "code", "pre", "blockquote", "a"];
    let mut stack: Vec<&str> = Vec::new();

    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = html[i..].find('>').map(|p| i + p).unwrap_or(bytes.len());
            let tag_content = &html[i + 1..end];
            if let Some(closing) = tag_content.strip_prefix('/') {
                let tag_name = closing.split_whitespace().next().unwrap_or("");
                if let Some(pos) = stack.iter().rposition(|&t| t == tag_name) {
                    stack.remove(pos);
                }
            } else {
                let tag_name = tag_content.split_whitespace().next().unwrap_or("");
                if tags.contains(&tag_name) {
                    stack.push(tag_name);
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }

    let mut result = html.to_string();
    for tag in stack.iter().rev() {
        result.push_str(&format!("</{tag}>"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::ampersand("a & b", "a &amp; b")]
    #[case::angle_brackets("<script>", "&lt;script&gt;")]
    #[case::all_three("a & <b> c", "a &amp; &lt;b&gt; c")]
    #[case::no_escaping("hello world", "hello world")]
    fn when_escaping_html_then_special_chars_replaced(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(escape_html(input), expected);
    }

    #[rstest]
    #[case::bold("**bold**", "<b>bold</b>")]
    #[case::italic_underscore("_italic_", "<i>italic</i>")]
    #[case::italic_star("*italic*", "<i>italic</i>")]
    #[case::strikethrough("~~strike~~", "<s>strike</s>")]
    #[case::inline_code("`code`", "<code>code</code>")]
    #[case::link(
        "[text](http://example.com)",
        "<a href=\"http://example.com\">text</a>"
    )]
    #[case::header("# Title", "<b>Title</b>")]
    fn when_formatting_markdown_then_html_tags_produced(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(format_telegram_html(input), expected);
    }

    #[rstest]
    fn when_formatting_fenced_code_then_pre_tag_with_language() {
        let input = "```rust\nfn main() {}\n```";
        let result = format_telegram_html(input);
        assert_eq!(
            result,
            "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>"
        );
    }

    #[rstest]
    fn when_formatting_fenced_code_without_lang_then_plain_pre() {
        let input = "```\nhello\n```";
        let result = format_telegram_html(input);
        assert_eq!(result, "<pre>hello\n</pre>");
    }

    #[rstest]
    fn when_code_contains_angle_brackets_then_escaped_inside_pre() {
        let input = "```\nif a < b && c > d\n```";
        let result = format_telegram_html(input);
        assert!(result.contains("&lt;"));
        assert!(result.contains("&gt;"));
        assert!(result.contains("&amp;&amp;"));
    }

    #[rstest]
    fn when_text_has_angle_brackets_outside_code_then_escaped() {
        let input = "use Vec<String>";
        let result = format_telegram_html(input);
        assert_eq!(result, "use Vec&lt;String&gt;");
    }

    #[rstest]
    fn when_blockquote_then_blockquote_tag() {
        let input = "> quoted line";
        let result = format_telegram_html(input);
        assert!(result.contains("<blockquote>"));
        assert!(result.contains("quoted line"));
    }

    #[rstest]
    #[case::unclosed_bold("<b>open", "<b>open</b>")]
    #[case::unclosed_italic("<i>text", "<i>text</i>")]
    #[case::nested_unclosed("<b><i>text", "<b><i>text</i></b>")]
    #[case::properly_closed("<b>text</b>", "<b>text</b>")]
    #[case::empty("", "")]
    fn when_closing_open_tags_then_appended(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(close_open_tags(input), expected);
    }

    #[rstest]
    fn when_formatting_mixed_content_then_no_double_escape() {
        let input = "**bold** and `<html>` code";
        let result = format_telegram_html(input);
        assert!(result.contains("<b>bold</b>"));
        assert!(result.contains("<code>&lt;html&gt;</code>"));
        assert!(!result.contains("&amp;lt;"));
    }
}
