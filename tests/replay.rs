use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use vaegram::connection::{Proxy, Transport};
use vaegram::raw::{Identifiable, Serializable, base, functions, types};
use vaegram::session::Replay;
use vaegram::storage::recording::{self, Record};
use vaegram::storage::session_string;
use vaegram::{Client, filters};

const RPC_RESULT: u32 = 0xf35c6d01;
const NEW_SESSION_CREATED: u32 = <types::NewSessionCreated as Identifiable>::CONSTRUCTOR_ID;
const SEND_MESSAGE: u32 = <functions::messages::SendMessage as Identifiable>::CONSTRUCTOR_ID;
const BOT: i64 = 1000;
const ALICE: i64 = 2000;
const DATE: i32 = 1_700_000_000;

#[tokio::test]
async fn replays_what_it_recorded() {
    let path = std::env::temp_dir().join(format!("vaegram-replay-{}.rec", std::process::id()));
    let first = play(&bot_session(), Some(&path)).await;
    assert_eq!(first, ["2000 /start -> echo: /start", "2000 ping -> echo: ping"]);

    let recorded = recording::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let count = |outgoing, id: u32| {
        recorded.iter().filter(|r| r.outgoing == outgoing && r.body.starts_with(&id.to_le_bytes())).count()
    };
    assert_eq!((count(true, SEND_MESSAGE), count(false, RPC_RESULT), count(false, NEW_SESSION_CREATED)), (2, 6, 1));
    let autologin = |body: &[u8]| body.windows(16).any(|w| w == b"autologin-secret");
    assert!(!recorded.iter().any(|r| autologin(&r.body)), "the config's autologin_token was recorded");
    assert_eq!(play(&recorded, None).await, first);
}

#[tokio::test]
async fn replaying_twice_gives_the_same_handler_output() {
    let recording = bot_session();
    for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
        let first = play_over(&recording, None, transport).await;
        assert_eq!(play_over(&recording, None, transport).await, first, "{transport:?}");
        assert_eq!(first, ["2000 /start -> echo: /start", "2000 ping -> echo: ping"], "{transport:?}");
    }
}

#[tokio::test]
async fn a_request_past_the_recording_names_itself() {
    let replay = Replay::start(&bot_session()).await.unwrap();
    let client =
        Client::builder("replay", 1, "replay").session_string(replay.session_string()).connect().await.unwrap();
    let e = client.invoke(&functions::help::GetNearestDc {}).await.unwrap_err();
    let vaegram::Error::Rpc(e) = e else { panic!("{e}") };
    assert_eq!((e.name.as_str(), e.value), ("REPLAY_EXHAUSTED", Some(functions::help::GetNearestDc::CONSTRUCTOR_ID)));
}

#[tokio::test]
async fn a_replay_client_resolves_every_dc_to_the_replay() {
    let replay = Replay::start(&bot_session()).await.unwrap();
    let replay_addr = session_string::parse(&replay.session_string()).unwrap().addr;
    let client =
        Client::builder("replay", 1, "replay").session_string(replay.session_string()).connect().await.unwrap();
    let exported = session_string::parse(&client.export_session_string()).unwrap();
    assert_eq!(exported.addr, replay_addr, "the home DC resolves to a Telegram address");
}

// A SOCKS5 proxy that logs where each connection wanted to go and refuses it.
async fn refusing_proxy() -> (u16, Arc<Mutex<Vec<[u8; 6]>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let targets = Arc::new(Mutex::new(Vec::new()));
    let log = targets.clone();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = listener.accept().await {
            let mut request = [0; 10];
            if s.read_exact(&mut request[..3]).await.is_err() || s.write_all(&[5, 0]).await.is_err() {
                continue;
            }
            if s.read_exact(&mut request).await.is_ok() {
                log.lock().unwrap().push(request[4..].try_into().unwrap());
                let _ = s.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
            }
        }
    });
    (port, targets)
}

#[tokio::test]
async fn an_unreachable_replay_is_never_swapped_for_telegram() {
    let session = Replay::start(&bot_session()).await.unwrap().session_string();
    let addr = session_string::parse(&session).unwrap().addr.unwrap();
    let SocketAddr::V4(addr) = addr else { panic!("{addr}") };
    let replay = [addr.ip().octets().as_slice(), &addr.port().to_be_bytes()].concat();
    // `.ipv6()` picks IPv6 addresses, but a replay's IPv4 loopback address still wins.
    for ipv6 in [false, true] {
        let (port, targets) = refusing_proxy().await;
        let proxy = Proxy::Socks5 { host: "127.0.0.1".into(), port, auth: None };
        let mut builder = Client::builder("replay", 1, "replay").session_string(session.clone()).proxy(proxy);
        if ipv6 {
            builder = builder.ipv6();
        }
        assert!(builder.connect().await.is_err());
        assert_eq!(*targets.lock().unwrap(), [<[u8; 6]>::try_from(replay.clone()).unwrap()], "ipv6: {ipv6}");
    }
}

#[cfg(feature = "json")]
#[test]
fn a_dump_of_the_bot_session_matches_the_golden_file() {
    // The golden file shows what a recording holds, so the config goes in without the autologin_token the
    // recorder drops.
    let base::Config::Config(mut config) = config();
    config.autologin_token = None;
    let dump: Vec<String> = bot_session_with(base::Config::Config(config)).iter().map(Record::to_json).collect();
    assert_eq!(dump, include_str!("data/bot_session.jsonl").lines().collect::<Vec<_>>());
}

async fn play(records: &[Record], record: Option<&Path>) -> Vec<String> {
    play_over(records, record, Transport::default()).await
}

