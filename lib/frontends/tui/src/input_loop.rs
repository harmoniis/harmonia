// ── Input loop: keyboard event dispatch for the TUI input box ─────────

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossterm::{
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    terminal::{self, Clear, ClearType},
};

use crate::autocomplete::*;
use crate::input::{byte_index_for_char, char_len, snapshot, InputCallbacks};
use crate::prompt::draw_prompt;
use crate::theme::*;

pub(crate) fn read_input_line(
    running: &Arc<AtomicBool>, cb: &mut dyn InputCallbacks, restore_draft: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    if restore_draft {
        if let Some(saved) = cb.draft_load() {
            cb.buf_set_text(&saved); eprintln!("  {DIM}Restored draft{RESET}");
        }
    }
    let mut ac_mode = AutocompleteMode::None;
    let mut box_height: u16 = 3;
    let workspace = cb.workspace();
    terminal::enable_raw_mode()?;
    std::io::stderr().flush()?; std::io::stdout().flush()?;
    let (_, start_row) = cursor::position()?;
    let (_, term_h) = terminal::size()?;
    let max_box_row = term_h.saturating_sub(3 + SLASH_MENU_MAX as u16);
    let box_row = if start_row > max_box_row {
        let mut err = std::io::stderr();
        for _ in 0..(start_row - max_box_row) { let _ = write!(err, "\n"); }
        let _ = err.flush();
        queue!(err, MoveTo(0, max_box_row))?; err.flush()?; max_box_row
    } else { start_row };
    { let (t, c) = snapshot(cb); box_height = draw_prompt(&t, c, box_row, box_height)?; }

    // Macros for brevity in key handlers
    macro_rules! redraw {
        () => {{ let (t, c) = snapshot(cb); box_height = draw_prompt(&t, c, box_row, box_height)?; (t, c) }};
    }
    macro_rules! update_ac {
        ($t:expr, $c:expr) => { update_autocomplete($t, $c, &mut ac_mode, box_row, box_height, &workspace) };
    }

    let result = loop {
        if !running.load(Ordering::Relaxed) { break Ok(String::new()); }
        if !event::poll(std::time::Duration::from_millis(100))? { continue; }
        let Event::Key(key) = event::read()? else { continue; };

        match key {
            KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } => {
                clear_menu(box_row, box_height);
                running.store(false, Ordering::Relaxed);
                break Ok(String::new());
            }
            KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, .. } => {
                if cb.buf_text().is_empty() {
                    clear_menu(box_row, box_height);
                    running.store(false, Ordering::Relaxed);
                    break Ok(String::new());
                }
            }
            KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                if cb.buf_undo() { let (t, _) = redraw!(); cb.draft_save(&t); }
            }
            KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                if cb.buf_redo() { let (t, _) = redraw!(); cb.draft_save(&t); }
            }
            KeyEvent { code: KeyCode::Char('u'), modifiers: KeyModifiers::CONTROL, .. } => {
                cb.buf_clear_line();
                let (t, c) = redraw!(); cb.draft_save(&t); update_ac!(&t, c);
            }
            KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. }
            | KeyEvent { code: KeyCode::Home, .. } => { cb.buf_move_home(); redraw!(); }
            KeyEvent { code: KeyCode::Char('e'), modifiers: KeyModifiers::CONTROL, .. }
            | KeyEvent { code: KeyCode::End, .. } => { cb.buf_move_end(); redraw!(); }
            KeyEvent { code: KeyCode::Char('w'), modifiers: KeyModifiers::CONTROL, .. } => {
                if cb.buf_delete_word_back() {
                    let (t, c) = redraw!(); cb.draft_save(&t); update_ac!(&t, c);
                }
            }
            KeyEvent { code: KeyCode::Up, .. } => {
                handle_arrow(cb, &mut ac_mode, box_row, box_height, &mut box_height, true)?;
            }
            KeyEvent { code: KeyCode::Down, .. } => {
                handle_arrow(cb, &mut ac_mode, box_row, box_height, &mut box_height, false)?;
            }
            KeyEvent { code: KeyCode::Enter, .. } => {
                match handle_enter(cb, &mut ac_mode, box_row, &mut box_height, &workspace)? {
                    EnterResult::Continue => continue,
                    EnterResult::Submit(s) => break Ok(s),
                }
            }
            KeyEvent { code: KeyCode::Tab, .. } => {
                handle_tab(cb, &mut ac_mode, box_row, &mut box_height, &workspace)?;
            }
            KeyEvent { code: KeyCode::Esc, .. } => {
                if !matches!(ac_mode, AutocompleteMode::None) {
                    ac_mode = AutocompleteMode::None;
                    clear_menu(box_row, box_height);
                }
            }
            KeyEvent { code: KeyCode::Backspace, .. } => {
                if cb.buf_backspace() { let (t, c) = redraw!(); cb.draft_save(&t); update_ac!(&t, c); }
            }
            KeyEvent { code: KeyCode::Delete, .. } => {
                if cb.buf_delete() { let (t, c) = redraw!(); cb.draft_save(&t); update_ac!(&t, c); }
            }
            KeyEvent { code: KeyCode::Left, .. } => { if cb.buf_move_left() { redraw!(); } }
            KeyEvent { code: KeyCode::Right, .. } => { if cb.buf_move_right() { redraw!(); } }
            KeyEvent { code: KeyCode::Char(ch), modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT, .. } => {
                cb.buf_insert_char(ch);
                let (t, c) = redraw!(); cb.draft_save(&t); update_ac!(&t, c);
            }
            _ => {}
        }
    };
    terminal::disable_raw_mode()?;
    result
}

