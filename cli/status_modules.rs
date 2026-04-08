//! Runtime IPC module query for `harmonia status`.

#[cfg(unix)]
pub fn query_runtime_modules() -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error>>
{
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let data_dir = super::paths::data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
    }
    let _ = harmonia_config_store::init_v2();
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    let state_root =
        harmonia_config_store::get_config_or("harmonia-runtime", "global", "state-root", &default)
            .unwrap_or(default);
    let sock_path = std::path::PathBuf::from(state_root).join("runtime.sock");
    if !sock_path.exists() {
        return Err("runtime socket not found".into());
    }

    let mut stream = UnixStream::connect(&sock_path)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    let msg = b"(:modules :op \"list\")";
    let len = (msg.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(msg)?;
    stream.flush()?;

    let mut hdr = [0u8; 4];
    stream.read_exact(&mut hdr)?;
    let rlen = u32::from_be_bytes(hdr) as usize;
    let mut buf = vec![0u8; rlen];
    stream.read_exact(&mut buf)?;
    let sexp = String::from_utf8_lossy(&buf).to_string();

    let mut result = Vec::new();
    let chars: Vec<char> = sexp.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 5 < chars.len() && chars[i] == '(' && chars[i + 1] == ':' && chars[i + 2] == 'n' {
            let start = i;
            let mut depth = 0;
            let mut end = i;
            for j in start..chars.len() {
                if chars[j] == '(' {
                    depth += 1;
                } else if chars[j] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        end = j + 1;
                        break;
                    }
                }
            }
            let entry: String = chars[start..end].iter().collect();
            if let Some(name) = extract_sexp_quoted(&entry, ":name") {
                let status = extract_sexp_unquoted(&entry, ":status").unwrap_or_default();
                let needs = extract_sexp_quoted(&entry, ":needs").unwrap_or_default();
                result.push((name, status, needs));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Ok(result)
}

#[cfg(unix)]
fn extract_sexp_quoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let bytes = after[1..].as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        if bytes[end] == b'"' {
            return Some(
                after[1..1 + end]
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\"),
            );
        }
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    None
}

#[cfg(unix)]
fn extract_sexp_unquoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    let val: String = after
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '"')
        .collect();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}
