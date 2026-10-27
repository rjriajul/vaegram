use num_bigint::BigUint;

use super::internals::data_center::Dc;
use super::internals::{MsgId, now};
use crate::connection::Connection;
use crate::crypto::rsa::PublicKey;
use crate::crypto::{aes, prime, random, random_fill, sha1, to_be_padded};
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, RemoteCall, Serializable, base, functions, types};

pub(crate) struct Created {
    pub auth_key: [u8; 256],
    pub salt: i64,
    pub time_offset: i64,
}

fn check(ok: bool, what: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(Error::Security(what)) }
}

async fn invoke_plain<R: RemoteCall>(conn: &mut Connection, msg_ids: &mut MsgId, request: &R) -> Result<R::Return> {
    let body = request.to_bytes();
    let mut packet = Vec::with_capacity(20 + body.len());
    packet.extend_from_slice(&[0; 8]);
    packet.extend_from_slice(&msg_ids.next().to_le_bytes());
    packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
    packet.extend_from_slice(&body);
    conn.send(&packet).await?;

    let answer = conn.recv().await?;
    check(answer.len() >= 20 && answer[..8] == [0; 8], "unencrypted answer has a bad header")?;
    let len = u32::from_le_bytes(answer[16..20].try_into().unwrap()) as usize;
    let body =
        answer.get(20..len.saturating_add(20)).ok_or(Error::Security("unencrypted answer is shorter than it says"))?;
    Ok(R::Return::from_bytes(body)?)
}

fn be_trimmed(p: u64) -> Vec<u8> {
    let b = p.to_be_bytes();
    b[b.iter().position(|&x| x != 0).unwrap_or(7)..].to_vec()
}

fn tmp_aes(new_nonce: &[u8; 32], server_nonce: &[u8; 16]) -> ([u8; 32], [u8; 32]) {
    let ns = sha1(&[new_nonce, server_nonce]);
    let sn = sha1(&[server_nonce, new_nonce]);
    let nn = sha1(&[new_nonce, new_nonce]);
    let mut key = [0; 32];
    key[..20].copy_from_slice(&ns);
    key[20..].copy_from_slice(&sn[..12]);
    let mut iv = [0; 32];
    iv[..8].copy_from_slice(&sn[12..]);
    iv[8..28].copy_from_slice(&nn);
    iv[28..].copy_from_slice(&new_nonce[..4]);
    (key, iv)
}

fn check_dh_value(x: &BigUint, dh_prime: &BigUint, what: &'static str) -> Result<()> {
    let margin = BigUint::from(1u8) << (2048 - 64);
    check(*x > margin && *x < dh_prime - &margin, what)
}

