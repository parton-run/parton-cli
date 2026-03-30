//! TUI styling helpers (ANSI escape codes, no external deps).

/// Print a styled section header.
pub fn print_header(text: &str) {
    eprintln!("\n  \x1b[1;36m{text}\x1b[0m");
    eprintln!("  \x1b[2m{}\x1b[0m", "─".repeat(50));
}

/// Print a success checkmark line.
pub fn print_ok(text: &str) {
    eprintln!("  \x1b[1;32m✓\x1b[0m {text}");
}

/// Print an error line.
pub fn print_err(text: &str) {
    eprintln!("  \x1b[1;31m✗\x1b[0m {text}");
}

/// Print a key-value info line.
pub fn print_kv(key: &str, value: &str) {
    eprintln!("  \x1b[1m{key:>12}\x1b[0m  {value}");
}

/// Format text as dim.
pub fn dim(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[0m")
}
