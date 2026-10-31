//! A data center on localhost that runs the key exchange, so tests can make permanent and temporary keys and
//! bind them. It answers the calls a client makes on connect and logs what each connection sent.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use num_bigint::BigUint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::internals::data_center::Dc;
use super::replay::{open, seal};
use super::{auth, constructor_id, internals};
use crate::connection::{RecvBuf, Transport};
use crate::crypto::rsa::PublicKey;
use crate::crypto::{aes, mtproto, prime, random, sha1, sha256, to_be_padded};
use crate::raw::core::msg_container;
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
use crate::storage::{SessionData, recording, session_string};

type Any = functions::Ping;
pub(super) const BIND: u32 = <functions::auth::BindTempAuthKey as Identifiable>::CONSTRUCTOR_ID;
pub(crate) const INVOKE_WITH_LAYER: u32 = <functions::InvokeWithLayer<Any> as Identifiable>::CONSTRUCTOR_ID;
pub(super) const GET_NEAREST_DC: u32 = <functions::help::GetNearestDc as Identifiable>::CONSTRUCTOR_ID;
pub(super) const GET_CONFIG: u32 = <functions::help::GetConfig as Identifiable>::CONSTRUCTOR_ID;
pub(crate) const EXPORT_AUTHORIZATION: u32 = <functions::auth::ExportAuthorization as Identifiable>::CONSTRUCTOR_ID;
pub(crate) const IMPORT_AUTHORIZATION: u32 = <functions::auth::ImportAuthorization as Identifiable>::CONSTRUCTOR_ID;
pub(crate) const PING: u32 = <functions::Ping as Identifiable>::CONSTRUCTOR_ID;
pub(crate) const INIT_CONNECTION: u32 = <functions::InitConnection<Any> as Identifiable>::CONSTRUCTOR_ID;
const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
const REQ_PQ_MULTI: u32 = <functions::ReqPqMulti as Identifiable>::CONSTRUCTOR_ID;
const REQ_DH_PARAMS: u32 = <functions::ReqDHParams as Identifiable>::CONSTRUCTOR_ID;
const SET_CLIENT_DH_PARAMS: u32 = <functions::SetClientDHParams as Identifiable>::CONSTRUCTOR_ID;
const RPC_RESULT: u32 = 0xf35c6d01;
const BOOL_TRUE: u32 = 0x997275b5;
const AUTHORIZATION_SIGN_UP_REQUIRED: u32 = 0x44747e9a;
// 1229739323 * 1402015859
const PQ: u64 = 0x17ED48941A08F981;

const PUBLIC_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAxUrhID+0PLIpH5bphuRWMGmZ/LzivWuNws63ZY2u7MuGCrunexQ4
RtE91R8w937hbQce/72spp7C98R6Iw5bJ/5UqyXWdkQVHwtdZ6FD7Hl8n6KGY7IH
OHCWDTe1LI7BRv8XstL0vCDYqSqYfxCkTv3JxX31g+tI/t68F4PWtD/MEoASlENV
rX83UwZSFNyLOizx1bAseMm2cBSM2/Z+ap9nBWEtzV6bHnhxrjT2wTAc2YfI7sdn
EipQwoouxy7CKSKVkR7/X4OJEFVkuwlkZ95+FLUmERk5/QXknPyTnWKq/p3DaazN
7BlxtmxghK6aIbLY5T0l0uwWTN65PqZakwIDAQAB
-----END RSA PUBLIC KEY-----";
const N_HEX: &str = concat!(
    "c54ae1203fb43cb2291f96e986e456306999fcbce2bd6b8dc2ceb7658daeeccb860abba77b143846d13dd51f30f77ee16d071effbdac",
    "a69ec2f7c47a230e5b27fe54ab25d67644151f0b5d67a143ec797c9fa28663b2073870960d37b52c8ec146ff17b2d2f4bc20d8a92a98",
    "7f10a44efdc9c57df583eb48fedebc1783d6b43fcc128012944355ad7f3753065214dc8b3a2cf1d5b02c78c9b670148cdbf67e6a9f67",
    "05612dcd5e9b1e7871ae34f6c1301cd987c8eec767122a50c28a2ec72ec2292295911eff5f8389105564bb096467de7e14b526111939",
    "fd05e49cfc939d62aafe9dc369accdec1971b66c6084ae9a21b2d8e53d25d2ec164cdeb93ea65a93",
);
const D_HEX: &str = concat!(
    "09f1f58cd2360f4acd6a483ccaf458757f7d41ab0bd7c2f6fc6a8b31bbb8a53b1ec70c875ec2202c1b4db0a02e49b85da248ef9c4bd8",
    "97c26368cf8d76fc85dfd1d43d312b7dfeb163786dfd5485bf9cae898be6965a59d86d4adcad4e9d1663856510e527f92a40f8d15e38",
    "b74bd17621e5b9800c5ca669a19296597ab441fc1cfcb5c1548b4e8c87dd418d764e90a92fef86a58434f0cfc841071a4aeef245e9ed",
    "1c202a6d46af462a2999709663b9f194c365aad33d6758668dd937bbff3cfceae8751a6ceb8ed5bf25940f374312d53b230618062c2f",
    "f2ff8deb7b985a7d8ff7af9efa4aa82625f5f686bdfe258f96f38dd890c36381504a56949eb11441",
);