async fn play_over(records: &[Record], record: Option<&Path>, transport: Transport) -> Vec<String> {
    let replay = Replay::start(records).await.unwrap();
    let mut builder =
        Client::builder("replay", 1, "replay").session_string(replay.session_string()).transport(transport);
    if let Some(path) = record {
        builder = builder.record(path);
    }
    let client = builder.connect().await.unwrap();
    let (tx, mut seen) = mpsc::unbounded_channel();
    client.on_message(filters::private(), move |m| {
        let tx = tx.clone();
        async move {
            let sent = m.reply(format!("echo: {}", m.text)).await?;
            let _ = tx.send(format!("{} {} -> {}", m.chat.id, m.text, sent.text));
            Ok(())
        }
    });
    let runner = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    let mut out = Vec::new();
    for _ in 0..2 {
        let next = tokio::time::timeout(Duration::from_secs(10), seen.recv()).await;
        out.push(next.expect("the handler didn't run from the replay").unwrap());
    }
    runner.abort();
    out
}

/// A bot's first seconds on Telegram: a new session, the startup calls, then two private messages it answers.
fn bot_session() -> Vec<Record> {
    bot_session_with(config())
}

fn bot_session_with(config: base::Config) -> Vec<Record> {
    let mut r = Vec::new();
    let new_session = types::NewSessionCreated { first_msg_id: 4, unique_id: 1, server_salt: 1 };
    unasked(&mut r, base::NewSession::NewSessionCreated(Box::new(new_session)).to_bytes());
    // The replay matches requests by constructor id only; full bodies where they're short put the call decoder
    // in the golden file.
    let init = <functions::InvokeWithLayer<functions::help::GetConfig> as Identifiable>::CONSTRUCTOR_ID;
    call(&mut r, init.to_le_bytes().to_vec(), config.to_bytes());
    let get_users = functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] };
    call(&mut r, get_users.to_bytes(), vec![user(BOT, true)].to_bytes());
    let state = types::updates::State { pts: 10, qts: 0, date: DATE, seq: 0, unread_count: 0 };
    call(&mut r, functions::updates::GetState {}.to_bytes(), base::updates::State::State(Box::new(state)).to_bytes());
    let get_difference = functions::updates::GetDifference {
        pts: 10,
        pts_limit: None,
        pts_total_limit: Some(1000),
        date: DATE,
        qts: 0,
        qts_limit: None,
    };
    let empty = types::updates::DifferenceEmpty { date: DATE, seq: 0 };
    let difference = base::updates::Difference::DifferenceEmpty(empty);
    call(&mut r, get_difference.to_bytes(), difference.to_bytes());
    for (pts, text) in [(11, "/start"), (13, "ping")] {
        unasked(&mut r, new_message(pts, text).to_bytes());
        let sent = types::UpdateShortSentMessage {
            out: true,
            id: pts + 1,
            pts: pts + 1,
            pts_count: 1,
            date: DATE,
            media: None,
            entities: None,
            ttl_period: None,
        };
        call(
            &mut r,
            SEND_MESSAGE.to_le_bytes().to_vec(),
            base::Updates::UpdateShortSentMessage(Box::new(sent)).to_bytes(),
        );
    }
    r
}

fn call(records: &mut Vec<Record>, request: Vec<u8>, result: Vec<u8>) {
    let msg_id = 4 * records.len() as i64 + 4;
    records.push(Record { outgoing: true, msg_id, unix_ms: 0, body: request });
    let body = [&RPC_RESULT.to_le_bytes()[..], &msg_id.to_le_bytes()[..], &result[..]].concat();
    records.push(Record { outgoing: false, msg_id: msg_id + 1, unix_ms: 0, body });
}

fn unasked(records: &mut Vec<Record>, body: Vec<u8>) {
    records.push(Record { outgoing: false, msg_id: 4 * records.len() as i64 + 1, unix_ms: 0, body });
}

fn new_message(pts: i32, text: &str) -> base::Updates {
    let alice = || base::Peer::PeerUser(types::PeerUser { user_id: ALICE });
    let message = types::Message {
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
        id: pts,
        from_id: Some(alice()),
        from_boosts_applied: None,
        from_rank: None,
        peer_id: alice(),
        saved_peer_id: None,
        fwd_from: None,
        via_bot_id: None,
        via_business_bot_id: None,
        guestchat_via_from: None,
        reply_to: None,
        date: DATE,
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
    };
    let update = types::UpdateNewMessage { message: base::Message::Message(Box::new(message)), pts, pts_count: 1 };
    base::Updates::Updates(Box::new(types::Updates {
        updates: vec![base::Update::UpdateNewMessage(Box::new(update))],
        users: vec![user(ALICE, false)],
        chats: Vec::new(),
        date: DATE,
        seq: 0,
    }))
}

fn user(id: i64, bot: bool) -> base::User {
    base::User::User(Box::new(types::User {
        is_self: bot,
        contact: false,
        mutual_contact: false,
        deleted: false,
        bot,
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
        first_name: Some(if bot { "Echo" } else { "Alice" }.into()),
        last_name: None,
        username: Some(format!("user{id}")),
        phone: None,
        photo: None,
        status: None,
        bot_info_version: bot.then_some(1),
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

fn config() -> base::Config {
    base::Config::Config(Box::new(types::Config {
        default_p2p_contacts: false,
        preload_featured_stickers: false,
        revoke_pm_inbox: false,
        blocked_mode: false,
        force_try_ipv6: false,
        date: DATE,
        expires: DATE + 3600,
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
        autologin_token: Some("autologin-secret".into()),
    }))
}
