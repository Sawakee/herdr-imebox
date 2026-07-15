//! Sent-message history, stored as one JSON string per line.

use std::fs;
use std::path::Path;

pub fn load(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<String>(l).ok())
        .collect()
}

/// Append a sent message, skipping consecutive duplicates and trimming the
/// file to the most recent `max` entries. Best-effort: failures are ignored.
pub fn append(path: &Path, text: &str, max: usize) {
    let mut items = load(path);
    if items.last().map(String::as_str) == Some(text) {
        return;
    }
    items.push(text.to_owned());
    let skip = items.len().saturating_sub(max);
    let body: String = items
        .iter()
        .skip(skip)
        .filter_map(|t| serde_json::to_string(t).ok())
        .map(|l| l + "\n")
        .collect();
    let _ = fs::write(path, body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("imebox-history-test-{name}-{}", std::process::id()))
    }

    #[test]
    fn roundtrip_multiline_and_trim() {
        let p = temp_path("roundtrip");
        let _ = fs::remove_file(&p);
        append(&p, "1行目\n2行目", 2);
        append(&p, "second", 2);
        append(&p, "second", 2); // consecutive duplicate is dropped
        assert_eq!(
            load(&p),
            vec!["1行目\n2行目".to_owned(), "second".to_owned()]
        );
        append(&p, "third", 2); // trims to the last 2
        assert_eq!(load(&p), vec!["second".to_owned(), "third".to_owned()]);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_empty() {
        assert!(load(&temp_path("missing")).is_empty());
    }
}