/// The key every loopback data center is trusted with in tests.
pub(super) fn public_key() -> PublicKey {
    PublicKey::from_pem(PUBLIC_PEM).unwrap()
}

/// One encrypted call: the connection (from 1), whether it came under a temp key, and its constructor ids from
/// the outer wrapper in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Seen {
    pub(crate) connection: usize,
    pub(crate) temp: bool,
    pub(crate) ids: Vec<u32>,
}

#[derive(Default)]
struct Shared {
    keys: HashMap<[u8; 8], ([u8; 256], bool)>,
    seen: Vec<Seen>,
    connections: usize,
    // Answer each connection's first bind with ENCRYPTED_MESSAGE_INVALID, as a server the key hasn't reached yet.
    refuse_first_bind: bool,
}

pub(crate) struct TestDc {
    addr: SocketAddr,
    shared: Arc<Mutex<Shared>>,
    server: JoinHandle<()>,
}

impl TestDc {
    /// Knows `perm_key` already, as if it had been made before; others come from key exchanges.
    pub(crate) async fn start(perm_key: [u8; 256], refuse_first_bind: bool) -> TestDc {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut shared = Shared { refuse_first_bind, ..Shared::default() };
        shared.keys.insert(mtproto::auth_key_id(&perm_key), (perm_key, false));
        let shared = Arc::new(Mutex::new(shared));
        let state = shared.clone();
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, state.clone()));
            }
        });
        TestDc { addr, shared, server }
    }

    pub(crate) fn dc(&self, id: i32) -> Dc {
        Dc { id, addr: self.addr, test: false }
    }

    pub(crate) fn session_string(&self, auth_key: [u8; 256]) -> String {
        let addr = Some(self.addr);
        session_string::export(&SessionData {
            dc_id: 2,
            test_mode: false,
            addr,
            api_id: 1,
            auth_key,
            user_id: 0,
            is_bot: false,
        })
    }

    /// A client on this data center, with `auth_key` as its login.
    pub(crate) async fn client(&self, auth_key: [u8; 256]) -> crate::Client {
        let builder = crate::Client::builder("test-dc", 1, "test-dc").session_string(self.session_string(auth_key));
        builder.connect().await.unwrap()
    }

    pub(crate) fn seen(&self) -> Vec<Seen> {
        self.shared.lock().unwrap().seen.clone()
    }
}

