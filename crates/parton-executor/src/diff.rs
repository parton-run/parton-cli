//! Diff-based edit: parse and apply LLM-generated diffs.
//!
//! For Edit mode on large files, the LLM outputs a diff instead of
//! the entire file. This avoids regenerating hundreds of unchanged lines.
//!
//! Format:
//! ```text
//! ===DIFF===
//! @@ after: export const subscriptions @@
//! +export const adminRoles = pgTable('admin_roles', {
//! +  id: text('id').primaryKey(),
//! +});
//!
//! @@ replace: export function isAdmin @@
//! -export function isAdmin(email: string): boolean {
//! -  return ADMIN_EMAILS.includes(email);
//! -}
//! +export function isAdmin(email: string, roles: Role[]): boolean {
//! +  return ADMIN_EMAILS.includes(email) || roles.length > 0;
//! +}
//!
//! @@ before: export default @@
//! +import { adminRoles } from './role-tables';
//! ===END===
//! ```

/// A single hunk in a diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Where to apply this hunk.
    pub anchor: Anchor,
    /// Lines to remove (without `-` prefix).
    pub removals: Vec<String>,
    /// Lines to add (without `+` prefix).
    pub additions: Vec<String>,
}

/// Where a diff hunk should be applied.
#[derive(Debug, Clone)]
pub enum Anchor {
    /// Insert after the line containing this text.
    After(String),
    /// Insert before the line containing this text.
    Before(String),
    /// Replace lines matching this text.
    Replace(String),
    /// Insert at the beginning of the file.
    Start,
    /// Insert at the end of the file.
    End,
}

/// Parse a diff from between ===DIFF=== and ===END=== markers.
pub fn parse_diff(content: &str) -> Option<Vec<DiffHunk>> {
    let trimmed = content.trim();

    let diff_str = extract_diff_block(trimmed)?;
    let hunks = parse_hunks(&diff_str);

    if hunks.is_empty() {
        None
    } else {
        Some(hunks)
    }
}

/// Extract content between ===DIFF=== and ===END=== markers.
fn extract_diff_block(content: &str) -> Option<String> {
    let start = content.find("===DIFF===")?;
    let after = &content[start + "===DIFF===".len()..];
    let after = after.strip_prefix('\n').unwrap_or(after);

    let end = after.find("===END===").unwrap_or(after.len());
    let block = after[..end].trim_end();

    if block.is_empty() {
        None
    } else {
        Some(block.to_string())
    }
}

/// Parse hunks from the diff block.
fn parse_hunks(block: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_anchor: Option<Anchor> = None;
    let mut removals = Vec::new();
    let mut additions = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();

        // Hunk header: @@ after: ... @@ or @@ replace: ... @@
        if trimmed.starts_with("@@") && trimmed.ends_with("@@") {
            // Save previous hunk if any.
            if let Some(anchor) = current_anchor.take() {
                if !additions.is_empty() || !removals.is_empty() {
                    hunks.push(DiffHunk {
                        anchor,
                        removals: std::mem::take(&mut removals),
                        additions: std::mem::take(&mut additions),
                    });
                }
            }
            current_anchor = parse_anchor(trimmed);
            continue;
        }

        // Removal line.
        if let Some(rest) = line.strip_prefix('-') {
            removals.push(rest.to_string());
            continue;
        }

        // Addition line.
        if let Some(rest) = line.strip_prefix('+') {
            additions.push(rest.to_string());
            continue;
        }

        // Empty lines between hunks — ignore.
    }

    // Save last hunk.
    if let Some(anchor) = current_anchor {
        if !additions.is_empty() || !removals.is_empty() {
            hunks.push(DiffHunk {
                anchor,
                removals,
                additions,
            });
        }
    }

    hunks
}

/// Parse an anchor from a hunk header line.
fn parse_anchor(header: &str) -> Option<Anchor> {
    let inner = header.trim_start_matches('@').trim_end_matches('@').trim();

    if let Some(text) = inner.strip_prefix("after:") {
        Some(Anchor::After(text.trim().to_string()))
    } else if let Some(text) = inner.strip_prefix("before:") {
        Some(Anchor::Before(text.trim().to_string()))
    } else if let Some(text) = inner.strip_prefix("replace:") {
        Some(Anchor::Replace(text.trim().to_string()))
    } else if inner == "start" {
        Some(Anchor::Start)
    } else if inner == "end" {
        Some(Anchor::End)
    } else {
        None
    }
}

/// Apply a list of diff hunks to an existing file.
///
/// Returns the patched file content, or None if any hunk fails.
pub fn apply_diff(original: &str, hunks: &[DiffHunk]) -> Option<String> {
    let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();

    for hunk in hunks {
        lines = apply_hunk(lines, hunk)?;
    }

    Some(lines.join("\n"))
}