pub(crate) async fn create(conn: &mut Connection, dc: Dc) -> Result<Created> {
    let key = if dc.test { PublicKey::test() } else { PublicKey::production() };
    let mut msg_ids = MsgId::new(0);

    let nonce: [u8; 16] = random();
    let base::ResPQ::ResPQ(res_pq) = invoke_plain(conn, &mut msg_ids, &functions::ReqPqMulti { nonce }).await?;
    check(res_pq.nonce == nonce, "resPQ nonce mismatch")?;
    check(res_pq.server_public_key_fingerprints.contains(&key.fingerprint()), "server doesn't offer our RSA key")?;
    check(res_pq.pq.len() <= 8, "pq is longer than 64 bits")?;
    let pq = res_pq.pq.iter().fold(0u64, |acc, &b| (acc << 8) | b as u64);
    let (p, q) = prime::factorize(pq).ok_or(Error::Security("pq doesn't factor"))?;

    let server_nonce = res_pq.server_nonce;
    let new_nonce: [u8; 32] = random();
    let inner = types::PQInnerDataDc {
        pq: res_pq.pq.clone(),
        p: be_trimmed(p),
        q: be_trimmed(q),
        nonce,
        server_nonce,
        new_nonce,
        dc: dc.wire_id(),
    };
    let request = functions::ReqDHParams {
        nonce,
        server_nonce,
        p: be_trimmed(p),
        q: be_trimmed(q),
        public_key_fingerprint: key.fingerprint(),
        encrypted_data: key.encrypt_pad(&inner.to_bytes()).to_vec(),
    };
    let params = match invoke_plain(conn, &mut msg_ids, &request).await? {
        base::ServerDHParams::ServerDHParamsOk(ok) => ok,
        base::ServerDHParams::ServerDHParamsFail(_) => return Err(Error::Security("server_DH_params_fail")),
    };
    check(params.nonce == nonce && params.server_nonce == server_nonce, "server_DH_params nonce mismatch")?;

    let (tmp_key, tmp_iv) = tmp_aes(&new_nonce, &server_nonce);
    let mut answer = params.encrypted_answer;
    check(answer.len() > 20, "encrypted_answer is too short")?;
    aes::ige256_decrypt(&mut answer, &tmp_key, &tmp_iv)
        .map_err(|_| Error::Security("encrypted_answer isn't block aligned"))?;
    let mut rest = &answer[20..];
    let base::ServerDHInnerData::ServerDHInnerData(dh) = base::ServerDHInnerData::deserialize(&mut rest)?;
    let inner_len = answer.len() - 20 - rest.len();
    check(answer[..20] == sha1(&[&answer[20..20 + inner_len]]), "server_DH_inner_data hash mismatch")?;
    check(rest.len() < 16, "too much padding after server_DH_inner_data")?;
    check(dh.nonce == nonce && dh.server_nonce == server_nonce, "server_DH_inner_data nonce mismatch")?;

    let dh_prime = BigUint::from_bytes_be(&dh.dh_prime);
    check(dh_prime == prime::dh_prime(), "server sent an unknown dh_prime")?;
    check((2..=7).contains(&dh.g), "g is out of range")?;
    let g = BigUint::from(dh.g as u32);
    let g_a = BigUint::from_bytes_be(&dh.g_a);
    check_dh_value(&g_a, &dh_prime, "g_a is out of the safe range")?;
    let time_offset = dh.server_time as i64 - now();

    let mut retry_id = 0;
    for _ in 0..5 {
        let b = BigUint::from_bytes_be(&random::<256>());
        let g_b = g.modpow(&b, &dh_prime);
        check_dh_value(&g_b, &dh_prime, "g_b is out of the safe range")?;
        let auth_key: [u8; 256] = to_be_padded(&g_a.modpow(&b, &dh_prime));

        let client_inner =
            types::ClientDHInnerData { nonce, server_nonce, retry_id, g_b: g_b.to_bytes_be() }.to_bytes();
        let mut data = sha1(&[&client_inner]).to_vec();
        data.extend_from_slice(&client_inner);
        let padding = (16 - data.len() % 16) % 16;
        let start = data.len();
        data.resize(start + padding, 0);
        random_fill(&mut data[start..]);
        aes::ige256_encrypt(&mut data, &tmp_key, &tmp_iv).unwrap();

        let request = functions::SetClientDHParams { nonce, server_nonce, encrypted_data: data };
        let answer = invoke_plain(conn, &mut msg_ids, &request).await?;

        let aux_hash: [u8; 8] = sha1(&[&auth_key])[..8].try_into().unwrap();
        let nonce_hash = |n: u8| -> [u8; 16] { sha1(&[&new_nonce, &[n], &aux_hash])[4..].try_into().unwrap() };
        match answer {
            base::SetClientDHParamsAnswer::DhGenOk(a) => {
                check(a.nonce == nonce && a.server_nonce == server_nonce, "dh_gen_ok nonce mismatch")?;
                check(a.new_nonce_hash1 == nonce_hash(1), "dh_gen_ok new_nonce_hash1 mismatch")?;
                let mut salt = [0; 8];
                for i in 0..8 {
                    salt[i] = new_nonce[i] ^ server_nonce[i];
                }
                return Ok(Created { auth_key, salt: i64::from_le_bytes(salt), time_offset });
            }
            base::SetClientDHParamsAnswer::DhGenRetry(a) => {
                check(a.nonce == nonce && a.server_nonce == server_nonce, "dh_gen_retry nonce mismatch")?;
                check(a.new_nonce_hash2 == nonce_hash(2), "dh_gen_retry new_nonce_hash2 mismatch")?;
                retry_id = i64::from_le_bytes(aux_hash);
            }
            base::SetClientDHParamsAnswer::DhGenFail(a) => {
                check(a.nonce == nonce && a.server_nonce == server_nonce, "dh_gen_fail nonce mismatch")?;
                check(a.new_nonce_hash3 == nonce_hash(3), "dh_gen_fail new_nonce_hash3 mismatch")?;
                return Err(Error::Security("server rejected the auth key (dh_gen_fail)"));
            }
        }
    }
    Err(Error::Security("server asked to retry the key exchange too many times"))
}
