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
        if !fs_dir.exists() {
            if let Err(e) = fs::create_dir_all(&fs_dir) {
                eprintln!("Warning: Could not create directory {:?}: {}", fs_dir, e);
            }
        }
        fs_dir
    }

    #[cfg(not(debug_assertions))]
    {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let monokrom_dir = base_dirs.home_dir().join(".monokrom");
            if !monokrom_dir.exists() {
                if let Err(e) = fs::create_dir_all(&monokrom_dir) {
                    eprintln!(
                        "Warning: Could not create directory {:?}: {}",
                        monokrom_dir, e
                    );
                }
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

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let path = dir.join(filename);
    fs::write(path, content)
}

/// Delete a file from the working directory
pub fn delete_file(filename: &str) -> io::Result<()> {
    let path = get_working_directory().join(filename);
    fs::remove_file(path)
}

/// Duplicate a file in the working directory
/// Returns the name of the new file
pub fn duplicate_file(filename: &str) -> io::Result<String> {
    let dir = get_working_directory();
    let source_path = dir.join(filename);
    let content = fs::read_to_string(&source_path)?;
    let new_name = find_next_duplicate_name(filename);
    let dest_path = dir.join(&new_name);
    fs::write(dest_path, content)?;
    Ok(new_name)
}

/// Find the next available name for a duplicate file
fn find_next_duplicate_name(filename: &str) -> String {
    let dir = get_working_directory();
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

#[cfg(test)]
mod tests {
    use crate::filesystem::storage::is_valid_filename;

    #[test]
    fn test_is_valid_filename_empty() {
        assert!(!is_valid_filename(""));
    }

    #[test]
    fn test_is_valid_filename_simple() {
        assert!(is_valid_filename("hello"));
        assert!(is_valid_filename("hello.txt"));
        assert!(is_valid_filename("my_file_123"));
    }

    #[test]
    fn test_is_valid_filename_with_slash() {
        assert!(!is_valid_filename("path/to/file"));
        assert!(!is_valid_filename("path\\to\\file"));
    }

    #[test]
    fn test_is_valid_filename_special_names() {
        assert!(!is_valid_filename("."));
        assert!(!is_valid_filename(".."));
    }

    #[test]
    fn test_is_valid_filename_too_long() {
        let long_name = "a".repeat(256);
        assert!(!is_valid_filename(&long_name));

        let ok_name = "a".repeat(255);
        assert!(is_valid_filename(&ok_name));
    }

    #[test]
    fn test_is_valid_filename_spaces() {
        assert!(is_valid_filename("hello world"));
        assert!(is_valid_filename(" leading space"));
        assert!(is_valid_filename("trailing space "));
    }

    #[test]
    fn test_is_valid_filename_dots() {
        assert!(is_valid_filename("..."));
        assert!(is_valid_filename(".hidden"));
        assert!(is_valid_filename("file.name.txt"));
    }
}
