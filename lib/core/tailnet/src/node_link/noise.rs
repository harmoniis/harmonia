use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use snow::{params::NoiseParams, Builder};
use std::net::TcpStream;

use super::peers::{KnownPeer, LocalIdentity};
use super::wire::{read_blob, write_blob, write_secure_header, MODE_IK, MODE_XX};
use super::SecurityContext;

const NOISE_XX_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const NOISE_IK_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

pub(crate) fn parse_noise_params(pattern: &str) -> Result<NoiseParams, String> {
    pattern.parse().map_err(|e| format!("noise params: {}", e))
}

pub(crate) fn secure_send_xx(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    expected_remote: Option<&KnownPeer>,
    payload: &[u8],
) -> Result<(), String> {
    write_secure_header(stream, MODE_XX, &local.public_key_id)?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_XX_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .build_initiator()
        .map_err(|e| format!("noise init xx: {}", e))?;
    let mut buffer = vec![0u8; payload.len() + 1024];
    let len1 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write1: {}", e))?;
    write_blob(stream, &buffer[..len1])?;

    let frame2 = read_blob(stream)?;
    handshake
        .read_message(&frame2, &mut buffer)
        .map_err(|e| format!("noise xx read2: {}", e))?;
    if let Some(expected) = expected_remote {
        let observed = handshake
            .get_remote_static()
            .ok_or_else(|| "noise xx missing responder static".to_string())?;
        if observed != expected.public_key.as_slice() {
            return Err("noise xx responder key mismatch".to_string());
        }
    }

    let len3 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write3: {}", e))?;
    write_blob(stream, &buffer[..len3])?;

    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise xx transport: {}", e))?;
    let cipher_len = transport
        .write_message(payload, &mut buffer)
        .map_err(|e| format!("noise xx encrypt: {}", e))?;
    write_blob(stream, &buffer[..cipher_len])
}

pub(crate) fn secure_send_ik(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    remote: &KnownPeer,
    payload: &[u8],
) -> Result<(), String> {
    write_secure_header(stream, MODE_IK, &local.public_key_id)?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_IK_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .remote_public_key(&remote.public_key)
        .map_err(|e| format!("noise remote key: {}", e))?
        .build_initiator()
        .map_err(|e| format!("noise init ik: {}", e))?;
    let mut buffer = vec![0u8; payload.len() + 1024];
    let len1 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise ik write1: {}", e))?;
    write_blob(stream, &buffer[..len1])?;

    let frame2 = read_blob(stream)?;
    handshake
        .read_message(&frame2, &mut buffer)
        .map_err(|e| format!("noise ik read2: {}", e))?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise ik transport: {}", e))?;
    let cipher_len = transport
        .write_message(payload, &mut buffer)
        .map_err(|e| format!("noise ik encrypt: {}", e))?;
    write_blob(stream, &buffer[..cipher_len])
}

pub(crate) fn recv_secure_xx(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    sender_key_id: &str,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let mut handshake = Builder::new(parse_noise_params(NOISE_XX_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .build_responder()
        .map_err(|e| format!("noise resp xx: {}", e))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let frame1 = read_blob(stream)?;
    handshake
        .read_message(&frame1, &mut buffer)
        .map_err(|e| format!("noise xx read1: {}", e))?;
    let len2 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write2: {}", e))?;
    write_blob(stream, &buffer[..len2])?;
    let frame3 = read_blob(stream)?;
    handshake
        .read_message(&frame3, &mut buffer)
        .map_err(|e| format!("noise xx read3: {}", e))?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise xx transport: {}", e))?;
    let remote_static = transport
        .get_remote_static()
        .ok_or_else(|| "noise xx missing initiator static".to_string())?
        .to_vec();
    let remote_key_id: String = URL_SAFE_NO_PAD
        .encode(&remote_static)
        .chars()
        .take(16)
        .collect();
    if !sender_key_id.is_empty() && sender_key_id != remote_key_id {
        return Err("noise xx sender key id mismatch".to_string());
    }
    let ciphertext = read_blob(stream)?;
    let plain_len = transport
        .read_message(&ciphertext, &mut buffer)
        .map_err(|e| format!("noise xx decrypt: {}", e))?;
    Ok((
        buffer[..plain_len].to_vec(),
        SecurityContext {
            transport_security: "noise-xx",
            remote_key_id,
        },
        remote_static,
    ))
}

pub(crate) fn recv_secure_ik(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    sender_key_id: &str,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let sender = super::peers::known_peer_by_key_id(sender_key_id)
        .ok_or_else(|| format!("noise ik unknown sender key id: {}", sender_key_id))?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_IK_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .remote_public_key(&sender.public_key)
        .map_err(|e| format!("noise remote key: {}", e))?
        .build_responder()
        .map_err(|e| format!("noise resp ik: {}", e))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let frame1 = read_blob(stream)?;
    handshake
        .read_message(&frame1, &mut buffer)
        .map_err(|e| format!("noise ik read1: {}", e))?;
    let len2 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise ik write2: {}", e))?;
    write_blob(stream, &buffer[..len2])?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise ik transport: {}", e))?;
    let remote_static = transport
        .get_remote_static()
        .ok_or_else(|| "noise ik missing initiator static".to_string())?
        .to_vec();
    let ciphertext = read_blob(stream)?;
    let plain_len = transport
        .read_message(&ciphertext, &mut buffer)
        .map_err(|e| format!("noise ik decrypt: {}", e))?;
    Ok((
        buffer[..plain_len].to_vec(),
        SecurityContext {
            transport_security: "noise-ik",
            remote_key_id: sender.public_key_id.clone(),
        },
        remote_static,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use snow::Builder as SnowBuilder;
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::super::peers::LocalIdentity;
    use super::super::wire::{read_secure_header, NODE_LINK_MAGIC};

    fn test_identity() -> (LocalIdentity, Vec<u8>) {
        let builder = SnowBuilder::new(parse_noise_params(NOISE_XX_PATTERN).expect("noise params"));
        let keypair = builder.generate_keypair().expect("keypair");
        let encoded = URL_SAFE_NO_PAD.encode(&keypair.public);
        (
            LocalIdentity {
                public_key_id: encoded.chars().take(16).collect(),
                private_key: keypair.private,
            },
            keypair.public,
        )
    }

    #[test]
    fn secure_xx_wire_round_trip() {
        let (server_identity, server_public) = test_identity();
        let (client_identity, _) = test_identity();

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut magic = [0u8; 4];
            stream.read_exact(&mut magic).expect("read magic");
            assert_eq!(&magic, NODE_LINK_MAGIC);
            let (mode, sender_key_id) = read_secure_header(&mut stream).expect("header");
            assert_eq!(mode, MODE_XX);
            let (body, security, _) =
                recv_secure_xx(&mut stream, &server_identity, &sender_key_id).expect("recv xx");
            assert_eq!(security.transport_security, "noise-xx");
            assert!(!security.remote_key_id.is_empty());
            body
        });

        let mut client_stream = TcpStream::connect(addr).expect("connect");
        let known_server = KnownPeer {
            public_key: server_public,
            public_key_id: String::new(),
        };
        secure_send_xx(
            &mut client_stream,
            &client_identity,
            Some(&known_server),
            b"hello secure tailnet",
        )
        .expect("send xx");

        let body = server_thread.join().expect("join server");
        assert_eq!(body, b"hello secure tailnet");
    }
}
