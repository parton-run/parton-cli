//! In-place progress display for parallel operations.
//!
//! Shows all items at once, updates each line in-place when completed.
//! Uses ANSI escape codes for cursor movement.

use std::io::{self, Write};
use std::sync::Mutex;

/// Spinner frames for in-progress items.
const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Tracks parallel progress for a list of items.
pub struct ParallelProgress {
    items: Mutex<Vec<ItemState>>,
    frame: Mutex<usize>,
}

struct ItemState {
    label: String,
    status: Status,
    elapsed_ms: u64,
}

enum Status {
    Pending,
    Done,
    Failed,
}

impl ParallelProgress {
    /// Create progress for a list of labels. Immediately renders all items.
    pub fn new(labels: &[String]) -> Self {
        let items: Vec<ItemState> = labels
            .iter()
            .map(|l| ItemState {
                label: l.clone(),
                status: Status::Pending,
                elapsed_ms: 0,
            })
            .collect();

        let progress = Self {
            items: Mutex::new(items),
            frame: Mutex::new(0),
        };

        // Print initial lines.
        progress.render_all();
        progress
    }

    /// Mark an item as completed. Updates its line in-place.
    pub fn complete(&self, label: &str, elapsed_ms: u64, success: bool) {
        {
            let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(item) = items.iter_mut().find(|i| i.label == label) {
                item.status = if success { Status::Done } else { Status::Failed };
                item.elapsed_ms = elapsed_ms;
            }
        }
        self.render_all();
    }

    /// Render all items, moving cursor back to overwrite previous output.
    fn render_all(&self) {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        let mut frame = self.frame.lock().unwrap_or_else(|e| e.into_inner());
        *frame += 1;

        let line_count = items.len();

        // Move cursor up to the first item line (except on first render).
        if *frame > 1 {
            eprint!("\x1b[{}A", line_count);
        }

        for item in items.iter() {
            // Clear line and render.
            eprint!("\x1b[2K");
            match item.status {
                Status::Pending => {
                    let spinner = FRAMES[*frame % FRAMES.len()];
                    eprintln!("    \x1b[36m{spinner}\x1b[0m {}", item.label);
                }
                Status::Done => {
                    if item.elapsed_ms > 0 {
                        let secs = item.elapsed_ms as f64 / 1000.0;
                        eprintln!(
                            "    \x1b[32m✓\x1b[0m {} \x1b[2m({:.1}s)\x1b[0m",
                            item.label, secs
                        );
                    } else {
                        eprintln!("    \x1b[32m✓\x1b[0m {}", item.label);
                    }
                }
                Status::Failed => {
                    eprintln!("    \x1b[31m✗\x1b[0m {} \x1b[2m(failed)\x1b[0m", item.label);
                }
            }
        }
        let _ = io::stderr().flush();
    }

    /// Start a background ticker that refreshes spinner animation.
    /// Returns a handle that stops the ticker when dropped.
    pub fn start_ticker(&self) -> TickerHandle {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // We need a raw pointer to self for the thread. This is safe because
        // the caller guarantees ParallelProgress outlives the ticker.
        let self_ptr = self as *const Self as usize;

        let handle = std::thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(80));
                // Check if any items are still pending.
                let progress = unsafe { &*(self_ptr as *const Self) };
                let items = progress.items.lock().unwrap_or_else(|e| e.into_inner());
                let any_pending = items.iter().any(|i| matches!(i.status, Status::Pending));
                drop(items);

                if any_pending {
                    progress.render_all();
                } else {
                    break;
                }
            }
        });

        TickerHandle {
            running,
            handle: Some(handle),
        }
    }
}

/// Handle that stops the spinner ticker when dropped.
pub struct TickerHandle {
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for TickerHandle {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