impl Drop for TestDc {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[derive(Default)]
struct Exchange {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
    a: BigUint,
    temp: bool,
}

async fn serve(mut stream: TcpStream, shared: Arc<Mutex<Shared>>) -> Option<()> {
    let connection = {
        let mut s = shared.lock().unwrap();
        s.connections += 1;
        s.connections
    };
    let mut tag = [0; 4];
    stream.read_exact(&mut tag).await.ok()?;
    let mut buf = RecvBuf::default();
    let (mut exchange, mut last_msg_id, mut content, mut binds) = (Exchange::default(), 0i64, 0, 0);
    let mut msg_id = || {
        last_msg_id = (last_msg_id + 4).max((internals::now() << 32) | 1);
        last_msg_id
    };
    loop {
        let mut packet = loop {
            if let Some(p) = Transport::Intermediate.decode(&mut buf).ok()? {
                break p;
            }
            if stream.read_buf(buf.make_room()).await.ok()? == 0 {
                return None;
            }
        };
        let mut replies = Vec::new();
        if packet.get(..8)? == [0; 8] {
            let len = u32::from_le_bytes(packet.get(16..20)?.try_into().unwrap()) as usize;
            let answer = exchange.step(packet.get(20..20 + len)?, &shared)?;
            let header = [[0; 8], msg_id().to_le_bytes()].concat();
            replies.push([&header[..], &(answer.len() as u32).to_le_bytes(), &answer].concat());
        } else {
            let id: [u8; 8] = packet[..8].try_into().unwrap();
            let (key, temp) = *shared.lock().unwrap().keys.get(&id)?;
            let (session_id, outer_id, body) = open(&key, &id, &mut packet)?;
            let messages = match constructor_id(body) {
                Some(msg_container::ID) => {
                    msg_container::parse(body).ok()?.iter().map(|m| (m.msg_id, m.body.to_vec())).collect()
                }
                _ => vec![(outer_id, body.to_vec())],
            };
            for (req_msg_id, body) in messages {
                if let Some(answer) = call(connection, temp, req_msg_id, &body, &mut binds, &shared) {
                    content += 1;
                    replies.push(seal(&key, &id, &session_id, msg_id(), content * 2 - 1, &answer));
                }
            }
        }
        for packet in replies {
            let mut framed = vec![0; Transport::ROOM];
            framed.extend_from_slice(&packet);
            let start = Transport::Intermediate.frame(&mut framed);
            stream.write_all(&framed[start..]).await.ok()?;
        }
    }
}

fn call(
    connection: usize,
    temp: bool,
    req_msg_id: i64,
    body: &[u8],
    binds: &mut usize,
    shared: &Mutex<Shared>,
) -> Option<Vec<u8>> {
    let mut ids = vec![constructor_id(body)?];
    let mut query = &body[4..];
    loop {
        match *ids.last()? {
            INVOKE_WITH_LAYER => {
                query = query.get(4..)?;
            }
            INIT_CONNECTION => recording::skip_init_connection(&mut query).ok()?,
            _ => break,
        }
        ids.push(constructor_id(query)?);
        query = &query[4..];
    }
    let innermost = *ids.last()?;
    if matches!(innermost, PING_DELAY_DISCONNECT) || innermost == <types::MsgsAck as Identifiable>::CONSTRUCTOR_ID {
        return (innermost == PING_DELAY_DISCONNECT).then(|| pong(req_msg_id, query));
    }
    let mut s = shared.lock().unwrap();
    s.seen.push(Seen { connection, temp, ids });
    let result = match innermost {
        BIND => {
            *binds += 1;
            if s.refuse_first_bind && *binds == 1 {
                error(400, "ENCRYPTED_MESSAGE_INVALID")
            } else {
                BOOL_TRUE.to_le_bytes().to_vec()
            }
        }
        PING => return Some(rpc_result(req_msg_id, &pong(req_msg_id, query))),
        GET_NEAREST_DC => {
            let nearest = types::NearestDc { country: "ZZ".into(), this_dc: 2, nearest_dc: 2 };
            base::NearestDc::NearestDc(Box::new(nearest)).to_bytes()
        }
        GET_CONFIG => config().to_bytes(),
        EXPORT_AUTHORIZATION => {
            let exported = types::auth::ExportedAuthorization { id: 1, bytes: vec![1] };
            base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(exported)).to_bytes()
        }
        IMPORT_AUTHORIZATION => [AUTHORIZATION_SIGN_UP_REQUIRED, 0].iter().flat_map(|w| w.to_le_bytes()).collect(),
        _ => error(400, "NOT_IN_THE_TEST_DC"),
    };
    Some(rpc_result(req_msg_id, &result))
}

