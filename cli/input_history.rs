use std::path::PathBuf;

/// Persistent input history with Up/Down navigation.
///
/// Stored as JSONL (one JSON-encoded string per line) so that multi-line
/// prompts survive round-tripping without corruption.
pub struct InputHistory {
    entries: Vec<String>,
    nav_index: Option<usize>,
    draft: Option<String>,
    file_path: PathBuf,
}

impl InputHistory {
    /// Load history from `~/.harmoniis/harmonia/nodes/{label}/input_history.jsonl`.
    pub fn load(node_label: &str) -> Self {
        let file_path = crate::paths::node_dir(node_label)
            .map(|d| d.join("input_history.jsonl"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/harmonia_input_history.jsonl"));

        let entries = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<String>(line).ok())
                .collect()
        } else {
            Vec::new()
        };

        Self {
            entries,
            nav_index: None,
            draft: None,
            file_path,
        }
    }

    /// Append an entry. Deduplicates consecutive identical entries.
    pub fn push(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.entries.last().map(|s| s.as_str()) == Some(trimmed) {
            return;
        }
        self.entries.push(trimmed.to_string());

        // Append to file (best-effort)
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(trimmed) {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
            {
                let _ = writeln!(f, "{}", json);
            }
        }
    }

    /// Navigate up (older). On first call, saves `current` as draft.
    /// Returns the history entry to display, or None if at the top.
    pub fn navigate_up(&mut self, current: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        match self.nav_index {
            None => {
                // First press — save current text as draft
                self.draft = Some(current.to_string());
                let idx = self.entries.len() - 1;
                self.nav_index = Some(idx);
                Some(&self.entries[idx])
            }
            Some(idx) => {
                if idx > 0 {
                    let new_idx = idx - 1;
                    self.nav_index = Some(new_idx);
                    Some(&self.entries[new_idx])
                } else {
                    // Already at oldest — stay
                    Some(&self.entries[0])
                }
            }
        }
    }

    /// Navigate down (newer). Returns None past newest (caller should restore draft).
    pub fn navigate_down(&mut self) -> Option<&str> {
        match self.nav_index {
            None => None,
            Some(idx) => {
                if idx + 1 < self.entries.len() {
                    let new_idx = idx + 1;
                    self.nav_index = Some(new_idx);
                    Some(&self.entries[new_idx])
                } else {
                    // Past newest — restore draft
                    self.nav_index = None;
                    self.draft.as_deref()
                }
            }
        }
    }

    /// Reset navigation state (call after submit).
    pub fn reset_navigation(&mut self) {
        self.nav_index = None;
        self.draft = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_history() -> InputHistory {
        InputHistory {
            entries: Vec::new(),
            nav_index: None,
            draft: None,
            file_path: PathBuf::from("/tmp/harmonia_test_history.jsonl"),
        }
    }

    #[test]
    fn push_dedup() {
        let mut h = temp_history();
        h.push("hello");
        h.push("hello");
        assert_eq!(h.entries.len(), 1);
        h.push("world");
        assert_eq!(h.entries.len(), 2);
    }

    #[test]
    fn navigate_up_down() {
        let mut h = temp_history();
        h.push("first");
        h.push("second");
        h.push("third");

        // Up from empty draft
        assert_eq!(h.navigate_up("current"), Some("third"));
        assert_eq!(h.navigate_up("current"), Some("second"));
        assert_eq!(h.navigate_up("current"), Some("first"));
        // Can't go past oldest
        assert_eq!(h.navigate_up("current"), Some("first"));

        // Down
        assert_eq!(h.navigate_down(), Some("second"));
        assert_eq!(h.navigate_down(), Some("third"));
        // Past newest — restore draft
        assert_eq!(h.navigate_down(), Some("current"));
        // Further down returns None
        assert_eq!(h.navigate_down(), None);
    }

    #[test]
    fn empty_history() {
        let mut h = temp_history();
        assert_eq!(h.navigate_up("draft"), None);
    }

    #[test]
    fn reset_navigation() {
        let mut h = temp_history();
        h.push("entry");
        h.navigate_up("draft");
        h.reset_navigation();
        assert!(h.nav_index.is_none());
        assert!(h.draft.is_none());
    }
}
