//! EditBuffer — text buffer with undo/redo history.
//!
//! Every mutation (insert, delete, replace) is recorded as an `Edit`.
//! Undo reverts the last edit, redo re-applies it.
//! Consecutive character inserts are coalesced into a single undo step.

/// A single reversible edit operation.
#[derive(Clone, Debug)]
enum Edit {
    /// Inserted `text` at byte position `pos`.
    Insert { pos: usize, text: String },
    /// Deleted `text` that was at byte position `pos`.
    Delete { pos: usize, text: String },
}

impl Edit {
    fn apply(&self, buf: &mut String) {
        match self {
            Edit::Insert { pos, text } => buf.insert_str(*pos, text),
            Edit::Delete { pos, text } => {
                buf.drain(*pos..*pos + text.len());
            }
        }
    }

    fn reverse(&self, buf: &mut String) {
        match self {
            Edit::Insert { pos, text } => {
                buf.drain(*pos..*pos + text.len());
            }
            Edit::Delete { pos, text } => buf.insert_str(*pos, text),
        }
    }
}

/// Text buffer with full undo/redo support.
pub struct EditBuffer {
    text: String,
    cursor: usize,                  // char position (not byte)
    undo_stack: Vec<(Edit, usize)>, // (edit, cursor_before)
    redo_stack: Vec<(Edit, usize)>, // (edit, cursor_before)
}