/// Apply a single hunk to lines.
fn apply_hunk(mut lines: Vec<String>, hunk: &DiffHunk) -> Option<Vec<String>> {
    match &hunk.anchor {
        Anchor::Start => {
            let mut new = hunk.additions.clone();
            new.extend(lines);
            Some(new)
        }
        Anchor::End => {
            lines.extend(hunk.additions.clone());
            Some(lines)
        }
        Anchor::After(text) => {
            let pos = find_line_containing(&lines, text)?;
            let insert_at = pos + 1;
            for (i, line) in hunk.additions.iter().enumerate() {
                lines.insert(insert_at + i, line.clone());
            }
            Some(lines)
        }
        Anchor::Before(text) => {
            let pos = find_line_containing(&lines, text)?;
            for (i, line) in hunk.additions.iter().enumerate() {
                lines.insert(pos + i, line.clone());
            }
            Some(lines)
        }
        Anchor::Replace(text) => {
            let pos = find_line_containing(&lines, text)?;
            // Remove lines matching removals starting at anchor.
            let remove_count = if hunk.removals.is_empty() {
                1 // Remove just the anchor line.
            } else {
                hunk.removals.len()
            };
            let end = (pos + remove_count).min(lines.len());
            lines.drain(pos..end);
            for (i, line) in hunk.additions.iter().enumerate() {
                lines.insert(pos + i, line.clone());
            }
            Some(lines)
        }
    }
}

/// Find the first line containing the given text.
fn find_line_containing(lines: &[String], text: &str) -> Option<usize> {
    lines.iter().position(|l| l.contains(text))
}

/// Check if LLM response contains a diff (===DIFF===) instead of full code.
pub fn is_diff_response(content: &str) -> bool {
    content.contains("===DIFF===")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_diff() {
        let input = "===DIFF===\n@@ after: const x = 1 @@\n+const y = 2;\n===END===";
        let hunks = parse_diff(input).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].additions, vec!["const y = 2;"]);
    }

    #[test]
    fn parse_replace_diff() {
        let input = "===DIFF===\n@@ replace: old line @@\n-old line\n+new line\n===END===";
        let hunks = parse_diff(input).unwrap();
        assert_eq!(hunks[0].removals, vec!["old line"]);
        assert_eq!(hunks[0].additions, vec!["new line"]);
    }

    #[test]
    fn apply_after() {
        let original = "line 1\nline 2\nline 3";
        let hunks = vec![DiffHunk {
            anchor: Anchor::After("line 2".into()),
            removals: vec![],
            additions: vec!["inserted".into()],
        }];
        let result = apply_diff(original, &hunks).unwrap();
        assert_eq!(result, "line 1\nline 2\ninserted\nline 3");
    }

    #[test]
    fn apply_before() {
        let original = "line 1\nline 2";
        let hunks = vec![DiffHunk {
            anchor: Anchor::Before("line 2".into()),
            removals: vec![],
            additions: vec!["inserted".into()],
        }];
        let result = apply_diff(original, &hunks).unwrap();
        assert_eq!(result, "line 1\ninserted\nline 2");
    }

    #[test]
    fn apply_replace() {
        let original = "a\nold\nb";
        let hunks = vec![DiffHunk {
            anchor: Anchor::Replace("old".into()),
            removals: vec!["old".into()],
            additions: vec!["new".into()],
        }];
        let result = apply_diff(original, &hunks).unwrap();
        assert_eq!(result, "a\nnew\nb");
    }

    #[test]
    fn apply_start() {
        let original = "existing";
        let hunks = vec![DiffHunk {
            anchor: Anchor::Start,
            removals: vec![],
            additions: vec!["header".into()],
        }];
        let result = apply_diff(original, &hunks).unwrap();
        assert_eq!(result, "header\nexisting");
    }

    #[test]
    fn apply_end() {
        let original = "existing";
        let hunks = vec![DiffHunk {
            anchor: Anchor::End,
            removals: vec![],
            additions: vec!["footer".into()],
        }];
        let result = apply_diff(original, &hunks).unwrap();
        assert_eq!(result, "existing\nfooter");
    }

    #[test]
    fn is_diff_detects() {
        assert!(is_diff_response("===DIFF===\n+x\n===END==="));
        assert!(!is_diff_response("===CODE===\nx\n===END==="));
    }

    #[test]
    fn apply_missing_anchor_returns_none() {
        let original = "a\nb";
        let hunks = vec![DiffHunk {
            anchor: Anchor::After("nonexistent".into()),
            removals: vec![],
            additions: vec!["x".into()],
        }];
        assert!(apply_diff(original, &hunks).is_none());
    }

    #[test]
    fn multiple_hunks() {
        let input =
            "===DIFF===\n@@ after: line 1 @@\n+new1\n\n@@ before: line 3 @@\n+new2\n===END===";
        let hunks = parse_diff(input).unwrap();
        assert_eq!(hunks.len(), 2);

        let original = "line 1\nline 2\nline 3";
        let result = apply_diff(original, &hunks).unwrap();
        assert!(result.contains("new1"));
        assert!(result.contains("new2"));
    }
}
