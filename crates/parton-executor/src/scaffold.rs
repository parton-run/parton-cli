//! Scaffold prompts and combined scaffold+enrich execution.

/// System prompt for combined scaffold+enrich — returns goal AND code in one call.
pub const SCAFFOLD_PROMPT: &str = "\
You are a code scaffolder and specification writer. You receive a file skeleton (path, exports, imports) and produce TWO things:

1. An ENRICHED GOAL — a detailed specification expanding the skeleton goal with exact signatures, behavior, edge cases
2. A SCAFFOLD FILE — minimal compilable code that satisfies all exports/imports

YOUR OUTPUT FORMAT:
===GOAL_START===
(detailed goal text — plain English, no code blocks)
===GOAL_END===
===FILE_START===
(complete minimal compilable source code)
===FILE_END===

GOAL RULES:
- PRESERVE all function names, prop names, type names from the skeleton goal
- ADD: exact parameter types, return types, behavior details, edge cases, state management
- For test files: describe what to test, expected inputs/outputs, scenarios

FILE RULES:
- Output the MINIMUM code needed to compile and satisfy all exports
- Functions: correct signature, return placeholder value
- Types/interfaces: complete definition with all fields
- Components: render minimal div, include all props in signature
- Hooks: return correct shape with placeholder values and stub functions
- Config files: complete and correct — this IS the final version
- Test files: single placeholder test that passes
- CSS/HTML files: complete — this IS the final version
- Export ALL symbols in MANDATORY Exports with EXACT names
- Import ALL symbols from Import Interfaces with EXACT names
- NEVER use markdown fences inside the markers. Just raw code.";

/// System prompt for final execution — full implementation preserving structure.
pub const FINAL_PROMPT: &str = "\
You are a file implementer. You receive a WORKING scaffold file and a detailed goal.
Your job is to replace stub implementations with real, complete code.

YOUR OUTPUT FORMAT:
Line 1: ===FILE_START===
Lines 2..N: the COMPLETE implemented source code
Last line: ===FILE_END===

CRITICAL RULES:
1. PRESERVE all import statements EXACTLY as they are in the scaffold. Do NOT rename, reorder, or remove any imports.
2. PRESERVE all export names EXACTLY. The same symbols must be exported with the same names.
3. REPLACE stub/placeholder implementations with real, working code.
4. The file must remain compilable after your changes.
5. If the scaffold file is already complete (config files, CSS, HTML), output it UNCHANGED.
6. For test files: replace placeholder tests with real tests covering the described functionality.
7. NEVER change function signatures — same parameters, same return types.
8. NEVER use markdown fences. Just raw code between markers.
9. Output the COMPLETE file, every line from first to last.";

/// Parse combined scaffold+enrich output into (goal, code).
pub fn parse_scaffold_output(content: &str) -> (String, String) {
    let trimmed = content.trim();

    let goal = extract_between(trimmed, "===GOAL_START===", "===GOAL_END===").unwrap_or_default();

    let code = extract_between(trimmed, "===FILE_START===", "===FILE_END===").unwrap_or_default();

    (goal, code)
}

fn extract_between(content: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = content.find(start_marker)?;
    let after = &content[start + start_marker.len()..];
    let code_start = after.strip_prefix('\n').unwrap_or(after);

    if let Some(end) = code_start.find(end_marker) {
        let text = code_start[..end].trim_end();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    } else {
        let text = code_start.trim_end();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_output() {
        let content = "===GOAL_START===\nDetailed goal here\n===GOAL_END===\n===FILE_START===\nconst x = 1;\n===FILE_END===";
        let (goal, code) = parse_scaffold_output(content);
        assert_eq!(goal, "Detailed goal here");
        assert_eq!(code, "const x = 1;");
    }

    #[test]
    fn parse_missing_goal() {
        let content = "===FILE_START===\ncode only\n===FILE_END===";
        let (goal, code) = parse_scaffold_output(content);
        assert!(goal.is_empty());
        assert_eq!(code, "code only");
    }

    #[test]
    fn parse_empty() {
        let (goal, code) = parse_scaffold_output("nothing");
        assert!(goal.is_empty());
        assert!(code.is_empty());
    }

    #[test]
    fn parse_multiline_goal() {
        let content = "===GOAL_START===\nLine 1\nLine 2\nLine 3\n===GOAL_END===\n===FILE_START===\nfn main(){}\n===FILE_END===";
        let (goal, code) = parse_scaffold_output(content);
        assert!(goal.contains("Line 1"));
        assert!(goal.contains("Line 3"));
        assert_eq!(code, "fn main(){}");
    }
}
