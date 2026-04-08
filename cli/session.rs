// ── Session: thin CLI wrapper around harmonia_tui ─────────────────────
//
// ALL TUI logic lives in the harmonia-tui library crate.
// Session creation, storage, and events are managed by the gateway session
// service (harmonia_gateway::sessions). This file implements SessionHost
// to bridge CLI-specific operations (paths, daemon management, frontend
// pairing) to the TUI library.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use console::{style, Term};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use harmonia_gateway::sessions as gsess;
use harmonia_tui::session::SessionHost;
use harmonia_tui::InputCallbacks;

use crate::session_flows;

// ── Public entry point ───────────────────────────────────────────────

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let node_identity = crate::paths::current_node_identity()?;
    let data_dir = crate::paths::data_dir()?;
    let session = Arc::new(
        gsess::create(&node_identity.label, &data_dir)
            .map_err(|e| format!("session create: {e}"))?,
    );
    harmonia_tui::run(&CliSessionHost {
        node_identity,
        data_dir,
        session,
    })
}

// ── CLI-specific SessionHost ─────────────────────────────────────────

struct CliSessionHost {
    node_identity: crate::paths::NodeIdentity,
    data_dir: PathBuf,
    session: Arc<gsess::Session>,
}

impl SessionHost for CliSessionHost {
    fn socket_path(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        crate::paths::socket_path()
    }

    fn data_dir(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        crate::paths::data_dir()
    }

    fn node_label(&self) -> &str { &self.node_identity.label }
    fn session_id(&self) -> &str { &self.session.id }

    fn ensure_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.node_identity.install_profile == crate::paths::InstallProfile::TuiClient {
            eprintln!("  {} Starting node service for {}...",
                style("\u{25c6}").cyan().bold(), self.node_identity.label);
            let _ = crate::pairing::ensure_pairing(&self.node_identity)?;
            crate::node_service::ensure_background(&self.node_identity)?;
        } else {
            eprintln!("  {} Starting daemon...", style("\u{25c6}").cyan().bold());
            crate::start::run("dev", false)?;
        }
        Ok(())
    }

    fn append_session_event(&self, actor: &str, kind: &str, text: &str) {
        let _ = gsess::append_event(&self.session, actor, kind, text);
    }

    fn create_input_callbacks(&self) -> Box<dyn InputCallbacks> {
        Box::new(CliInputCallbacks::new(&self.session, &self.node_identity.label))
    }

    fn print_help(&self) { session_flows::print_help_text(); }
    fn print_session_summary(&self) {
        session_flows::print_session_info(&self.session, &self.node_identity);
    }
    fn print_status(&self) { eprintln!("\n  Use 'harmonia status' for full system status.\n"); }
    fn print_providers(&self) {}
    fn print_recent_log(&self) { session_flows::print_log(); }

    fn clear_and_new_session(&self, term: &Term) {
        let node = match crate::paths::current_node_identity() {
            Ok(n) => n,
            Err(_) => {
                let _ = term.clear_screen();
                harmonia_tui::render::print_banner(
                    term, &self.session.node_label, &self.session.id);
                return;
            }
        };
        match gsess::create(&node.label, &self.data_dir) {
            Ok(s) => {
                let _ = term.clear_screen();
                harmonia_tui::render::print_banner(term, &s.node_label, &s.id);
                eprintln!("  \x1b[2mNew session started.\x1b[0m");
                eprintln!();
            }
            Err(_) => {
                let _ = term.clear_screen();
                harmonia_tui::render::print_banner(
                    term, &self.session.node_label, &self.session.id);
            }
        }
    }

    fn run_rewind_flow(&self, stdout: &mut std::io::Stdout, term: &Term) {
        if let Err(e) = session_flows::run_rewind(stdout, &self.session, term) {
            eprintln!("\n  \x1b[31mRewind error: {}\x1b[0m", e);
        }
    }

    fn run_resume_flow(&self, stdout: &mut std::io::Stdout) {
        if let Err(e) = session_flows::run_resume(
            stdout, &self.session, &self.node_identity.label, &self.data_dir,
        ) {
            eprintln!("\n  \x1b[31mResume error: {}\x1b[0m", e);
        }
    }

    fn run_policies_flow(&self, _stdout: &mut std::io::Stdout) {}

    fn run_frontends(&self, stdout: &mut std::io::Stdout) {
        match crate::paths::current_node_identity() {
            Ok(n) => {
                if let Err(e) = crate::frontend_pairing::run_pairing_menu(stdout, &n) {
                    eprintln!("\n  \x1b[31mFrontend error: {}\x1b[0m", e);
                }
            }
            Err(e) => eprintln!("\n  \x1b[31mCannot load node identity: {}\x1b[0m", e),
        }
    }

    #[cfg(unix)]
    fn run_menu_flow(
        &self, _stdout: &mut std::io::Stdout, _writer: &mut UnixStream,
        _waiting: &Arc<AtomicBool>, _running: &Arc<AtomicBool>,
        _reader_alive: &Arc<AtomicBool>, _response_buf: &Arc<Mutex<Vec<String>>>,
        _assistant_label: &str,
    ) {
        eprintln!("\n  \x1b[2mUse slash commands directly. /help for available commands.\x1b[0m\n");
    }
}