// ── Arrow key handlers ───────────────────────────────────────────────

fn cycle(sel: &mut usize, max: usize, up: bool) {
    *sel = if up { if *sel == 0 { max } else { *sel - 1 } }
           else  { if *sel >= max { 0 } else { *sel + 1 } };
}

fn handle_arrow(
    cb: &mut dyn InputCallbacks, ac: &mut AutocompleteMode,
    box_row: u16, bh: u16, out_bh: &mut u16, up: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match ac {
        AutocompleteMode::Slash { selected } => {
            let (text, _) = snapshot(cb);
            let m = slash_matches(&text);
            if !m.is_empty() {
                cycle(selected, m.len().min(SLASH_MENU_MAX) - 1, up);
                draw_slash_menu(box_row, bh, &m, *selected);
            }
        }
        AutocompleteMode::File { selected, matches, .. } => {
            if !matches.is_empty() {
                cycle(selected, matches.len().min(SLASH_MENU_MAX) - 1, up);
                draw_file_menu(box_row, bh, matches, *selected);
            }
        }
        AutocompleteMode::None if up => {
            let current = cb.buf_text().to_string();
            if let Some(text) = cb.history_navigate_up(&current) {
                cb.buf_set_text(&text);
                let (t, c) = snapshot(cb);
                *out_bh = draw_prompt(&t, c, box_row, bh)?;
            }
        }
        AutocompleteMode::None => {
            match cb.history_navigate_down() {
                Some(text) => cb.buf_set_text(&text),
                None => cb.buf_set_text(""),
            }
            let (t, c) = snapshot(cb);
            *out_bh = draw_prompt(&t, c, box_row, bh)?;
        }
    }
    Ok(())
}

// ── Enter/Tab handlers ───────────────────────────────────────────────

enum EnterResult { Continue, Submit(String) }

fn handle_enter(
    cb: &mut dyn InputCallbacks, ac: &mut AutocompleteMode,
    box_row: u16, bh: &mut u16, ws: &Option<std::path::PathBuf>,
) -> Result<EnterResult, Box<dyn std::error::Error>> {
    match ac {
        AutocompleteMode::Slash { selected } => {
            let (text, _) = snapshot(cb);
            let m = slash_matches(&text);
            if *selected < m.len() {
                cb.buf_clear_line();
                for c in m[*selected].0.chars() { cb.buf_insert_char(c); }
            }
        }
        AutocompleteMode::File { selected, matches, token_start } => {
            if *selected < matches.len() {
                let fm = matches[*selected].clone();
                let ts = *token_start;
                let cont = insert_file_match(cb, &fm, ts, ac, box_row, bh, ws)?;
                if cont { return Ok(EnterResult::Continue); }
            }
        }
        AutocompleteMode::None => {}
    }
    // Paste detection
    if event::poll(std::time::Duration::from_millis(5)).unwrap_or(false) {
        cb.buf_insert_char('\n');
        let (t, c) = snapshot(cb);
        *bh = draw_prompt(&t, c, box_row, *bh)?;
        cb.draft_save(&t);
        update_autocomplete(&t, c, ac, box_row, *bh, ws);
        return Ok(EnterResult::Continue);
    }
    // Genuine submit
    let mut err = std::io::stderr();
    for r in 0..(*bh + SLASH_MENU_MAX as u16) {
        let _ = queue!(err, MoveTo(0, box_row + r), Clear(ClearType::CurrentLine));
    }
    let _ = queue!(err, MoveTo(0, box_row));
    let _ = err.flush();
    let submitted = cb.buf_take();
    cb.draft_clear(); cb.history_push(&submitted); cb.history_reset_navigation();
    Ok(EnterResult::Submit(submitted))
}

