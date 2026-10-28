//! A loopback MTProto DC for benchmarks and offline tests.
//!
//! It skips the key exchange: the client pins Telegram's RSA keys and nobody has their private
//! halves, so the session string carries a fixed auth key instead. Everything after that is the
//! real wire: transport framing, MTProto 2.0 encryption, containers and acks.

#[path = "seal.rs"]
mod seal;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::raw::core::msg_container;
use vaegram::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
use vaegram::storage::{SessionData, session_string};

pub const AUTH_KEY: [u8; 256] = [7; 256];
pub const SELF_ID: i64 = 1000;
pub const SENDER_ID: i64 = 2000;
/// A basic group: the client calls it `-GROUP_ID`.
pub const GROUP_ID: i64 = 3000;
const MAX_PART: usize = 1 << 20;

const RPC_RESULT: u32 = 0xf35c6d01;
const MSGS_ACK: u32 = <types::MsgsAck as Identifiable>::CONSTRUCTOR_ID;
const PING: u32 = <functions::Ping as Identifiable>::CONSTRUCTOR_ID;
const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_LAYER: u32 = <functions::InvokeWithLayer<()> as Identifiable>::CONSTRUCTOR_ID;
const INIT_CONNECTION: u32 = <functions::InitConnection<()> as Identifiable>::CONSTRUCTOR_ID;
const GET_CONFIG: u32 = <functions::help::GetConfig as Identifiable>::CONSTRUCTOR_ID;
const GET_USERS: u32 = <functions::users::GetUsers as Identifiable>::CONSTRUCTOR_ID;
const GET_STATE: u32 = <functions::updates::GetState as Identifiable>::CONSTRUCTOR_ID;
const GET_DIFFERENCE: u32 = <functions::updates::GetDifference as Identifiable>::CONSTRUCTOR_ID;
const SEND_MESSAGE: u32 = <functions::messages::SendMessage as Identifiable>::CONSTRUCTOR_ID;
const SAVE_FILE_PART: u32 = <functions::upload::SaveFilePart as Identifiable>::CONSTRUCTOR_ID;
const SAVE_BIG_FILE_PART: u32 = <functions::upload::SaveBigFilePart as Identifiable>::CONSTRUCTOR_ID;
const GET_FILE: u32 = <functions::upload::GetFile as Identifiable>::CONSTRUCTOR_ID;

const CONTAINER_MAX: usize = 100;

#[derive(Default)]
pub struct Stats {
    pub sent: AtomicU64,
    pub parts_saved: AtomicU64,
    pub states: AtomicU64,
}

pub struct MockDc {
    pub addr: SocketAddr,
    pub stats: Arc<Stats>,
    pushes: Arc<Mutex<Option<mpsc::UnboundedSender<u32>>>>,
}

impl MockDc {
    /// Every answer leaves `latency` after its request arrived.
    pub fn start(latency: Duration) -> MockDc {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(Stats::default());
        let pushes = Arc::new(Mutex::new(None));
        let (s, p) = (stats.clone(), pushes.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                let listener = TcpListener::from_std(listener).unwrap();
                while let Ok((stream, _)) = listener.accept().await {
                    stream.set_nodelay(true).unwrap();
                    let (tx, rx) = mpsc::unbounded_channel();
                    *p.lock().unwrap() = Some(tx);
                    tokio::spawn(serve(stream, addr, latency, s.clone(), rx));
                }
            });
        });
        MockDc { addr, stats, pushes }
    }

    pub fn session_string(&self) -> String {
        session_string::export(&SessionData {
            dc_id: 2,
            test_mode: false,
            addr: Some(self.addr),
            api_id: 1,
            auth_key: AUTH_KEY,
            user_id: SELF_ID,
            is_bot: true,
        })
    }

    pub fn push(&self, n: u32) {
        if let Some(tx) = &*self.pushes.lock().unwrap() {
            let _ = tx.send(n);
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().unwrap()))
}

fn le_i64(b: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_le_bytes(b.get(at..at + 8)?.try_into().unwrap()))
}

/// The query inside `initConnection`, or `None` if it carries a proxy or params we don't parse.
fn init_connection_query(b: &[u8]) -> Option<&[u8]> {
    if le_u32(b, 4)? & 3 != 0 {
        return None;
    }
    let mut rest = b.get(12..)?;
    for _ in 0..6 {
        Vec::<u8>::deserialize(&mut rest).ok()?;
    }
    Some(rest)
}

pub fn get_file(offset: i64, limit: i32) -> functions::upload::GetFile {
    let location = types::InputDocumentFileLocation {
        id: 1,
        access_hash: 1,
        file_reference: Vec::new(),
        thumb_size: String::new(),
    };
    functions::upload::GetFile {
        precise: false,
        cdn_supported: false,
        location: base::InputFileLocation::InputDocumentFileLocation(Box::new(location)),
        offset,
        limit,
    }
}