fn pong(msg_id: i64, ping: &[u8]) -> Vec<u8> {
    let ping_id = ping.get(..8).map_or(0, |b| i64::from_le_bytes(b.try_into().unwrap()));
    base::Pong::Pong(Box::new(types::Pong { msg_id, ping_id })).to_bytes()
}

fn error(code: i32, message: &str) -> Vec<u8> {
    let error = types::RpcError { error_code: code, error_message: message.into() };
    base::RpcError::RpcError(Box::new(error)).to_bytes()
}

fn rpc_result(req_msg_id: i64, result: &[u8]) -> Vec<u8> {
    [&RPC_RESULT.to_le_bytes()[..], &req_msg_id.to_le_bytes(), result].concat()
}

impl Exchange {
    fn step(&mut self, body: &[u8], shared: &Mutex<Shared>) -> Option<Vec<u8>> {
        let mut b = body.get(4..)?;
        match constructor_id(body)? {
            REQ_PQ_MULTI => {
                self.nonce = <[u8; 16]>::deserialize(&mut b).ok()?;
                self.server_nonce = random();
                let pq = PQ.to_be_bytes().to_vec();
                let fingerprints = vec![public_key().fingerprint()];
                let (nonce, server_nonce) = (self.nonce, self.server_nonce);
                let res_pq = types::ResPQ { nonce, server_nonce, pq, server_public_key_fingerprints: fingerprints };
                Some(base::ResPQ::ResPQ(Box::new(res_pq)).to_bytes())
            }
            REQ_DH_PARAMS => {
                b = b.get(32..)?;
                Vec::<u8>::deserialize(&mut b).ok()?;
                Vec::<u8>::deserialize(&mut b).ok()?;
                i64::deserialize(&mut b).ok()?;
                let inner = decrypt_pad(&Vec::<u8>::deserialize(&mut b).ok()?)?;
                (self.new_nonce, self.temp) = match base::PQInnerData::deserialize(&mut &inner[..]).ok()? {
                    base::PQInnerData::PQInnerDataDc(d) => (d.new_nonce, false),
                    base::PQInnerData::PQInnerDataTempDc(d) => (d.new_nonce, true),
                    _ => return None,
                };
                self.a = BigUint::from_bytes_be(&random::<256>());
                let dh_prime = prime::dh_prime();
                let g_a = BigUint::from(3u32).modpow(&self.a, &dh_prime);
                let (nonce, server_nonce) = (self.nonce, self.server_nonce);
                let inner = types::ServerDHInnerData {
                    nonce,
                    server_nonce,
                    g: 3,
                    dh_prime: dh_prime.to_bytes_be(),
                    g_a: g_a.to_bytes_be(),
                    server_time: internals::now() as i32,
                };
                let inner = base::ServerDHInnerData::ServerDHInnerData(Box::new(inner)).to_bytes();
                let mut answer = sha1(&[&inner]).to_vec();
                answer.extend_from_slice(&inner);
                answer.resize(answer.len().next_multiple_of(16), 0);
                let (key, iv) = auth::tmp_aes(&self.new_nonce, &self.server_nonce);
                aes::ige256_encrypt(&mut answer, &key, &iv).unwrap();
                let ok = types::ServerDHParamsOk { nonce, server_nonce, encrypted_answer: answer };
                Some(base::ServerDHParams::ServerDHParamsOk(Box::new(ok)).to_bytes())
            }
            SET_CLIENT_DH_PARAMS => {
                let mut data = Vec::<u8>::deserialize(&mut b.get(32..)?).ok()?;
                let (key, iv) = auth::tmp_aes(&self.new_nonce, &self.server_nonce);
                aes::ige256_decrypt(&mut data, &key, &iv).ok()?;
                let base::ClientDHInnerData::ClientDHInnerData(inner) =
                    base::ClientDHInnerData::deserialize(&mut data.get(20..)?).ok()?;
                let g_b = BigUint::from_bytes_be(&inner.g_b);
                let auth_key: [u8; 256] = to_be_padded(&g_b.modpow(&self.a, &prime::dh_prime()));
                shared.lock().unwrap().keys.insert(mtproto::auth_key_id(&auth_key), (auth_key, self.temp));
                let aux_hash = &sha1(&[&auth_key])[..8];
                let new_nonce_hash1 = sha1(&[&self.new_nonce, &[1], aux_hash])[4..].try_into().unwrap();
                let (nonce, server_nonce) = (self.nonce, self.server_nonce);
                let ok = types::DhGenOk { nonce, server_nonce, new_nonce_hash1 };
                Some(base::SetClientDHParamsAnswer::DhGenOk(Box::new(ok)).to_bytes())
            }
            _ => None,
        }
    }
}

