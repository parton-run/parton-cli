//! Simple progress output (no external deps).

/// Progress tracker for parallel file execution.
pub struct TurboProgress {
    files: Vec<String>,
}

/// Minimal overall progress counter.
pub struct OverallBar;

impl TurboProgress {
    /// Create a new progress tracker.
    pub fn new(_count: usize) -> Self {
        Self { files: Vec::new() }
    }

    /// Register a file. Returns its index.
    pub fn add_file(&mut self, path: &str) -> usize {
        self.files.push(path.to_string());
        self.files.len() - 1
    }

    /// Mark a file as completed.
    pub fn complete_file(&self, _index: usize, path: &str, elapsed_ms: u64, success: bool) {
        let secs = elapsed_ms as f64 / 1000.0;
        if success {
            eprintln!("    \x1b[32m✓\x1b[0m {} \x1b[2m({:.1}s)\x1b[0m", path, secs);
        } else {
            eprintln!("    \x1b[31m✗\x1b[0m {} \x1b[2m(failed)\x1b[0m", path);
        }
    }

    /// Create an overall progress bar.
    pub fn overall_bar(&self, _total: usize) -> OverallBar {
        OverallBar
    }
}

impl OverallBar {
    /// Increment progress.
    pub fn inc(&self, _n: u64) {}
    /// Clear the bar.
    pub fn finish_and_clear(&self) {}
}
