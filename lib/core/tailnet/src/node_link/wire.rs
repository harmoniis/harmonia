use std::io::{Read, Write};
use std::net::TcpStream;

pub(crate) const NODE_LINK_MAGIC: &[u8; 4] = b"HNL1";
pub(crate) const MODE_XX: u8 = 1;
pub(crate) const MODE_IK: u8 = 2;

pub(crate) fn write_u16(stream: &mut TcpStream, value: u16) -> Result<(), String> {
    stream
        .write_all(&value.to_be_bytes())
        .map_err(|e| format!("write u16: {}", e))
}

pub(crate) fn read_u16(stream: &mut TcpStream) -> Result<u16, String> {
    let mut buf = [0u8; 2];
    stream
        .read_exact(&mut buf)
        .map_err(|e| format!("read u16: {}", e))?;
    Ok(u16::from_be_bytes(buf))
}

pub(crate) fn write_blob(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), String> {
    let len = u32::try_from(bytes.len()).map_err(|_| "blob too large".to_string())?;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("write blob len: {}", e))?;
    stream
        .write_all(bytes)
        .map_err(|e| format!("write blob body: {}", e))
}

pub(crate) fn read_blob(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read blob len: {}", e))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 16 * 1024 * 1024 {
        return Err(format!("node-link blob too large: {}", len));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .map_err(|e| format!("read blob body: {}", e))?;
    Ok(body)
}

pub(crate) fn write_secure_header(
    stream: &mut TcpStream,
    mode: u8,
    sender_key_id: &str,
) -> Result<(), String> {
    stream
        .write_all(NODE_LINK_MAGIC)
        .map_err(|e| format!("write node-link magic: {}", e))?;
    stream
        .write_all(&[mode])
        .map_err(|e| format!("write node-link mode: {}", e))?;
    let sender = sender_key_id.as_bytes();
    let len = u16::try_from(sender.len()).map_err(|_| "sender key id too large".to_string())?;
    write_u16(stream, len)?;
    stream
        .write_all(sender)
        .map_err(|e| format!("write sender key id: {}", e))
}

pub(crate) fn read_secure_header(stream: &mut TcpStream) -> Result<(u8, String), String> {
    let mut mode = [0u8; 1];
    stream
        .read_exact(&mut mode)
        .map_err(|e| format!("read node-link mode: {}", e))?;
    let sender_len = read_u16(stream)? as usize;
    let mut sender_buf = vec![0u8; sender_len];
    if sender_len > 0 {
        stream
            .read_exact(&mut sender_buf)
            .map_err(|e| format!("read sender key id: {}", e))?;
    }
    let sender_key_id =
        String::from_utf8(sender_buf).map_err(|e| format!("sender key id utf8: {}", e))?;
    Ok((mode[0], sender_key_id))
}