// ── InputCallbacks: bridges CLI edit buffer/history/draft to TUI ─────

struct CliInputCallbacks {
    buf: crate::edit_buffer::EditBuffer,
    history: crate::input_history::InputHistory,
    draft: crate::draft_store::DraftStore,
    workspace: Option<PathBuf>,
}

impl CliInputCallbacks {
    fn new(session: &gsess::Session, node_label: &str) -> Self {
        Self {
            buf: crate::edit_buffer::EditBuffer::new(),
            history: crate::input_history::InputHistory::load(node_label),
            draft: crate::draft_store::DraftStore::new(&session.events_dir),
            workspace: std::env::current_dir().ok()
                .or_else(|| crate::paths::user_workspace().ok()),
        }
    }
}

impl InputCallbacks for CliInputCallbacks {
    fn buf_text(&self) -> &str { self.buf.text() }
    fn buf_cursor(&self) -> usize { self.buf.cursor() }
    fn buf_insert_char(&mut self, c: char) { self.buf.insert_char(c); }
    fn buf_backspace(&mut self) -> bool { self.buf.backspace() }
    fn buf_delete(&mut self) -> bool { self.buf.delete() }
    fn buf_delete_word_back(&mut self) -> bool { self.buf.delete_word_back() }
    fn buf_move_left(&mut self) -> bool { self.buf.move_left() }
    fn buf_move_right(&mut self) -> bool { self.buf.move_right() }
    fn buf_move_home(&mut self) { self.buf.move_home(); }
    fn buf_move_end(&mut self) { self.buf.move_end(); }
    fn buf_clear_line(&mut self) { self.buf.clear_line(); }
    fn buf_set_text(&mut self, text: &str) { self.buf.set_text(text); }
    fn buf_set_cursor(&mut self, pos: usize) { self.buf.set_cursor(pos); }
    fn buf_undo(&mut self) -> bool { self.buf.undo() }
    fn buf_redo(&mut self) -> bool { self.buf.redo() }
    fn buf_take(&mut self) -> String { self.buf.take() }

    fn history_navigate_up(&mut self, current: &str) -> Option<String> {
        self.history.navigate_up(current).map(|s| s.to_string())
    }
    fn history_navigate_down(&mut self) -> Option<String> {
        self.history.navigate_down().map(|s| s.to_string())
    }
    fn history_push(&mut self, entry: &str) { self.history.push(entry); }
    fn history_reset_navigation(&mut self) { self.history.reset_navigation(); }
    fn draft_save(&self, text: &str) { self.draft.save(text); }
    fn draft_load(&self) -> Option<String> { self.draft.load() }
    fn draft_clear(&self) { self.draft.clear(); }
    fn workspace(&self) -> Option<PathBuf> { self.workspace.clone() }
}
