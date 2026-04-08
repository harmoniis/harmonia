// ── Input: raw terminal input handling ────────────────────────────────

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossterm::{
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    terminal::{self, Clear, ClearType},
};
use unicode_width::UnicodeWidthChar;

use crate::autocomplete::*;
use crate::prompt::draw_prompt;
use crate::theme::*;

pub(crate) fn term_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

pub(crate) fn char_len(input: &str) -> usize {
    input.chars().count()
}

pub(crate) fn byte_index_for_char(input: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    input
        .char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| input.len())
}

pub(crate) fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
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

/// Helper: snapshot text + cursor from callbacks, avoiding borrow conflicts.
fn snapshot(cb: &dyn InputCallbacks) -> (String, usize) {
    (cb.buf_text().to_string(), cb.buf_cursor())
}

pub(crate) fn read_input_line(
    running: &Arc<AtomicBool>,
    cb: &mut dyn InputCallbacks,
    restore_draft: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    // Restore draft from crash/kill
    if restore_draft {
        if let Some(saved) = cb.draft_load() {
            cb.buf_set_text(&saved);
            eprintln!("  {DIM}Restored draft{RESET}");
        }
    }

    let mut ac_mode = AutocompleteMode::None;
    let mut box_height: u16 = 3;

    let workspace = cb.workspace();

    terminal::enable_raw_mode()?;

    std::io::stderr().flush()?;
    std::io::stdout().flush()?;

    let (_, start_row) = cursor::position()?;
    let (_, term_h) = terminal::size()?;
    let total_needed: u16 = 3 + SLASH_MENU_MAX as u16;
    let max_box_row = term_h.saturating_sub(total_needed);
    let box_row = if start_row > max_box_row {
        let deficit = start_row - max_box_row;
        let mut err = std::io::stderr();
        for _ in 0..deficit {
            let _ = write!(err, "\n");
        }
        let _ = err.flush();
        queue!(err, MoveTo(0, max_box_row))?;
        err.flush()?;
        max_box_row
    } else {
        start_row
    };

    {
        let (text, cursor) = snapshot(cb);
        box_height = draw_prompt(&text, cursor, box_row, box_height)?;
    }

    // Helper: update autocomplete menu after input changes
    let update_menu = |input: &str,
                       cursor_pos: usize,
                       ac_mode: &mut AutocompleteMode,
                       box_row: u16,
                       box_height: u16,
                       workspace: &Option<std::path::PathBuf>| {
        if input.starts_with('/') {
            let m = slash_matches(input);
            if !m.is_empty() {
                let sel = match ac_mode {
                    AutocompleteMode::Slash { selected } => (*selected).min(m.len() - 1),
                    _ => 0,
                };
                clear_menu(box_row, box_height);
                draw_slash_menu(box_row, box_height, &m, sel);
                *ac_mode = AutocompleteMode::Slash { selected: sel };
            } else {
                *ac_mode = AutocompleteMode::None;
                clear_menu(box_row, box_height);
            }
        } else if let Some(ws) = workspace {
            if let Some((token_start, partial)) = find_at_token(input, cursor_pos) {
                let matches = file_matches(ws, &partial);
                if !matches.is_empty() {
                    let sel = match ac_mode {
                        AutocompleteMode::File { selected, .. } => {
                            (*selected).min(matches.len() - 1)
                        }
                        _ => 0,
                    };
                    clear_menu(box_row, box_height);
                    draw_file_menu(box_row, box_height, &matches, sel);
                    *ac_mode = AutocompleteMode::File {
                        selected: sel,
                        matches,
                        token_start,
                    };
                } else {
                    *ac_mode = AutocompleteMode::None;
                    clear_menu(box_row, box_height);
                }
            } else {
                *ac_mode = AutocompleteMode::None;
                clear_menu(box_row, box_height);
            }
        } else {
            *ac_mode = AutocompleteMode::None;
            clear_menu(box_row, box_height);
        }
    };

    let result = loop {
        if !running.load(Ordering::Relaxed) {
            break Ok(String::new());
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    // Ctrl+C -- exit
                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        clear_menu(box_row, box_height);
                        running.store(false, Ordering::Relaxed);
                        break Ok(String::new());
                    }

                    // Ctrl+D -- exit on empty line
                    KeyEvent {
                        code: KeyCode::Char('d'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cb.buf_text().is_empty() {
                            clear_menu(box_row, box_height);
                            running.store(false, Ordering::Relaxed);
                            break Ok(String::new());
                        }
                    }

                    // Undo -- Ctrl+Z
                    KeyEvent {
                        code: KeyCode::Char('z'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cb.buf_undo() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                        }
                    }

                    // Redo -- Ctrl+Y
                    KeyEvent {
                        code: KeyCode::Char('y'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cb.buf_redo() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                        }
                    }

                    // Ctrl+U -- clear line
                    KeyEvent {
                        code: KeyCode::Char('u'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        cb.buf_clear_line();
                        let (text, cursor) = snapshot(cb);
                        box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                        cb.draft_save(&text);
                        update_menu(&text, cursor, &mut ac_mode, box_row, box_height, &workspace);
                    }

                    // Ctrl+A -- beginning of line
                    KeyEvent {
                        code: KeyCode::Char('a'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Home,
                        ..
                    } => {
                        cb.buf_move_home();
                        let (text, cursor) = snapshot(cb);
                        box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                    }

                    // Ctrl+E -- end of line
                    KeyEvent {
                        code: KeyCode::Char('e'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::End, ..
                    } => {
                        cb.buf_move_end();
                        let (text, cursor) = snapshot(cb);
                        box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                    }

                    // Ctrl+W -- delete word backward
                    KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cb.buf_delete_word_back() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                            update_menu(
                                &text, cursor, &mut ac_mode, box_row, box_height, &workspace,
                            );
                        }
                    }

                    // Up arrow
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => match &mut ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let (text, _) = snapshot(cb);
                            let m = slash_matches(&text);
                            if !m.is_empty() {
                                *selected = if *selected == 0 {
                                    m.len().min(SLASH_MENU_MAX) - 1
                                } else {
                                    *selected - 1
                                };
                                draw_slash_menu(box_row, box_height, &m, *selected);
                            }
                        }
                        AutocompleteMode::File {
                            selected, matches, ..
                        } => {
                            if !matches.is_empty() {
                                let max = matches.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected == 0 { max } else { *selected - 1 };
                                draw_file_menu(box_row, box_height, matches, *selected);
                            }
                        }
                        AutocompleteMode::None => {
                            let current = cb.buf_text().to_string();
                            if let Some(text) = cb.history_navigate_up(&current) {
                                cb.buf_set_text(&text);
                                let (t, c) = snapshot(cb);
                                box_height = draw_prompt(&t, c, box_row, box_height)?;
                            }
                        }
                    },

                    // Down arrow
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => match &mut ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let (text, _) = snapshot(cb);
                            let m = slash_matches(&text);
                            if !m.is_empty() {
                                let max = m.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected >= max { 0 } else { *selected + 1 };
                                draw_slash_menu(box_row, box_height, &m, *selected);
                            }
                        }
                        AutocompleteMode::File {
                            selected, matches, ..
                        } => {
                            if !matches.is_empty() {
                                let max = matches.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected >= max { 0 } else { *selected + 1 };
                                draw_file_menu(box_row, box_height, matches, *selected);
                            }
                        }
                        AutocompleteMode::None => {
                            if let Some(text) = cb.history_navigate_down() {
                                cb.buf_set_text(&text);
                            } else {
                                cb.buf_set_text("");
                            }
                            let (t, c) = snapshot(cb);
                            box_height = draw_prompt(&t, c, box_row, box_height)?;
                        }
                    },

                    // Enter -- submit or select file
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        match &ac_mode {
                            AutocompleteMode::Slash { selected } => {
                                let (text, _) = snapshot(cb);
                                let m = slash_matches(&text);
                                if *selected < m.len() {
                                    cb.buf_clear_line();
                                    for c in m[*selected].0.chars() {
                                        cb.buf_insert_char(c);
                                    }
                                }
                            }
                            AutocompleteMode::File {
                                selected,
                                matches,
                                token_start,
                            } => {
                                if *selected < matches.len() {
                                    let fm = &matches[*selected];
                                    let replacement = if fm.is_dir {
                                        format!("@{}", fm.full_path)
                                    } else {
                                        format!("@{} ", fm.full_path)
                                    };
                                    let is_dir = fm.is_dir;
                                    let ts = *token_start;
                                    let cur = cb.buf_cursor();
                                    let text = cb.buf_text().to_string();
                                    let at_start = byte_index_for_char(&text, ts);
                                    let at_end = byte_index_for_char(&text, cur);
                                    let mut new_text = String::new();
                                    new_text.push_str(&text[..at_start]);
                                    new_text.push_str(&replacement);
                                    new_text.push_str(&text[at_end..]);
                                    let new_cursor = ts + char_len(&replacement);
                                    cb.buf_set_text(&new_text);
                                    cb.buf_set_cursor(new_cursor);
                                    let (t, c) = snapshot(cb);
                                    box_height = draw_prompt(&t, c, box_row, box_height)?;
                                    cb.draft_save(&t);
                                    if is_dir {
                                        update_menu(
                                            &t,
                                            c,
                                            &mut ac_mode,
                                            box_row,
                                            box_height,
                                            &workspace,
                                        );
                                    } else {
                                        ac_mode = AutocompleteMode::None;
                                        clear_menu(box_row, box_height);
                                    }
                                    continue;
                                }
                            }
                            AutocompleteMode::None => {}
                        }
                        // Paste detection
                        if event::poll(std::time::Duration::from_millis(5)).unwrap_or(false) {
                            cb.buf_insert_char('\n');
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                            update_menu(
                                &text, cursor, &mut ac_mode, box_row, box_height, &workspace,
                            );
                            continue;
                        }
                        // No more events -- genuine submit
                        let mut err = std::io::stderr();
                        let total_rows = box_height + SLASH_MENU_MAX as u16;
                        for r in 0..total_rows {
                            let _ = queue!(
                                err,
                                MoveTo(0, box_row + r),
                                Clear(ClearType::CurrentLine)
                            );
                        }
                        let _ = queue!(err, MoveTo(0, box_row));
                        let _ = err.flush();
                        let submitted = cb.buf_take();
                        cb.draft_clear();
                        cb.history_push(&submitted);
                        cb.history_reset_navigation();
                        break Ok(submitted);
                    }

                    // Tab -- accept selected into input
                    KeyEvent {
                        code: KeyCode::Tab, ..
                    } => match &ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let sel = *selected;
                            let (text, _) = snapshot(cb);
                            let m = slash_matches(&text);
                            if sel < m.len() {
                                cb.buf_clear_line();
                                for c in m[sel].0.chars() {
                                    cb.buf_insert_char(c);
                                }
                                let (t, c) = snapshot(cb);
                                box_height = draw_prompt(&t, c, box_row, box_height)?;
                                cb.draft_save(&t);
                                update_menu(
                                    &t, c, &mut ac_mode, box_row, box_height, &workspace,
                                );
                            }
                        }
                        AutocompleteMode::File {
                            selected,
                            matches,
                            token_start,
                        } => {
                            if *selected < matches.len() {
                                let fm = matches[*selected].clone();
                                let ts = *token_start;
                                let cur = cb.buf_cursor();
                                let replacement = if fm.is_dir {
                                    format!("@{}", fm.full_path)
                                } else {
                                    format!("@{} ", fm.full_path)
                                };
                                let text = cb.buf_text().to_string();
                                let at_start = byte_index_for_char(&text, ts);
                                let at_end = byte_index_for_char(&text, cur);
                                let mut new_text = String::new();
                                new_text.push_str(&text[..at_start]);
                                new_text.push_str(&replacement);
                                new_text.push_str(&text[at_end..]);
                                let new_cursor = ts + char_len(&replacement);
                                cb.buf_set_text(&new_text);
                                cb.buf_set_cursor(new_cursor);
                                let (t, c) = snapshot(cb);
                                box_height = draw_prompt(&t, c, box_row, box_height)?;
                                cb.draft_save(&t);
                                if fm.is_dir {
                                    update_menu(
                                        &t, c, &mut ac_mode, box_row, box_height, &workspace,
                                    );
                                } else {
                                    ac_mode = AutocompleteMode::None;
                                    clear_menu(box_row, box_height);
                                }
                            }
                        }
                        AutocompleteMode::None => {}
                    },

                    // Escape -- close menu
                    KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        if !matches!(ac_mode, AutocompleteMode::None) {
                            ac_mode = AutocompleteMode::None;
                            clear_menu(box_row, box_height);
                        }
                    }

                    // Backspace
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        if cb.buf_backspace() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                            update_menu(
                                &text, cursor, &mut ac_mode, box_row, box_height, &workspace,
                            );
                        }
                    }

                    // Delete
                    KeyEvent {
                        code: KeyCode::Delete,
                        ..
                    } => {
                        if cb.buf_delete() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                            cb.draft_save(&text);
                            update_menu(
                                &text, cursor, &mut ac_mode, box_row, box_height, &workspace,
                            );
                        }
                    }

                    // Left arrow
                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    } => {
                        if cb.buf_move_left() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                        }
                    }

                    // Right arrow
                    KeyEvent {
                        code: KeyCode::Right,
                        ..
                    } => {
                        if cb.buf_move_right() {
                            let (text, cursor) = snapshot(cb);
                            box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                        }
                    }

                    // Regular character
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        ..
                    } => {
                        cb.buf_insert_char(c);
                        let (text, cursor) = snapshot(cb);
                        box_height = draw_prompt(&text, cursor, box_row, box_height)?;
                        cb.draft_save(&text);
                        update_menu(
                            &text, cursor, &mut ac_mode, box_row, box_height, &workspace,
                        );
                    }

                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    result
}