fn handle_tab(
    cb: &mut dyn InputCallbacks, ac: &mut AutocompleteMode,
    box_row: u16, bh: &mut u16, ws: &Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    match ac {
        AutocompleteMode::Slash { selected } => {
            let sel = *selected;
            let (text, _) = snapshot(cb);
            let m = slash_matches(&text);
            if sel < m.len() {
                cb.buf_clear_line();
                for c in m[sel].0.chars() { cb.buf_insert_char(c); }
                let (t, c) = snapshot(cb);
                *bh = draw_prompt(&t, c, box_row, *bh)?;
                cb.draft_save(&t);
                update_autocomplete(&t, c, ac, box_row, *bh, ws);
            }
        }
        AutocompleteMode::File { selected, matches, token_start } => {
            if *selected < matches.len() {
                let _ = insert_file_match(cb, &matches[*selected].clone(), *token_start, ac, box_row, bh, ws)?;
            }
        }
        AutocompleteMode::None => {}
    }
    Ok(())
}

// ── File match insertion (shared by Enter + Tab) ─────────────────────

fn insert_file_match(
    cb: &mut dyn InputCallbacks, fm: &FileMatch, ts: usize,
    ac: &mut AutocompleteMode, box_row: u16, bh: &mut u16,
    ws: &Option<std::path::PathBuf>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let repl = if fm.is_dir { format!("@{}", fm.full_path) } else { format!("@{} ", fm.full_path) };
    let text = cb.buf_text().to_string();
    let new_text = format!("{}{}{}", &text[..byte_index_for_char(&text, ts)],
        repl, &text[byte_index_for_char(&text, cb.buf_cursor())..]);
    cb.buf_set_text(&new_text);
    cb.buf_set_cursor(ts + char_len(&repl));
    let (t, c) = snapshot(cb);
    *bh = draw_prompt(&t, c, box_row, *bh)?;
    cb.draft_save(&t);
    if fm.is_dir { update_autocomplete(&t, c, ac, box_row, *bh, ws); }
    else { *ac = AutocompleteMode::None; clear_menu(box_row, *bh); }
    Ok(true)
}

// ── Autocomplete update ──────────────────────────────────────────────

fn update_autocomplete(
    input: &str, cursor_pos: usize, ac: &mut AutocompleteMode,
    box_row: u16, bh: u16, ws: &Option<std::path::PathBuf>,
) {
    if input.starts_with('/') {
        let m = slash_matches(input);
        if !m.is_empty() {
            let sel = match ac { AutocompleteMode::Slash { selected } => (*selected).min(m.len() - 1), _ => 0 };
            clear_menu(box_row, bh);
            draw_slash_menu(box_row, bh, &m, sel);
            *ac = AutocompleteMode::Slash { selected: sel };
        } else { *ac = AutocompleteMode::None; clear_menu(box_row, bh); }
    } else if let Some(ws) = ws {
        if let Some((ts, partial)) = find_at_token(input, cursor_pos) {
            let matches = file_matches(ws, &partial);
            if !matches.is_empty() {
                let sel = match ac { AutocompleteMode::File { selected, .. } => (*selected).min(matches.len() - 1), _ => 0 };
                clear_menu(box_row, bh);
                draw_file_menu(box_row, bh, &matches, sel);
                *ac = AutocompleteMode::File { selected: sel, matches, token_start: ts };
            } else { *ac = AutocompleteMode::None; clear_menu(box_row, bh); }
        } else { *ac = AutocompleteMode::None; clear_menu(box_row, bh); }
    } else { *ac = AutocompleteMode::None; clear_menu(box_row, bh); }
}
