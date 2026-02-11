#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

/// Validate filename (no path separators, not empty, reasonable length)
pub fn is_valid_filename(filename: &str) -> bool {
    if filename.is_empty() {
        return false;
    }

    if filename.len() > 255 {
        return false;
    }

    // No path separators
    if filename.contains('/') || filename.contains('\\') {
        return false;
    }

    // No special names
    if filename == "." || filename == ".." {
        return false;
    }

    true
}
