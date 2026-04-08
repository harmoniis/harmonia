// ── Input: trait and utility functions for terminal input handling ─────

use unicode_width::UnicodeWidthChar;

/// Terminal width in columns.
pub(crate) fn term_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

/// Character count (not byte count).
pub(crate) fn char_len(input: &str) -> usize {
    input.chars().count()
}

/// Byte index for the nth character.
pub(crate) fn byte_index_for_char(input: &str, char_index: usize) -> usize {
    if char_index == 0 { return 0; }
    input.char_indices().nth(char_index).map(|(idx, _)| idx).unwrap_or_else(|| input.len())
}

/// Display width accounting for wide characters.
pub(crate) fn display_width(text: &str) -> usize {
    text.chars().map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0)).sum()
}

/// Callbacks the input loop uses to interact with the host binary's
/// edit buffer, input history, and draft store.
pub trait InputCallbacks {
    // ── EditBuffer ──
    fn buf_text(&self) -> &str;
    fn buf_cursor(&self) -> usize;
    fn buf_insert_char(&mut self, c: char);
    fn buf_backspace(&mut self) -> bool;
    fn buf_delete(&mut self) -> bool;
    fn buf_delete_word_back(&mut self) -> bool;
    fn buf_move_left(&mut self) -> bool;
    fn buf_move_right(&mut self) -> bool;
    fn buf_move_home(&mut self);
    fn buf_move_end(&mut self);
    fn buf_clear_line(&mut self);
    fn buf_set_text(&mut self, text: &str);
    fn buf_set_cursor(&mut self, pos: usize);
    fn buf_undo(&mut self) -> bool;
    fn buf_redo(&mut self) -> bool;
    fn buf_take(&mut self) -> String;

    // ── InputHistory ──
    fn history_navigate_up(&mut self, current: &str) -> Option<String>;
    fn history_navigate_down(&mut self) -> Option<String>;
    fn history_push(&mut self, entry: &str);
    fn history_reset_navigation(&mut self);

    // ── DraftStore ──
    fn draft_save(&self, text: &str);
    fn draft_load(&self) -> Option<String>;
    fn draft_clear(&self);

    // ── Workspace ──
    fn workspace(&self) -> Option<std::path::PathBuf>;
}

/// Snapshot text + cursor from callbacks, avoiding borrow conflicts.
pub(crate) fn snapshot(cb: &dyn InputCallbacks) -> (String, usize) {
    (cb.buf_text().to_string(), cb.buf_cursor())
}

// Re-export read_input_line from the input_loop module.
pub(crate) use crate::input_loop::read_input_line;
