//! Output parsing — extract file content from LLM responses.

use crate::scaffold::strip_markdown_fences_public;

/// Extract file content from between code markers.
///
/// Supports both `===CODE===`/`===END===` (new) and
/// `===FILE_START===`/`===FILE_END===` (legacy) markers.
/// Strips markdown fences that LLMs sometimes add.
/// Returns an empty string if no markers found.
pub fn clean_output(content: &str) -> String {
    let trimmed = content.trim();

    // Try new markers first.
    if let Some(code) = extract_between_markers(trimmed, "===CODE===", "===END===") {
        return strip_markdown_fences_public(&code);
    }

    // Fallback to legacy markers.
    if let Some(code) = extract_between_markers(trimmed, "===FILE_START===", "===FILE_END===") {
        return strip_markdown_fences_public(&code);
    }

    // Try JSON fallback — some responses may still come as {"code": "..."}.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(code) = val.get("code").and_then(|c| c.as_str()) {
            if !code.is_empty() {
                return strip_markdown_fences_public(code);
            }
        }
    }

    String::new()
}

/// Extract text between start and end markers.
fn extract_between_markers(content: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = content.find(start)?;
    let after = &content[start_idx + start.len()..];
    let code_start = after.strip_prefix('\n').unwrap_or(after);

    let code = if let Some(end_idx) = code_start.find(end) {
        code_start[..end_idx].trim_end()
    } else {
        code_start.trim_end()
    };

    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_new_markers() {
        let input = "===CODE===\nconst x = 1;\n===END===";
        assert_eq!(clean_output(input), "const x = 1;");
    }

    #[test]
    fn extract_legacy_markers() {
        let input = "===FILE_START===\nconst x = 1;\n===FILE_END===";
        assert_eq!(clean_output(input), "const x = 1;");
    }

    #[test]
    fn extract_json_fallback() {
        let input = r#"{"code": "const x = 1;"}"#;
        assert_eq!(clean_output(input), "const x = 1;");
    }

    #[test]
    fn missing_markers_returns_empty() {
        assert_eq!(clean_output("just some text"), "");
    }

    #[test]
    fn strips_markdown_fences() {
        let input = "===CODE===\n```typescript\nconst x = 1;\n```\n===END===";
        assert_eq!(clean_output(input), "const x = 1;");
    }

    #[test]
    fn multiline_code() {
        let input = "===CODE===\nline 1\nline 2\nline 3\n===END===";
        assert_eq!(clean_output(input), "line 1\nline 2\nline 3");
    }

    #[test]
    fn surrounding_text_ignored() {
        let input = "Here's the file:\n===CODE===\ncode\n===END===\nDone!";
        assert_eq!(clean_output(input), "code");
    }

    #[test]
    fn no_end_marker_takes_rest() {
        let input = "===CODE===\nconst x = 1;\nconst y = 2;";
        assert_eq!(clean_output(input), "const x = 1;\nconst y = 2;");
    }
}