impl EditBuffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.char_len());
    }

    pub fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn take(&mut self) -> String {
        let result = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        result
    }

    /// Replace buffer contents entirely (for history navigation).
    /// Clears undo/redo stacks and positions cursor at end.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.char_len();
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    // ── Mutations (all recorded for undo) ──────────────────────────

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let byte_pos = self.byte_index(self.cursor);
        let mut s = String::new();
        s.push(ch);

        // Coalesce with previous insert if adjacent AND within the same word.
        // Whitespace boundaries break coalescing so each word is a separate
        // undo step (typing "hello world" = two undo steps: "hello " + "world").
        let coalesced = if let Some((Edit::Insert { pos, text }, _)) = self.undo_stack.last_mut() {
            if *pos + text.len() == byte_pos {
                let prev_is_ws = text.ends_with(char::is_whitespace);
                let cur_is_ws = ch.is_whitespace();
                // Break on word boundary: space→letter or letter→space
                if prev_is_ws != cur_is_ws {
                    false
                } else {
                    text.push(ch);
                    true
                }
            } else {
                false
            }
        } else {
            false
        };

        if !coalesced {
            self.undo_stack.push((
                Edit::Insert {
                    pos: byte_pos,
                    text: s.clone(),
                },
                self.cursor,
            ));
        }

        self.text.insert(byte_pos, ch);
        self.cursor += 1;
        self.redo_stack.clear();
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = self.byte_index(self.cursor - 1);
        let end = self.byte_index(self.cursor);
        let deleted: String = self.text[start..end].to_string();
        let cursor_before = self.cursor;

        self.text.drain(start..end);
        self.cursor -= 1;
        self.undo_stack.push((
            Edit::Delete {
                pos: start,
                text: deleted,
            },
            cursor_before,
        ));
        self.redo_stack.clear();
        true
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete(&mut self) -> bool {
        if self.cursor >= self.char_len() {
            return false;
        }
        let start = self.byte_index(self.cursor);
        let end = self.byte_index(self.cursor + 1);
        let deleted: String = self.text[start..end].to_string();
        let cursor_before = self.cursor;

        self.text.drain(start..end);
        self.undo_stack.push((
            Edit::Delete {
                pos: start,
                text: deleted,
            },
            cursor_before,
        ));
        self.redo_stack.clear();
        true
    }

    /// Delete the word before the cursor (Ctrl+W).
    pub fn delete_word_back(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let end_byte = self.byte_index(self.cursor);
        // Walk back past spaces, then past word chars
        let chars: Vec<char> = self.text[..end_byte].chars().collect();
        let mut i = chars.len();
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let start_byte = chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>();
        let deleted: String = self.text[start_byte..end_byte].to_string();
        let cursor_before = self.cursor;

        self.text.drain(start_byte..end_byte);
        self.cursor = i;
        self.undo_stack.push((
            Edit::Delete {
                pos: start_byte,
                text: deleted,
            },
            cursor_before,
        ));
        self.redo_stack.clear();
        true
    }

    /// Clear entire line (Ctrl+U).
    pub fn clear_line(&mut self) -> bool {
        if self.text.is_empty() {
            return false;
        }
        let old = self.text.clone();
        let cursor_before = self.cursor;
        self.text.clear();
        self.cursor = 0;
        self.undo_stack
            .push((Edit::Delete { pos: 0, text: old }, cursor_before));
        self.redo_stack.clear();
        true
    }

    // ── Undo / Redo ────────────────────────────────────────────────

    /// Undo the last edit. Returns true if something was undone.
    pub fn undo(&mut self) -> bool {
        if let Some((edit, cursor_before)) = self.undo_stack.pop() {
            let cursor_after = self.cursor;
            edit.reverse(&mut self.text);
            self.cursor = cursor_before;
            self.redo_stack.push((edit, cursor_after));
            true
        } else {
            false
        }
    }

    /// Redo the last undone edit. Returns true if something was redone.
    pub fn redo(&mut self) -> bool {
        if let Some((edit, cursor_before)) = self.redo_stack.pop() {
            let cursor_after = self.cursor;
            edit.apply(&mut self.text);
            self.cursor = cursor_before;
            self.undo_stack.push((edit, cursor_after));
            true
        } else {
            false
        }
    }

    // ── Cursor movement ────────────────────────────────────────────

    pub fn move_left(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }

    pub fn move_right(&mut self) -> bool {
        if self.cursor < self.char_len() {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }
    pub fn move_end(&mut self) {
        self.cursor = self.char_len();
    }

    // ── Internal ───────────────────────────────────────────────────

    fn byte_index(&self, char_pos: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_undo() {
        let mut buf = EditBuffer::new();
        buf.insert_char('h');
        buf.insert_char('i');
        assert_eq!(buf.text(), "hi");
        assert_eq!(buf.cursor(), 2);
        // Consecutive inserts coalesce — one undo reverts both
        buf.undo();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn undo_redo_cycle() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        assert_eq!(buf.text(), "abc");
        buf.undo();
        assert_eq!(buf.text(), "");
        buf.redo();
        assert_eq!(buf.text(), "abc");
    }

    #[test]
    fn backspace_undo() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.backspace();
        assert_eq!(buf.text(), "ab");
        buf.undo();
        assert_eq!(buf.text(), "abc");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn delete_word_undo() {
        let mut buf = EditBuffer::new();
        for c in "hello world".chars() {
            buf.insert_char(c);
        }
        buf.delete_word_back();
        assert_eq!(buf.text(), "hello ");
        buf.undo();
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn clear_line_undo() {
        let mut buf = EditBuffer::new();
        for c in "test".chars() {
            buf.insert_char(c);
        }
        buf.clear_line();
        assert_eq!(buf.text(), "");
        buf.undo();
        assert_eq!(buf.text(), "test");
    }

    #[test]
    fn redo_cleared_on_new_edit() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.undo();
        assert_eq!(buf.text(), "");
        buf.insert_char('b');
        // Redo should be gone
        assert!(!buf.redo());
        assert_eq!(buf.text(), "b");
    }

    #[test]
    fn non_coalesced_inserts() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.move_home(); // move cursor to 0
        buf.insert_char('b'); // insert at different position — separate undo step
        assert_eq!(buf.text(), "ba");
        buf.undo(); // undo 'b' insert
        assert_eq!(buf.text(), "a");
        buf.undo(); // undo 'a' insert
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn cursor_movement() {
        let mut buf = EditBuffer::new();
        for c in "abc".chars() {
            buf.insert_char(c);
        }
        assert_eq!(buf.cursor(), 3);
        buf.move_left();
        assert_eq!(buf.cursor(), 2);
        buf.move_home();
        assert_eq!(buf.cursor(), 0);
        buf.move_end();
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn word_level_undo() {
        let mut buf = EditBuffer::new();
        for c in "hello world".chars() {
            buf.insert_char(c);
        }
        assert_eq!(buf.text(), "hello world");
        // Undo removes "world" (second word)
        buf.undo();
        assert_eq!(buf.text(), "hello ");
        // Undo removes " " (whitespace group)
        buf.undo();
        assert_eq!(buf.text(), "hello");
        // Undo removes "hello" (first word)
        buf.undo();
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn set_text_clears_stacks() {
        let mut buf = EditBuffer::new();
        for c in "old".chars() {
            buf.insert_char(c);
        }
        buf.set_text("new text");
        assert_eq!(buf.text(), "new text");
        assert_eq!(buf.cursor(), 8);
        // Undo stack was cleared
        assert!(!buf.undo());
    }
}
