//! Path compression — detect common prefixes and produce globs.

/// Compress a list of file paths into compact glob patterns.
///
/// Examples:
/// - `["lib/db/schema.ts", "lib/db/index.ts"]` → `["lib/db/[schema,index].ts"]`
/// - `["app/api/admin/roles/route.ts", "app/api/admin/users/route.ts"]` → `["app/api/admin/*/route.ts"]`
/// - `["lib/auth.ts"]` → `["lib/auth.ts"]` (no compression)
pub fn compress_paths(paths: &[String]) -> Vec<String> {
    if paths.len() <= 1 {
        return paths.to_vec();
    }

    let mut result: Vec<String> = Vec::new();
    let mut used = vec![false; paths.len()];

    // Try wildcard compression: same prefix + suffix with varying middle segment.
    try_wildcard_compress(paths, &mut result, &mut used);

    // Try bracket compression: same directory + extension, different filenames.
    try_bracket_compress(paths, &mut result, &mut used);

    // Add remaining uncompressed paths.
    for (i, path) in paths.iter().enumerate() {
        if !used[i] {
            result.push(path.clone());
        }
    }

    result.sort();
    result
}

/// Detect `prefix/*/suffix` patterns.
fn try_wildcard_compress(paths: &[String], result: &mut Vec<String>, used: &mut [bool]) {
    let mut patterns: std::collections::HashMap<(String, String), Vec<usize>> =
        std::collections::HashMap::new();

    for (i, path) in paths.iter().enumerate() {
        if used[i] {
            continue;
        }
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 3 {
            continue;
        }
        // Try replacing each middle segment with *.
        for skip in 1..parts.len() - 1 {
            let prefix = parts[..skip].join("/");
            let suffix = parts[skip + 1..].join("/");
            patterns.entry((prefix, suffix)).or_default().push(i);
        }
    }

    for ((prefix, suffix), indices) in &patterns {
        if indices.len() >= 3 {
            result.push(format!("{prefix}/*/{suffix}"));
            for &i in indices {
                used[i] = true;
            }
        }
    }
}

/// Detect `dir/[a,b,c].ext` patterns.
fn try_bracket_compress(paths: &[String], result: &mut Vec<String>, used: &mut [bool]) {
    let mut groups: std::collections::HashMap<(String, String), Vec<(usize, String)>> =
        std::collections::HashMap::new();

    for (i, path) in paths.iter().enumerate() {
        if used[i] {
            continue;
        }
        if let Some(slash) = path.rfind('/') {
            let dir = &path[..slash];
            let filename = &path[slash + 1..];
            if let Some(dot) = filename.rfind('.') {
                let stem = &filename[..dot];
                let ext = &filename[dot..];
                groups
                    .entry((dir.to_string(), ext.to_string()))
                    .or_default()
                    .push((i, stem.to_string()));
            }
        }
    }

    for ((dir, ext), items) in &groups {
        if items.len() >= 2 {
            let stems: Vec<&str> = items.iter().map(|(_, s)| s.as_str()).collect();
            result.push(format!("{dir}/[{}]{ext}", stems.join(",")));
            for (i, _) in items {
                used[*i] = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_path_unchanged() {
        let paths = vec!["lib/auth.ts".into()];
        assert_eq!(compress_paths(&paths), vec!["lib/auth.ts"]);
    }

    #[test]
    fn bracket_compression() {
        let paths = vec!["lib/db/schema.ts".into(), "lib/db/index.ts".into()];
        let compressed = compress_paths(&paths);
        assert_eq!(compressed.len(), 1);
        assert!(compressed[0].contains('['));
        assert!(compressed[0].contains("schema"));
        assert!(compressed[0].contains("index"));
    }

    #[test]
    fn wildcard_compression() {
        let paths = vec![
            "app/api/admin/roles/route.ts".into(),
            "app/api/admin/users/route.ts".into(),
            "app/api/admin/images/route.ts".into(),
        ];
        let compressed = compress_paths(&paths);
        assert!(compressed.iter().any(|p| p.contains('*')));
    }

    #[test]
    fn mixed_paths() {
        let paths = vec![
            "lib/db/schema.ts".into(),
            "lib/db/index.ts".into(),
            "lib/config.ts".into(),
        ];
        let compressed = compress_paths(&paths);
        assert!(compressed.len() <= 2);
    }
}
