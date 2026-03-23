use std::path::PathBuf;

/// Best-effort autosave of in-progress input text.
///
/// Saves to `{session_dir}/draft.txt`. On crash/kill, the draft survives
/// and can be restored on the next session start.
pub struct DraftStore {
    path: PathBuf,
}

impl DraftStore {
    pub fn new(session: &crate::paths::SessionPaths) -> Self {
        Self {
            path: session.dir.join("draft.txt"),
        }
    }

    /// Save current text. Empty text deletes the file. Errors are ignored.
    pub fn save(&self, text: &str) {
        if text.is_empty() {
            let _ = std::fs::remove_file(&self.path);
        } else {
            let _ = std::fs::write(&self.path, text);
        }
    }

    /// Load saved draft. Returns None if missing or empty.
    pub fn load(&self) -> Option<String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Delete the draft file.
    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_clear() {
        let dir = std::env::temp_dir().join("harmonia_draft_test");
        let _ = std::fs::create_dir_all(&dir);

        let store = DraftStore {
            path: dir.join("draft.txt"),
        };

        // Initially no draft
        store.clear();
        assert!(store.load().is_none());

        // Save and load
        store.save("hello world");
        assert_eq!(store.load(), Some("hello world".to_string()));

        // Save empty clears
        store.save("");
        assert!(store.load().is_none());

        // Explicit clear
        store.save("something");
        store.clear();
        assert!(store.load().is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
