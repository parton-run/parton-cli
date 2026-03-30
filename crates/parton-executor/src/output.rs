//! Output parsing — extract file content from LLM responses.

/// Extract file content from between `===FILE_START===` and `===FILE_END===` markers.
///
/// Returns an empty string if markers are missing (invalid output).
pub fn clean_output(content: &str) -> String {
    let trimmed = content.trim();

    if let Some(start_idx) = trimmed.find("===FILE_START===") {
        let after = &trimmed[start_idx + "===FILE_START===".len()..];
        let code_start = after.strip_prefix('\n').unwrap_or(after);

        if let Some(end_idx) = code_start.find("===FILE_END===") {
            let code = code_start[..end_idx].trim_end();
            return if code.is_empty() {
                String::new()
            } else {
                code.to_string()
            };
        }

        // Start marker but no end — take everything after start.
        let code = code_start.trim_end();
        return if code.is_empty() {
            String::new()
        } else {
            code.to_string()
        };
    }

    // No markers — invalid output.
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_between_markers() {
        let input = "===FILE_START===\nconst x = 1;\n===FILE_END===";
        assert_eq!(clean_output(input), "const x = 1;");
    }

    #[test]
    fn extract_multiline() {
        let input = "===FILE_START===\nline 1\nline 2\nline 3\n===FILE_END===";
        assert_eq!(clean_output(input), "line 1\nline 2\nline 3");
    }

    #[test]
    fn missing_markers_returns_empty() {
        assert_eq!(clean_output("just some text"), "");
    }

    #[test]
    fn empty_between_markers_returns_empty() {
        assert_eq!(clean_output("===FILE_START===\n===FILE_END==="), "");
    }

    #[test]
    fn no_end_marker_takes_rest() {
        let input = "===FILE_START===\nconst x = 1;\nconst y = 2;";
        assert_eq!(clean_output(input), "const x = 1;\nconst y = 2;");
    }

    #[test]
    fn surrounding_text_ignored() {
        let input = "Here's the file:\n===FILE_START===\ncode\n===FILE_END===\nDone!";
        assert_eq!(clean_output(input), "code");
    }

    #[test]
    fn whitespace_trimmed() {
        let input = "  \n===FILE_START===\ncode\n===FILE_END===  \n";
        assert_eq!(clean_output(input), "code");
    }
}