pub fn user(id: i64, is_self: bool) -> base::User {
    base::User::User(Box::new(types::User {
        is_self,
        contact: false,
        mutual_contact: false,
        deleted: false,
        bot: is_self,
        bot_chat_history: false,
        bot_nochats: false,
        verified: false,
        restricted: false,
        min: false,
        bot_inline_geo: false,
        support: false,
        scam: false,
        apply_min_photo: false,
        fake: false,
        bot_attach_menu: false,
        premium: false,
        attach_menu_enabled: false,
        bot_can_edit: false,
        close_friend: false,
        stories_hidden: false,
        stories_unavailable: false,
        contact_require_premium: false,
        bot_business: false,
        bot_has_main_app: false,
        bot_forum_view: false,
        bot_forum_can_manage_topics: false,
        bot_can_manage_bots: false,
        bot_guestchat: false,
        bot_guard: false,
        id,
        access_hash: Some(id * 7),
        first_name: Some(if is_self { "Bench Bot" } else { "Sender" }.into()),
        last_name: None,
        username: Some(format!("user{id}")),
        phone: None,
        photo: None,
        status: None,
        bot_info_version: is_self.then_some(1),
        restriction_reason: None,
        bot_inline_placeholder: None,
        lang_code: None,
        emoji_status: None,
        usernames: None,
        stories_max_id: None,
        color: None,
        profile_color: None,
        bot_active_users: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
        linked_community_id: None,
    }))
}

pub fn group() -> base::Chat {
    base::Chat::Chat(Box::new(types::Chat {
        creator: false,
        left: false,
        deactivated: false,
        call_active: false,
        call_not_empty: false,
        noforwards: false,
        id: GROUP_ID,
        title: "Bench".into(),
        photo: base::ChatPhoto::ChatPhotoEmpty,
        participants_count: 2,
        date: 0,
        version: 1,
        migrated_to: None,
        admin_rights: None,
        default_banned_rights: None,
    }))
}

pub fn message(id: i32, date: i32, text: &str) -> base::Message {
    base::Message::Message(Box::new(types::Message {
        out: false,
        mentioned: false,
        media_unread: false,
        silent: false,
        post: false,
        from_scheduled: false,
        legacy: false,
        edit_hide: false,
        pinned: false,
        noforwards: false,
        invert_media: false,
        offline: false,
        video_processing_pending: false,
        paid_suggested_post_stars: false,
        paid_suggested_post_ton: false,
        id,
        from_id: Some(base::Peer::PeerUser(types::PeerUser { user_id: SENDER_ID })),
        from_boosts_applied: None,
        from_rank: None,
        peer_id: base::Peer::PeerChat(types::PeerChat { chat_id: GROUP_ID }),
        saved_peer_id: None,
        fwd_from: None,
        via_bot_id: None,
        via_business_bot_id: None,
        guestchat_via_from: None,
        reply_to: None,
        date,
        message: text.into(),
        media: None,
        reply_markup: None,
        entities: None,
        views: None,
        forwards: None,
        replies: None,
        edit_date: None,
        post_author: None,
        grouped_id: None,
        reactions: None,
        restriction_reason: None,
        ttl_period: None,
        quick_reply_shortcut_id: None,
        effect: None,
        factcheck: None,
        report_delivery_until_date: None,
        paid_message_stars: None,
        suggested_post: None,
        schedule_repeat_period: None,
        summary_from_language: None,
        rich_message: None,
    }))
}

