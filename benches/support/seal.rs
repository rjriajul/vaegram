//! MTProto 2.0 encryption the way the server does it, for the benches.

use sha2::{Digest, Sha256};

/// Server to client: the msg_key comes from auth_key[96..128] and x = 8.
pub fn seal(
    auth_key: &[u8; 256],
    auth_key_id: &[u8; 8],
    session_id: &[u8; 8],
    msg_id: i64,
    seq: i32,
    body: &[u8],
) -> Vec<u8> {
    let padding = 12 + (16 - (32 + body.len() + 12) % 16) % 16;
    let mut packet = Vec::with_capacity(24 + 32 + body.len() + padding);
    packet.extend_from_slice(auth_key_id);
    packet.extend_from_slice(&[0; 16]);
    packet.extend_from_slice(&0i64.to_le_bytes());
    packet.extend_from_slice(session_id);
    packet.extend_from_slice(&msg_id.to_le_bytes());
    packet.extend_from_slice(&seq.to_le_bytes());
    packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
    packet.extend_from_slice(body);
    packet.resize(packet.len() + padding, 0);
    let hash = Sha256::new().chain_update(&auth_key[96..128]).chain_update(&packet[24..]).finalize();
    let msg_key: [u8; 16] = hash[8..24].try_into().unwrap();
    packet[8..24].copy_from_slice(&msg_key);
    let (key, iv) = warpcrypto::kdf(auth_key, &msg_key, false);
    warpcrypto::ige256_encrypt(&mut packet[24..], &key, &iv).unwrap();
    packet
}
