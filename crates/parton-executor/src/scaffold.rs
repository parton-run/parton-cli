//! Scaffold prompts and combined scaffold+enrich execution.

/// System prompt for combined scaffold+enrich — returns goal as JSON + code with markers.
pub const SCAFFOLD_PROMPT: &str = "\
You are a code scaffolder and specification writer. You receive a file skeleton (path, exports, imports) and produce TWO things:

1. An ENRICHED GOAL — a detailed specification expanding the skeleton goal
2. A SCAFFOLD FILE — minimal compilable code that satisfies all exports/imports

YOUR OUTPUT FORMAT (follow EXACTLY):
{\"goal\": \"detailed enriched goal text here\"}

Then choose ONE of these based on the task:

FOR NEW FILES (Create) or SMALL FILES:
===CODE===
(complete minimal compilable source code)
===END===

FOR EDITING LARGE EXISTING FILES (over 100 lines):
===DIFF===
@@ after: anchor line text @@
+new lines to add

@@ replace: anchor line text @@
-old lines
+new lines
===END===

Diff anchors: after/before/replace/start/end. Lines with + are added, - are removed.

GOAL RULES:
- PRESERVE all function names, prop names, type names from the skeleton goal
- ADD: exact parameter types, return types, behavior details, edge cases, state management
- For test files: describe what to test, expected inputs/outputs, scenarios

CODE RULES:
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
- NEVER use markdown fences (```). Just raw code between ===CODE=== and ===END===.

ABSOLUTE REQUIREMENT — YOUR OUTPUT MUST CONTAIN RAW SOURCE CODE:
- You MUST output actual source code between ===CODE=== and ===END===.
- NEVER output explanations, meta-comments, or descriptions instead of code.
- NEVER write text like '(no changes needed)' or '(file is unchanged)' — that is NOT code.
- If editing an existing file, output the COMPLETE file with your changes applied.
- If the file needs zero changes, output the EXACT existing file content unchanged.
- There is NO scenario where you skip writing code. ALWAYS output the full file.";

/// System prompt for final execution — full implementation.
pub const FINAL_PROMPT: &str = "\
You are a file implementer. You receive a WORKING scaffold file and a detailed goal.
Your job is to replace stub implementations with real, complete code.

OUTPUT FORMAT — choose based on task:

FOR NEW FILES (Create) or SMALL FILES (under 100 lines):
===CODE===
(complete source code — every line)
===END===

FOR EDITING LARGE EXISTING FILES (over 100 lines):
Use diff format to output ONLY the changes:
===DIFF===
@@ after: line text to anchor after @@
+new line to insert
+another new line

@@ replace: line text to find and replace @@
-old line to remove
+new replacement line

@@ before: line text to anchor before @@
+line to insert before anchor
===END===

Diff anchors:
- @@ after: TEXT @@ — insert additions after the line containing TEXT
- @@ before: TEXT @@ — insert additions before the line containing TEXT
- @@ replace: TEXT @@ — remove lines starting at TEXT, add replacements
- @@ start @@ — insert at file beginning
- @@ end @@ — insert at file end
- Lines starting with + are added, lines starting with - are removed

CRITICAL RULES:
1. PRESERVE all import statements EXACTLY as they are in the scaffold.
2. PRESERVE all export names EXACTLY.
3. REPLACE stub/placeholder implementations with real, working code.
4. The file must remain compilable after your changes.
5. For test files: replace placeholder tests with real tests.
6. NEVER change function signatures — same parameters, same return types.
7. NEVER use markdown fences (```). Just raw code between ===CODE=== and ===END===.
8. Output the COMPLETE file, every line from first to last.

ABSOLUTE REQUIREMENT — YOUR OUTPUT MUST CONTAIN RAW SOURCE CODE:
- You MUST output actual source code between ===CODE=== and ===END===.
- NEVER output explanations, meta-comments, or descriptions instead of code.
- NEVER write text like '(no changes needed)' or '(file is unchanged)' — that is NOT code.
- NEVER write '(The file content is identical...)' — that is NOT code.
- If the scaffold is already complete, output THE EXACT SAME CODE again.
- There is NO scenario where you skip writing code. ALWAYS output the full file.
- Your ONLY output is: ===CODE=== then every line of source code then ===END===.";

/// Parse combined scaffold+enrich output into (goal, code).
///
/// Expected format:
/// ```text
/// {"goal": "..."}
/// ===CODE===
/// source code
/// ===END===
/// ```
pub fn parse_scaffold_output(content: &str) -> (String, String) {
    let trimmed = content.trim();

    // Extract goal from JSON line before ===CODE===.
    let goal = extract_goal_json(trimmed);

    // Extract code between ===CODE=== and ===END===.
    let code = extract_code_block(trimmed);

    (goal, code)
}

/// Strip markdown code fences that LLMs sometimes add (public API).
pub fn strip_markdown_fences_public(text: &str) -> String {
    strip_markdown_fences(text)
}

/// Extract the goal from a JSON object before the code markers.
fn extract_goal_json(content: &str) -> String {
    // Find the JSON object with goal field.
    let json_part = if let Some(code_pos) = content.find("===CODE===") {
        content[..code_pos].trim()
    } else {
        content.trim()
    };

    // Try to parse as JSON.
    if let Some(start) = json_part.find('{') {
        if let Some(end) = json_part.rfind('}') {
            let json_str = &json_part[start..=end];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(goal) = val.get("goal").and_then(|g| g.as_str()) {
                    return goal.to_string();
                }
            }
        }
    }

    String::new()
}

