//! Plan parsing — extract a [`RunPlan`] from raw LLM output.

use parton_core::RunPlan;

/// Error type for plan parsing failures.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// No JSON object found in the output.
    #[error("no JSON object found in planner output")]
    NoJson,

    /// JSON braces are unbalanced.
    #[error("unbalanced JSON braces in planner output")]
    UnbalancedBraces,

    /// JSON is malformed.
    #[error("malformed JSON: {0}")]
    Malformed(String),
}

/// Parse a [`RunPlan`] from raw LLM text output.
///
/// Handles common LLM quirks: leading/trailing text, markdown fences, etc.
pub fn parse_plan(content: &str) -> Result<RunPlan, ParseError> {
    let trimmed = content.trim();

    // Try direct parse first (ideal: clean JSON).
    if let Ok(plan) = serde_json::from_str::<RunPlan>(trimmed) {
        return Ok(plan);
    }

    // Find first `{` and extract via balanced brace matching.
    let start = trimmed.find('{').ok_or(ParseError::NoJson)?;
    let end = find_matching_brace(trimmed, start).ok_or(ParseError::UnbalancedBraces)?;

    let json_str = &trimmed[start..=end];
    serde_json::from_str::<RunPlan>(json_str).map_err(|e| ParseError::Malformed(e.to_string()))
}

/// Find the closing `}` that matches the opening `{` at `start`.
fn find_matching_brace(s: &str, start: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in s[start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let json = r#"{"summary":"test","files":[],"validation_commands":[]}"#;
        let plan = parse_plan(json).unwrap();
        assert_eq!(plan.summary, "test");
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let input =
            r#"Here's the plan: {"summary":"test","files":[],"validation_commands":[]} done!"#;
        let plan = parse_plan(input).unwrap();
        assert_eq!(plan.summary, "test");
    }

    #[test]
    fn parse_no_json_errors() {
        let result = parse_plan("no json here");
        assert!(matches!(result, Err(ParseError::NoJson)));
    }

    #[test]
    fn parse_malformed_json_errors() {
        let result = parse_plan(r#"{"summary": 42}"#);
        assert!(matches!(result, Err(ParseError::Malformed(_))));
    }

    #[test]
    fn find_matching_brace_simple() {
        assert_eq!(find_matching_brace("{}", 0), Some(1));
    }

    #[test]
    fn find_matching_brace_nested() {
        assert_eq!(find_matching_brace(r#"{"a":{"b":1}}"#, 0), Some(12));
    }

    #[test]
    fn find_matching_brace_with_strings() {
        assert_eq!(find_matching_brace(r#"{"key":"val{ue}"}"#, 0), Some(16));
    }

    #[test]
    fn find_matching_brace_unbalanced() {
        assert_eq!(find_matching_brace("{", 0), None);
    }
}
