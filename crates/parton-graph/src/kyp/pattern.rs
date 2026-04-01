//! Pattern detection from export signatures.
//!
//! Finds repeated patterns across files in a cluster to produce
//! compact pattern strings like `async(req):NextResponse+guard`.

use crate::types::{CodeGraph, SymbolKind};

/// Detect the dominant pattern in a cluster of files.
///
/// Looks at exported function signatures and finds commonalities.
/// Returns a compact pattern string, or None if no clear pattern.
pub fn detect_pattern(file_paths: &[String], graph: &CodeGraph) -> Option<String> {
    let mut sig_parts: Vec<String> = Vec::new();

    for path in file_paths {
        let node = match graph.get_file(path) {
            Some(n) => n,
            None => continue,
        };
        for sym in &node.symbols {
            if !sym.exported || sym.kind != SymbolKind::Function {
                continue;
            }
            if let Some(part) = extract_pattern_part(&sym.signature) {
                sig_parts.push(part);
            }
        }
    }

    if sig_parts.is_empty() {
        return None;
    }

    // Find the most common pattern.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for part in &sig_parts {
        if let Some(entry) = counts.iter_mut().find(|(p, _)| p == part) {
            entry.1 += 1;
        } else {
            counts.push((part.clone(), 1));
        }
    }

    counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Only report patterns that appear 2+ times.
    counts
        .first()
        .filter(|(_, count)| *count >= 2)
        .map(|(pattern, _)| pattern.clone())
}

/// Extract a compact pattern part from a function signature.
///
/// `export async function GET(req: Request): NextResponse` → `async(req):NextResponse`
fn extract_pattern_part(sig: &str) -> Option<String> {
    let trimmed = sig.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_async = trimmed.contains("async");

    // Extract return type if present.
    let ret = if let Some(colon) = trimmed.rfind("): ") {
        let after = &trimmed[colon + 3..];
        let ret_type = after
            .split_whitespace()
            .next()
            .unwrap_or(after)
            .trim_end_matches('{');
        Some(ret_type.trim())
    } else {
        None
    };

    // Extract parameter pattern.
    let params = if let Some(open) = trimmed.find('(') {
        if let Some(close) = trimmed.find(')') {
            let inner = &trimmed[open + 1..close];
            if inner.is_empty() {
                "()"
            } else {
                let first_param = inner.split(',').next().unwrap_or(inner);
                let param_type = first_param.split(':').nth(1).unwrap_or(first_param).trim();
                param_type
            }
        } else {
            ""
        }
    } else {
        ""
    };

    let mut pattern = String::new();
    if is_async {
        pattern.push_str("async");
    }
    if !params.is_empty() {
        pattern.push_str(&format!("({params})"));
    }
    if let Some(ret) = ret {
        pattern.push_str(&format!(":{ret}"));
    }

    if pattern.is_empty() {
        None
    } else {
        Some(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_async_handler_pattern() {
        let sig = "export async function GET(req: Request): NextResponse";
        let part = extract_pattern_part(sig);
        assert!(part.is_some());
        let p = part.unwrap();
        assert!(p.contains("async"));
        assert!(p.contains("Request"));
    }

    #[test]
    fn extract_simple_function() {
        let sig = "export function checkAdmin(email: string): boolean";
        let part = extract_pattern_part(sig);
        assert!(part.is_some());
    }

    #[test]
    fn empty_sig_returns_none() {
        assert!(extract_pattern_part("").is_none());
    }
}
