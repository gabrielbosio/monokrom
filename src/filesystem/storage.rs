#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::PathBuf;

/// Get the working directory for Monokrom files
/// - Debug mode: ./fs directory (relative to binary)
/// - Release mode: ~/.monokrom
pub fn get_working_directory() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let fs_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("fs");
        // Create directory if it doesn't exist
        if !fs_dir.exists() {
            let _ = fs::create_dir_all(&fs_dir);
        }
        fs_dir
    }

    #[cfg(not(debug_assertions))]
    {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let monokrom_dir = base_dirs.home_dir().join(".monokrom");
            // Create directory if it doesn't exist
            if !monokrom_dir.exists() {
                let _ = fs::create_dir_all(&monokrom_dir);
            }
            monokrom_dir
        } else {
            PathBuf::from(".")
        }
    }
}

/// List all files in the working directory (not directories)
pub fn list_files() -> io::Result<Vec<String>> {
    let dir = get_working_directory();
    let mut files = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if let Some(name) = filename.to_str() {
                        // Skip hidden files
                        if !name.starts_with('.') {
                            files.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Read a file from the working directory
pub fn read_file(filename: &str) -> io::Result<String> {
    let path = get_working_directory().join(filename);
    fs::read_to_string(path)
}

/// Write a file to the working directory
pub fn write_file(filename: &str, content: &str) -> io::Result<()> {
    let dir = get_working_directory();

    // Ensure directory exists
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let path = dir.join(filename);
    fs::write(path, content)
}

/// Check if a file exists in the working directory
pub fn file_exists(filename: &str) -> bool {
    let path = get_working_directory().join(filename);
    path.exists() && path.is_file()
}

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

/// Delete a file from the working directory
pub fn delete_file(filename: &str) -> io::Result<()> {
    let path = get_working_directory().join(filename);
    fs::remove_file(path)
}

/// Duplicate a file in the working directory
/// Returns the name of the new file
/// Naming scheme: original -> original0, original0 -> original1, etc.
pub fn duplicate_file(filename: &str) -> io::Result<String> {
    let dir = get_working_directory();
    let source_path = dir.join(filename);

    // Read source file content
    let content = fs::read_to_string(&source_path)?;

    // Find the next available name
    let new_name = find_next_duplicate_name(filename);
    let dest_path = dir.join(&new_name);

    // Write to new file
    fs::write(dest_path, content)?;

    Ok(new_name)
}

/// Find the next available name for a duplicate file
/// my_file -> my_file0, my_file0 -> my_file1, etc.
fn find_next_duplicate_name(filename: &str) -> String {
    let dir = get_working_directory();

    // Start with index 0
    let mut index = 0;
    loop {
        let candidate = format!("{}{}", filename, index);
        let candidate_path = dir.join(&candidate);
        if !candidate_path.exists() {
            return candidate;
        }
        index += 1;
    }
}
