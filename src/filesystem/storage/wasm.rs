use std::io;

const KEY_PREFIX: &str = "mkr:";

fn prefixed(name: &str) -> String {
    format!("{}{}", KEY_PREFIX, name)
}

/// List all files stored in LocalStorage with mkr: prefix
pub fn list_files() -> io::Result<Vec<String>> {
    let storage = quad_storage::STORAGE.lock().unwrap();
    let mut files = Vec::new();
    for i in 0..storage.len() {
        if let Some(key) = storage.key(i) {
            if let Some(name) = key.strip_prefix(KEY_PREFIX) {
                if !name.is_empty() && !name.starts_with('.') {
                    files.push(name.to_string());
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Read a file from LocalStorage
pub fn read_file(filename: &str) -> io::Result<String> {
    let storage = quad_storage::STORAGE.lock().unwrap();
    storage
        .get(&prefixed(filename))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "file not found"))
}

/// Write a file to LocalStorage
pub fn write_file(filename: &str, content: &str) -> io::Result<()> {
    let mut storage = quad_storage::STORAGE.lock().unwrap();
    storage.set(&prefixed(filename), content);
    Ok(())
}

/// Delete a file from LocalStorage
pub fn delete_file(filename: &str) -> io::Result<()> {
    let mut storage = quad_storage::STORAGE.lock().unwrap();
    storage.remove(&prefixed(filename));
    Ok(())
}

/// Duplicate a file in LocalStorage
pub fn duplicate_file(filename: &str) -> io::Result<String> {
    let content = read_file(filename)?;
    let new_name = find_next_duplicate_name(filename);
    write_file(&new_name, &content)?;
    Ok(new_name)
}

/// Find the next available name for a duplicate file
fn find_next_duplicate_name(filename: &str) -> String {
    let storage = quad_storage::STORAGE.lock().unwrap();
    let mut index = 0;
    loop {
        let candidate = format!("{}{}", filename, index);
        if storage.get(&prefixed(&candidate)).is_none() {
            return candidate;
        }
        index += 1;
    }
}
