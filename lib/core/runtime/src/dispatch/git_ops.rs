//! Git-ops component dispatch.
//!
//! Ops: status, log, diff, diff-full, branch, branch-current, commit, push.
//! All ops require :repo (path to git repo). Commit requires :message.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    let repo = extract_sexp_string(sexp, ":repo").unwrap_or_default();

    if repo.is_empty() && op != "healthcheck" {
        return "(:error \"git-ops: :repo required\")".to_string();
    }

    match op.as_str() {
        "healthcheck" => "(:ok :status \"git-ops ready\")".to_string(),

        "status" => match harmonia_git_ops::status(&repo) {
            Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },

        "log" => {
            let n: usize = extract_sexp_string(sexp, ":limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);
            match harmonia_git_ops::log_oneline(&repo, n) {
                Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }

        "diff" => match harmonia_git_ops::diff_summary(&repo) {
            Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },

        "diff-full" => match harmonia_git_ops::diff_full(&repo) {
            Ok(out) => {
                let trimmed = if out.len() > 4000 { &out[..4000] } else { &out };
                format!("(:ok :result \"{}\")", esc(trimmed.trim()))
            }
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },

        "branch" => match harmonia_git_ops::branch_list(&repo) {
            Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },

        "branch-current" => match harmonia_git_ops::current_branch(&repo) {
            Ok(out) => format!("(:ok :result \"{}\")", esc(&out)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },

        "commit" => {
            let message = extract_sexp_string(sexp, ":message").unwrap_or_default();
            if message.is_empty() {
                return "(:error \"git-ops commit: :message required\")".to_string();
            }
            let author = extract_sexp_string(sexp, ":author")
                .unwrap_or_else(|| "Harmonia".to_string());
            let email = extract_sexp_string(sexp, ":email")
                .unwrap_or_else(|| "harmonia@local.invalid".to_string());
            match harmonia_git_ops::commit(&repo, &message, &author, &email) {
                Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }

        "push" => {
            let remote = extract_sexp_string(sexp, ":remote")
                .unwrap_or_else(|| "origin".to_string());
            let branch = extract_sexp_string(sexp, ":branch")
                .unwrap_or_else(|| "main".to_string());
            match harmonia_git_ops::push(&repo, &remote, &branch) {
                Ok(out) => format!("(:ok :result \"{}\")", esc(out.trim())),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }

        _ => format!("(:error \"git-ops: unknown op '{}'\")", esc(&op)),
    }
}