pub fn config(addr: SocketAddr, date: i32) -> base::Config {
    let dc = |id| {
        base::DcOption::DcOption(Box::new(types::DcOption {
            ipv6: false,
            media_only: false,
            tcpo_only: false,
            cdn: false,
            r#static: false,
            this_port_only: false,
            id,
            ip_address: addr.ip().to_string(),
            port: addr.port() as i32,
            secret: None,
        }))
    };
    base::Config::Config(Box::new(types::Config {
        default_p2p_contacts: false,
        preload_featured_stickers: false,
        revoke_pm_inbox: false,
        blocked_mode: false,
        force_try_ipv6: false,
        date,
        expires: date + 3600,
        test_mode: false,
        this_dc: 2,
        dc_options: (1..=5).map(dc).collect(),
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

struct Conn {
    addr: SocketAddr,
    stats: Arc<Stats>,
    auth_key_id: [u8; 8],
    session_id: [u8; 8],
    last_msg_id: i64,
    content: i32,
    pts: i32,
    message_id: i32,
    /// Messages for the next packet: (msg_id, seq_no, body).
    out: Vec<(i64, i32, Vec<u8>)>,
}

impl Conn {
    fn msg_id(&mut self) -> i64 {
        self.last_msg_id = (self.last_msg_id + 4).max((unix_now() << 32) | 1);
        self.last_msg_id
    }

    fn reply(&mut self, body: Vec<u8>) {
        let (msg_id, seq) = (self.msg_id(), self.content * 2 + 1);
        self.content += 1;
        self.out.push((msg_id, seq, body));
    }

    fn rpc_result(&mut self, req_msg_id: i64, result: &[u8]) {
        let mut body = Vec::with_capacity(12 + result.len());
        body.extend_from_slice(&RPC_RESULT.to_le_bytes());
        body.extend_from_slice(&req_msg_id.to_le_bytes());
        body.extend_from_slice(result);
        self.reply(body);
    }

    fn handle(&mut self, msg_id: i64, body: &[u8]) {
        let date = unix_now() as i32;
        let Some(id) = le_u32(body, 0) else { return };
        let result = match id {
            MSGS_ACK => return,
            PING | PING_DELAY_DISCONNECT => {
                let ping_id = le_i64(body, 4).unwrap_or(0);
                let pong = base::Pong::Pong(Box::new(types::Pong { msg_id, ping_id }));
                return self.reply(pong.to_bytes());
            }
            INVOKE_WITH_LAYER => return self.handle(msg_id, body.get(8..).unwrap_or_default()),
            INIT_CONNECTION => match init_connection_query(body) {
                Some(query) => return self.handle(msg_id, query),
                None => unsupported(id),
            },
            GET_CONFIG => config(self.addr, date).to_bytes(),
            GET_USERS => vec![user(SELF_ID, true)].to_bytes(),
            GET_STATE => {
                self.stats.states.fetch_add(1, Ordering::Relaxed);
                base::updates::State::State(Box::new(types::updates::State {
                    pts: self.pts,
                    qts: 0,
                    date,
                    seq: 0,
                    unread_count: 0,
                }))
                .to_bytes()
            }
            GET_DIFFERENCE => {
                base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date, seq: 0 }).to_bytes()
            }
            SEND_MESSAGE => {
                self.stats.sent.fetch_add(1, Ordering::Relaxed);
                self.pts += 1;
                self.message_id += 1;
                base::Updates::UpdateShortSentMessage(Box::new(types::UpdateShortSentMessage {
                    out: true,
                    id: self.message_id,
                    pts: self.pts,
                    pts_count: 1,
                    date,
                    media: None,
                    entities: None,
                    ttl_period: None,
                }))
                .to_bytes()
            }
            SAVE_FILE_PART | SAVE_BIG_FILE_PART => {
                self.stats.parts_saved.fetch_add(1, Ordering::Relaxed);
                true.to_bytes()
            }
            // offset:long limit:int close the request, after a location of any shape.
            GET_FILE => {
                let limit = body.len().checked_sub(4).and_then(|at| le_u32(body, at)).unwrap_or(0) as usize;
                base::upload::File::File(Box::new(types::upload::File {
                    r#type: base::storage::FileType::FilePartial,
                    mtime: 0,
                    bytes: (0..limit.min(MAX_PART)).map(|i| i as u8).collect(),
                }))
                .to_bytes()
            }
            _ => unsupported(id),
        };
        self.rpc_result(msg_id, &result);
    }

    fn push(&mut self, n: u32) {
        let date = unix_now() as i32;
        for _ in 0..n {
            self.pts += 1;
            self.message_id += 1;
            let update = types::UpdateNewMessage {
                message: message(self.message_id, date, "ping"),
                pts: self.pts,
                pts_count: 1,
            };
            let updates = base::Updates::Updates(Box::new(types::Updates {
                updates: vec![base::Update::UpdateNewMessage(Box::new(update))],
                users: vec![user(SENDER_ID, false)],
                chats: vec![group()],
                date,
                seq: 0,
            }));
            self.reply(updates.to_bytes());
        }
    }

    fn flush(&mut self, packets: &mut Vec<Vec<u8>>) {
        let out = std::mem::take(&mut self.out);
        for chunk in out.chunks(CONTAINER_MAX) {
            let packet = if let [(msg_id, seq, body)] = chunk {
                seal::seal(&AUTH_KEY, &self.auth_key_id, &self.session_id, *msg_id, *seq, body)
            } else {
                let mut body = Vec::with_capacity(8 + chunk.iter().map(|m| 16 + m.2.len()).sum::<usize>());
                body.extend_from_slice(&msg_container::ID.to_le_bytes());
                body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
                for (msg_id, seq, b) in chunk {
                    body.extend_from_slice(&msg_id.to_le_bytes());
                    body.extend_from_slice(&seq.to_le_bytes());
                    body.extend_from_slice(&(b.len() as u32).to_le_bytes());
                    body.extend_from_slice(b);
                }
                let msg_id = self.msg_id();
                seal::seal(&AUTH_KEY, &self.auth_key_id, &self.session_id, msg_id, self.content * 2, &body)
            };
            packets.push(packet);
        }
    }

    /// Decrypts a client packet and returns (msg_id, body), learning the session id from it.
    fn open(&mut self, packet: &mut [u8]) -> Option<(i64, std::ops::Range<usize>)> {
        if packet.len() < 24 + 32 || (packet.len() - 24) % 16 != 0 || packet[..8] != self.auth_key_id {
            return None;
        }
        let msg_key: [u8; 16] = packet[8..24].try_into().unwrap();
        let (key, iv) = warpcrypto::kdf(&AUTH_KEY, &msg_key, true);
        warpcrypto::ige256_decrypt(&mut packet[24..], &key, &iv).ok()?;
        let check = Sha256::new().chain_update(&AUTH_KEY[88..120]).chain_update(&packet[24..]).finalize();
        if check[8..24] != msg_key {
            return None;
        }
        self.session_id = packet[32..40].try_into().unwrap();
        let len = le_u32(packet, 52)? as usize;
        (56 + len <= packet.len()).then(|| (le_i64(packet, 40).unwrap(), 56..56 + len))
    }

    fn handle_packet(&mut self, packet: &mut [u8]) {
        let Some((msg_id, body)) = self.open(packet) else { return };
        let body = &packet[body];
        if le_u32(body, 0) == Some(msg_container::ID) {
            for m in msg_container::parse(body).unwrap_or_default() {
                self.handle(m.msg_id, m.body);
            }
        } else {
            self.handle(msg_id, body);
        }
    }
}

fn unsupported(id: u32) -> Vec<u8> {
    let message = format!("BENCH_UNSUPPORTED_{id:08x}").into_bytes();
    base::RpcError::RpcError(Box::new(types::RpcError { error_code: 400, error_message: message })).to_bytes()
}

async fn serve(
    stream: TcpStream,
    addr: SocketAddr,
    latency: Duration,
    stats: Arc<Stats>,
    mut pushes: mpsc::UnboundedReceiver<u32>,
) {
    let (mut reader, writer) = stream.into_split();
    // The client's default framing; the benches don't use the others.
    let mut tag = [0u8; 4];
    if reader.read_exact(&mut tag).await.is_err() || tag != [0xee; 4] {
        return;
    }
    let (packets_tx, mut packets) = mpsc::channel::<Vec<u8>>(64);
    tokio::spawn(async move {
        while let Ok(len) = reader.read_u32_le().await {
            let mut p = vec![0; len as usize];
            if reader.read_exact(&mut p).await.is_err() || packets_tx.send(p).await.is_err() {
                break;
            }
        }
    });
    let (out_tx, out_rx) = mpsc::unbounded_channel();
    tokio::spawn(write_packets(writer, out_rx));

    let auth_key_id = Sha1::digest(AUTH_KEY)[12..].try_into().unwrap();
    let mut conn = Conn {
        addr,
        stats,
        auth_key_id,
        session_id: [0; 8],
        last_msg_id: 0,
        content: 0,
        pts: 1,
        message_id: 0,
        out: Vec::new(),
    };
    let mut sealed = Vec::new();
    loop {
        tokio::select! {
            packet = packets.recv() => match packet {
                Some(mut p) => conn.handle_packet(&mut p),
                None => return,
            },
            n = pushes.recv() => match n {
                Some(n) => conn.push(n),
                None => return,
            },
        }
        conn.flush(&mut sealed);
        let due = Instant::now() + latency;
        for packet in sealed.drain(..) {
            if out_tx.send((due, packet)).is_err() {
                return;
            }
        }
    }
}

/// Holds each packet until its due time, so latency adds to every round trip without
/// limiting how many are in flight.
async fn write_packets(
    writer: tokio::net::tcp::OwnedWriteHalf,
    mut packets: mpsc::UnboundedReceiver<(Instant, Vec<u8>)>,
) {
    let mut writer = BufWriter::with_capacity(1 << 20, writer);
    while let Some((due, packet)) = packets.recv().await {
        if due > Instant::now() {
            if writer.flush().await.is_err() {
                return;
            }
            tokio::time::sleep_until(due.into()).await;
        }
        let len = (packet.len() as u32).to_le_bytes();
        if writer.write_all(&len).await.is_err()
            || writer.write_all(&packet).await.is_err()
            || (packets.is_empty() && writer.flush().await.is_err())
        {
            return;
        }
    }
}