/// Extract code between ===CODE=== and ===END=== markers.
fn extract_code_block(content: &str) -> String {
    let code_marker = "===CODE===";
    let end_marker = "===END===";

    let start = match content.find(code_marker) {
        Some(pos) => pos + code_marker.len(),
        None => {
            // Fallback: try old ===FILE_START=== markers.
            if let Some(pos) = content.find("===FILE_START===") {
                pos + "===FILE_START===".len()
            } else {
                return String::new();
            }
        }
    };

    let after = &content[start..];
    let code_start = after.strip_prefix('\n').unwrap_or(after);

    let code = if let Some(end_pos) = code_start.find(end_marker) {
        code_start[..end_pos].trim_end()
    } else if let Some(end_pos) = code_start.find("===FILE_END===") {
        code_start[..end_pos].trim_end()
    } else {
        code_start.trim_end()
    };

    if code.is_empty() {
        return String::new();
    }

    strip_markdown_fences(code)
}

/// Strip markdown code fences that LLMs sometimes add despite instructions.
fn strip_markdown_fences(text: &str) -> String {
    let trimmed = text.trim();

    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let after_open = match trimmed.find('\n') {
        Some(pos) => &trimmed[pos + 1..],
        None => return trimmed.to_string(),
    };

    if let Some(pos) = after_open.rfind("```") {
        let before_close = after_open[..pos].trim_end();
        if before_close.is_empty() {
            return String::new();
        }
        before_close.to_string()
    } else {
        after_open.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_output() {
        let content = r#"{"goal": "Detailed goal here"}
===CODE===
const x = 1;
export function foo() { return x; }
===END==="#;
        let (goal, code) = parse_scaffold_output(content);
        assert_eq!(goal, "Detailed goal here");
        assert!(code.contains("const x = 1;"));
        assert!(code.contains("export function foo()"));
    }

    #[test]
    fn parse_code_only() {
        let content = "===CODE===\ncode only\n===END===";
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
    fn parse_strips_markdown_fences() {
        let content = "===CODE===\n```typescript\nconst x = 1;\n```\n===END===";
        let (_, code) = parse_scaffold_output(content);
        assert_eq!(code, "const x = 1;");
    }

    #[test]
    fn parse_old_markers_fallback() {
        let content = "===FILE_START===\nold format\n===FILE_END===";
        let (_, code) = parse_scaffold_output(content);
        assert_eq!(code, "old format");
    }

    #[test]
    fn parse_multiline_code() {
        let content = r#"{"goal": "Multi-line goal"}
===CODE===
import { db } from "@/lib/db";

export function getUsers() {
    return db.select().from(users);
}
===END==="#;
        let (goal, code) = parse_scaffold_output(content);
        assert_eq!(goal, "Multi-line goal");
        assert!(code.contains("import { db }"));
        assert!(code.contains("getUsers"));
    }
}
