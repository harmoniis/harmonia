//! Shell command execution RPC operations.

use harmonia_node_rpc::{NodePathRef, NodeRpcResult};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::helpers::{default_exec_cwd, resolve_path};

pub(crate) fn shell_exec(
    node: &crate::paths::NodeIdentity,
    program: &str,
    args: &[String],
    cwd: Option<&NodePathRef>,
    timeout_ms: u64,
) -> Result<NodeRpcResult, String> {
    let cwd = match cwd {
        Some(path_ref) => resolve_path(node, path_ref)?,
        None => default_exec_cwd(node)?,
    };
    let (status, stdout, stderr, timed_out) = run_command(program, args, &cwd, timeout_ms)?;
    Ok(NodeRpcResult::ShellExec {
        status,
        stdout,
        stderr,
        timed_out,
    })
}

fn run_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_ms: u64,
) -> Result<(Option<i32>, String, String, bool), String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {program} failed: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.clamp(1, 120_000));
    let mut timed_out = false;
    loop {
        if child
            .try_wait()
            .map_err(|e| format!("wait on {program} failed: {e}"))?
            .is_some()
        {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("collect output for {program} failed: {e}"))?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out,
    ))
}
