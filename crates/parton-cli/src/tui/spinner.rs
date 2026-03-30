//! Simple inline spinner for async operations.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// An inline spinner that runs in a background thread.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Start a spinner with a message (e.g. "Planning...").
    pub fn start(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let msg = message.to_string();

        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                // Use \x1b[2K to clear entire line, then \r to go to start.
                eprint!("\x1b[2K\r  {} {} ", frames[i % frames.len()], msg);
                let _ = io::stderr().flush();
                thread::sleep(Duration::from_millis(80));
                i += 1;
            }
            // Clear the line completely.
            eprint!("\x1b[2K\r");
            let _ = io::stderr().flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the spinner.
    pub fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
