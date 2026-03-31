//! Output parsing — extract file content from LLM responses.

use crate::scaffold::strip_markdown_fences_public;

/// Extract file content from between code markers.
///
/// Supports both `===CODE===`/`===END===` (new) and
/// `===FILE_START===`/`===FILE_END===` (legacy) markers.
/// Strips markdown fences that LLMs sometimes add.
/// Returns an empty string if no markers found or if the LLM
/// returned a meta-comment instead of actual code.
pub fn clean_output(content: &str) -> String {
    let trimmed = content.trim();

    // Try new markers first.
    if let Some(code) = extract_between_markers(trimmed, "===CODE===", "===END===") {
        let cleaned = strip_markdown_fences_public(&code);
        if is_meta_comment(&cleaned) {
            return String::new();
        }
        return cleaned;
    }

    // Fallback to legacy markers.
    if let Some(code) = extract_between_markers(trimmed, "===FILE_START===", "===FILE_END===") {
        let cleaned = strip_markdown_fences_public(&code);
        if is_meta_comment(&cleaned) {
            return String::new();
        }
        return cleaned;
    }

    // Try JSON fallback — some responses may still come as {"code": "..."}.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(code) = val.get("code").and_then(|c| c.as_str()) {
            if !code.is_empty() && !is_meta_comment(code) {
                return strip_markdown_fences_public(code);
            }
        }
    }

    String::new()
}

/// Detect LLM meta-comments that aren't actual code.
///
/// LLMs sometimes return explanations instead of code, e.g.
/// "(The file content is identical to the EXISTING SCAFFOLD...)".
fn is_meta_comment(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Short content starting with '(' is almost always a meta-comment.
    let looks_like_comment = trimmed.starts_with('(') && trimmed.ends_with(')');
    let has_meta_phrases = [
        "identical to",
        "no changes",
        "no modifications",
        "already implemented",
        "already fully implemented",
        "nothing to replace",
        "no stubs to replace",
        "unchanged",
        "same as",
    ]
    .iter()
    .any(|p| trimmed.to_lowercase().contains(p));

    looks_like_comment && has_meta_phrases
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
    fn meta_comment_returns_empty() {
        let input = "===CODE===\n(The file content is identical to the EXISTING SCAFFOLD provided — it is already fully implemented with no stubs to replace.)\n===END===";
        assert_eq!(clean_output(input), "");
    }

    #[test]
    fn meta_comment_no_changes_returns_empty() {
        let input = "===CODE===\n(No changes needed — the file is unchanged.)\n===END===";
        assert_eq!(clean_output(input), "");
    }

    #[test]
    fn real_code_with_parens_not_meta() {
        let input = "===CODE===\n(function() { return 1; })()\n===END===";
        assert_eq!(clean_output(input), "(function() { return 1; })()");
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