// RSA_PAD backwards: https://core.telegram.org/mtproto/auth_key#presenting-proof-of-work-server-authentication
fn decrypt_pad(data: &[u8]) -> Option<Vec<u8>> {
    let n = BigUint::parse_bytes(N_HEX.as_bytes(), 16)?;
    let d = BigUint::parse_bytes(D_HEX.as_bytes(), 16)?;
    let key_aes_encrypted: [u8; 256] = to_be_padded(&BigUint::from_bytes_be(data).modpow(&d, &n));
    let (masked, aes_encrypted) = key_aes_encrypted.split_at(32);
    let hash = sha256(&[aes_encrypted]);
    let temp_key: [u8; 32] = std::array::from_fn(|i| masked[i] ^ hash[i]);
    let mut plain = aes_encrypted.to_vec();
    aes::ige256_decrypt(&mut plain, &temp_key, &[0; 32]).ok()?;
    let mut padded = plain[..192].to_vec();
    padded.reverse();
    Some(padded)
}

fn config() -> base::Config {
    base::Config::Config(Box::new(types::Config {
        default_p2p_contacts: false,
        preload_featured_stickers: false,
        revoke_pm_inbox: false,
        blocked_mode: false,
        force_try_ipv6: false,
        date: 1_700_000_000,
        expires: 1_700_003_600,
        test_mode: false,
        this_dc: 2,
        dc_options: Vec::new(),
        dc_txt_domain_name: String::new(),
        chat_size_max: 200,
        megagroup_size_max: 200_000,
        forwarded_count_max: 100,
        online_update_period_ms: 210_000,
        offline_blur_timeout_ms: 5000,
        offline_idle_timeout_ms: 30_000,
        online_cloud_timeout_ms: 300_000,
        notify_cloud_delay_ms: 30_000,
        notify_default_delay_ms: 1500,
        push_chat_period_ms: 60_000,
        push_chat_limit: 2,
        edit_time_limit: 172_800,
        revoke_time_limit: i32::MAX,
        revoke_pm_time_limit: i32::MAX,
        rating_e_decay: 2_419_200,
        stickers_recent_limit: 200,
        channels_read_media_period: 604_800,
        tmp_sessions: None,
        call_receive_timeout_ms: 20_000,
        call_ring_timeout_ms: 90_000,
        call_connect_timeout_ms: 30_000,
        call_packet_timeout_ms: 10_000,
        me_url_prefix: "https://t.me/".into(),
        autoupdate_url_prefix: None,
        gif_search_username: None,
        venue_search_username: None,
        img_search_username: None,
        static_maps_provider: None,
        caption_length_max: 1024,
        message_length_max: 4096,
        webfile_dc_id: 4,
        suggested_lang_code: None,
        lang_pack_version: None,
        base_lang_pack_version: None,
        reactions_default: None,
        autologin_token: None,
    }))
}
