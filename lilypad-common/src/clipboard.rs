//! Clipboard utilities for Lilypad.
//!
//! Provides secure clipboard operations with automatic clearing after a timeout.
//! Uses a generation counter to mitigate race conditions when clearing.

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Global generation counter for clipboard operations.
/// Each copy operation increments this counter, and the clear operation
/// only proceeds if the generation hasn't changed (meaning no new copy happened).
static CLIPBOARD_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Copies a value to the clipboard.
///
/// # Arguments
/// * `value` - The text to copy to clipboard
///
/// # Returns
/// Ok(()) on success, or an error if clipboard access fails.
pub fn copy_to_clipboard(value: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().map_err(|err| anyhow!("clipboard unavailable: {err}"))?;
    clipboard
        .set_text(value.to_string())
        .map_err(|err| anyhow!("failed to copy to clipboard: {err}"))?;
    // Increment generation on every copy
    CLIPBOARD_GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Copies a value to the clipboard and clears it after a timeout.
///
/// The clearing happens in a background thread. Uses a generation counter
/// to ensure we only clear if no new copy operation has occurred since
/// this one. This mitigates race conditions where the user might paste
/// elsewhere and then a new copy occurs before the timeout.
///
/// # Arguments
/// * `value` - The text to copy to clipboard
/// * `timeout_secs` - Seconds to wait before clearing (0 = never clear)
///
/// # Returns
/// Ok(()) on success, or an error if clipboard access fails.
pub fn copy_to_clipboard_with_timeout(value: &str, timeout_secs: u64) -> Result<()> {
    copy_to_clipboard(value)?;

    // Get generation after copying to track this specific copy operation
    let our_generation = CLIPBOARD_GENERATION.load(Ordering::SeqCst);

    if timeout_secs > 0 {
        let value = value.to_string();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(timeout_secs));

            // Only clear if no newer copy operation has occurred
            let current_generation = CLIPBOARD_GENERATION.load(Ordering::SeqCst);
            if current_generation != our_generation {
                // A newer copy operation occurred, don't clear
                return;
            }

            if let Ok(mut clipboard) = Clipboard::new() {
                // Double-check: only clear if clipboard still contains our value
                if clipboard.get_text().ok().as_deref() == Some(&value) {
                    let _ = clipboard.set_text(String::new());
                }
            }
        });
    }

    Ok(())
}

/// Clears the clipboard after a delay if it still contains the specified value.
///
/// This function spawns a background thread that waits for the specified
/// duration, then checks if the clipboard still contains the original value
/// before clearing it.
///
/// # Arguments
/// * `original_value` - The value that should trigger clearing
/// * `timeout_secs` - Seconds to wait before clearing
///
/// # Returns
/// A channel receiver that will receive `true` when clearing completes,
/// or `false` if the clipboard was modified by the user.
pub fn clear_clipboard_after(original_value: String, timeout_secs: u64) -> mpsc::Receiver<bool> {
    let (tx, rx) = mpsc::channel();

    if timeout_secs == 0 {
        let _ = tx.send(false);
        return rx;
    }

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(timeout_secs));
        let cleared = if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.get_text().ok().as_deref() == Some(&original_value) {
                clipboard.set_text(String::new()).is_ok()
            } else {
                false
            }
        } else {
            false
        };
        let _ = tx.send(cleared);
    });

    rx
}

/// Represents the state of a clipboard operation with automatic clearing.
pub struct ClipboardGuard {
    value: String,
    timeout_secs: u64,
    cleared: bool,
}

impl ClipboardGuard {
    /// Creates a new ClipboardGuard that will copy the value and schedule clearing.
    pub fn new(value: String, timeout_secs: u64) -> Result<Self> {
        copy_to_clipboard(&value)?;
        Ok(Self {
            value,
            timeout_secs,
            cleared: false,
        })
    }

    /// Starts the background clearing timer.
    pub fn start_clear_timer(&mut self) {
        if self.timeout_secs > 0 && !self.cleared {
            let value = self.value.clone();
            let timeout = self.timeout_secs;
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(timeout));
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.get_text().ok().as_deref() == Some(&value) {
                        let _ = clipboard.set_text(String::new());
                    }
                }
            });
            self.cleared = true;
        }
    }

    /// Returns the timeout in seconds.
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_clipboard_after_returns_receiver() {
        let rx = clear_clipboard_after("test".to_string(), 0);
        // With 0 timeout, should immediately return false
        assert!(!rx.recv().unwrap());
    }
}
