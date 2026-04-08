mod noise;
pub(crate) mod peers;
pub(crate) mod wire;

use std::io::Read;
use std::net::TcpStream;

use self::noise::{recv_secure_ik, recv_secure_xx, secure_send_ik, secure_send_xx};
use self::peers::{
    load_local_identity, observed_peer_for_target, pairing_peer_for_target, remember_peer,
};
use self::wire::{read_secure_header, NODE_LINK_MAGIC, MODE_IK, MODE_XX};

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub transport_security: &'static str,
    pub remote_key_id: String,
}

pub fn write_secure_message(
    stream: &mut TcpStream,
    to_addr: &str,
    message_bytes: &[u8],
) -> Result<bool, String> {
    let Some(local) = load_local_identity() else {
        return Ok(false);
    };

    if let Some(peer) = observed_peer_for_target(to_addr) {
        secure_send_ik(stream, &local, &peer, message_bytes)?;
        return Ok(true);
    }

    if let Some(peer) = pairing_peer_for_target(to_addr) {
        secure_send_xx(stream, &local, Some(&peer), message_bytes)?;
        return Ok(true);
    }

    Ok(false)
}

fn read_secure_message(
    stream: &mut TcpStream,
    local: &peers::LocalIdentity,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let (mode, sender_key_id) = read_secure_header(stream)?;
    match mode {
        MODE_XX => recv_secure_xx(stream, local, &sender_key_id),
        MODE_IK => recv_secure_ik(stream, local, &sender_key_id),
        other => Err(format!("unknown node-link mode: {}", other)),
    }
}

pub fn secure_or_plain_body(
    stream: &mut TcpStream,
    first4: [u8; 4],
) -> Result<(Vec<u8>, Option<SecurityContext>), String> {
    if first4 == *NODE_LINK_MAGIC {
        let local = load_local_identity().ok_or_else(|| {
            "secure node-link message received but local node-link identity is missing".to_string()
        })?;
        let peer_addr = stream.peer_addr().ok();
        let (body, security, remote_static) = read_secure_message(stream, &local)?;
        let msg: crate::model::MeshMessage =
            serde_json::from_slice(&body).map_err(|e| format!("deserialize secure mesh: {}", e))?;
        remember_peer(&remote_static, &security.remote_key_id, peer_addr, &msg)?;
        Ok((body, Some(security)))
    } else {
        let len = u32::from_be_bytes(first4) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(format!("message too large: {} bytes", len));
        }
        let mut body = vec![0u8; len];
        stream
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {}", e))?;
        Ok((body, None))
    }
}

pub fn apply_security_context(msg: &mut crate::model::MeshMessage, security: &SecurityContext) {
    let origin = msg.origin.get_or_insert_with(|| crate::model::MeshOrigin {
        node_id: msg.from.clone(),
        node_label: None,
        node_role: None,
        channel_class: None,
        node_key_id: None,
        transport_security: None,
    });
    origin.node_key_id = Some(security.remote_key_id.clone());
    origin.transport_security = Some(security.transport_security.to_string());
}
