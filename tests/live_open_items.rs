// Run with `cargo test --features sqlite --test live_open_items -- --ignored --test-threads=1`; see
// tests/support/mod.rs for the env. notes/live-plan.md says which open item each test settles.
// These tests share session files, so run with `--test-threads=1`.

#[allow(dead_code)]
mod support;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::connection::Proxy;
use vaegram::enums::{ChatMemberStatus, ClientEvent};
use vaegram::types::{InputRichMessage, RichBlock, RichBlockKind, Upload};
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

async fn bot_id_for_user(kit: &support::Kit) -> i64 {
    let me = kit.bot.get_me().await.unwrap();
    let username = me.username.expect("the kit bot has no username");
    kit.user.client.resolve_username(&username).await.unwrap()
}

/// The `VAEGRAM_USER_SESSION` account on its own, or `None` (after saying so) when the env isn't set.
async fn user_client() -> Option<(Client, support::Disconnect)> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let builder = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let client = builder.catch_up(false).connect().await.unwrap();
    Some((client.clone(), support::Disconnect(vec![client])))
}

/// A second logged-in account, or `None` (after saying so) when `VAEGRAM_SECOND_USER_SESSION` isn't set.
async fn second_user() -> Option<(Client, support::Disconnect)> {
    let Ok(session) = std::env::var("VAEGRAM_SECOND_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_SECOND_USER_SESSION (a second logged-in account's session name)");
        return None;
    };
    let builder = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let client = builder.catch_up(false).connect().await.unwrap();
    Some((client.clone(), support::Disconnect(vec![client])))
}

fn supergroup_chat_id() -> Option<i64> {
    match std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse::<i64>().ok()) {
        Some(id) if id <= -1_000_000_000_000 => Some(id),
        _ => {
            eprintln!("skipped: set VAEGRAM_CHAT_ID to a supergroup with the kit bot and user");
            None
        }
    }
}

/// Setup for the pin checks of item 67 (here and in `live_0_2_updates`): the user, owner of `VAEGRAM_CHAT_ID`, makes
/// the kit bot an admin there that may pin messages, and nothing more. An admin that may already pin is left as it
/// is; one that may not keeps its other rights. `notes/kaggle/live.sh` runs this before every other test.
#[tokio::test]
#[ignore = "promotes the kit bot in VAEGRAM_CHAT_ID with the owner's user session"]
async fn a_setup_kit_bot_can_pin() {
    let Some(chat_id) = supergroup_chat_id() else { return };
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let bot_id = bot_id_for_user(&kit).await;
    let member = user.get_chat_member(chat_id, bot_id).await.unwrap();
    let admin = member.status() == ChatMemberStatus::Administrator;
    if admin && member.privileges().is_some_and(|p| p.can_pin_messages) {
        eprintln!("the kit bot may already pin in VAEGRAM_CHAT_ID");
        return;
    }

    let privileges = member.privileges().cloned().unwrap_or_default().can_pin_messages(true);
    user.promote_chat_member(chat_id, bot_id, privileges).await.unwrap();
    let after = user.get_chat_member(chat_id, bot_id).await.unwrap();
    assert_eq!(after.status(), ChatMemberStatus::Administrator);
    assert!(after.privileges().is_some_and(|p| p.can_pin_messages), "the kit bot still may not pin: {after:?}");
}

/// Item 65: rich-message HTML with a newline inside a paragraph next to a `<tg-emoji>`, sent by the bot. Prints how
/// the newline came back; fails when the two lines were run together.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_rich_html_keeps_a_newline() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;
    let icons = kit.bot.get_forum_topic_icon_stickers().await.unwrap();
    let emoji_id = icons.first().expect("get_forum_topic_icon_stickers returned no stickers").id;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    })
    .group(1);

    let html = format!("<p>first line\nsecond line <tg-emoji emoji-id=\"{emoji_id}\">⭐</tg-emoji></p>");
    let _sent = kit.bot.send_rich_message(user_id, InputRichMessage::html(html)).await.unwrap();
    let got = tokio::time::timeout(WAIT, rx.recv()).await.expect("timed out waiting for the rich message");
    let got = got.expect("message channel closed");
    let fetched = user.get_rich_message(bot_id, got.id).await;
    support::cleanup(user.delete_messages(bot_id, &[got.id]).await, "deleting the rich newline message");

    let rich = fetched.unwrap().and_then(|m| m.rich_message).expect("the message has no rich content");
    let blocks = rich.blocks();
    let paragraphs: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b.kind() {
            RichBlockKind::Paragraph(t) => Some(t.text()),
            _ => None,
        })
        .collect();
    eprintln!("the HTML newline came back as {} block(s), paragraphs {paragraphs:?}", blocks.len());
    assert!(
        paragraphs.join("\n").contains("first line\nsecond line"),
        "the newline between the two lines was lost: {paragraphs:?}"
    );
}

/// Item 65: a widget sent by the bot. Telegram refused every widget with `INPUT_MEDIA_WIDGET_NOT_ALLOWED` from
/// 2026-11-13; that answer is reported as a skip. Anything sent must reach the user as the same widget.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_sends_a_widget() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        if m.widget().is_some() {
            let _ = tx.send(m);
        }
        async { Ok(()) }
    })
    .group(1);

    let html = "<!doctype html><html><body><b>vaegram widget</b></body></html>";
    let sent = match kit.bot.send_widget(user_id, html).height(120).await {
        Ok(sent) => sent,
        Err(e) if e.is("INPUT_MEDIA_WIDGET_NOT_ALLOWED") => {
            eprintln!("skipped: Telegram still refuses the bot's widget (INPUT_MEDIA_WIDGET_NOT_ALLOWED)");
            return;
        }
        Err(e) => panic!("send_widget failed: {e:?}"),
    };
    assert_eq!(sent.widget().map(|w| w.html.as_str()), Some(html));
    let got = tokio::time::timeout(WAIT, rx.recv()).await.expect("the widget never reached the user");
    let got = got.expect("message channel closed");
    support::cleanup(user.delete_messages(bot_id, &[got.id]).await, "deleting the widget message");
    assert_eq!(got.widget().map(|w| w.html.as_str()), Some(html));
}

/// Version 3 of the session database the Python MTProto libraries keep: what their older `.session` files hold.
#[cfg(feature = "sqlite")]
const SCHEMA_V3: &str = "
CREATE TABLE sessions
(
    dc_id     INTEGER PRIMARY KEY,
    api_id    INTEGER,
    test_mode INTEGER,
    auth_key  BLOB,
    date      INTEGER NOT NULL,
    user_id   INTEGER,
    is_bot    INTEGER
);
CREATE TABLE peers
(
    id             INTEGER PRIMARY KEY,
    access_hash    INTEGER,
    type           INTEGER NOT NULL,
    username       TEXT,
    phone_number   TEXT,
    last_update_on INTEGER NOT NULL DEFAULT (CAST(STRFTIME('%s', 'now') AS INTEGER))
);
CREATE TABLE version
(
    number INTEGER PRIMARY KEY
);
CREATE INDEX idx_peers_id ON peers (id);
CREATE INDEX idx_peers_username ON peers (username);
CREATE INDEX idx_peers_phone_number ON peers (phone_number);
CREATE TRIGGER trg_peers_last_update_on
    AFTER UPDATE
    ON peers
BEGIN
    UPDATE peers
    SET last_update_on = CAST(STRFTIME('%s', 'now') AS INTEGER)
    WHERE id = NEW.id;
END;
INSERT INTO version VALUES (3);
";

/// Item 66: a SQLite session file of the Python libraries, schema version 3, logs in through `SqliteStorage`, which
/// migrates it to version 8 in place. Uses a copy of `VAEGRAM_SQLITE_SESSION` (such a file made by one of them) when
/// set; otherwise writes a version 3 file by hand from the `VAEGRAM_USER_SESSION` login.
#[cfg(feature = "sqlite")]
#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn a_schema_3_sqlite_session_logs_in() {
    use vaegram::storage::{FileStorage, SqliteStorage, Storage};

    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let path = std::path::PathBuf::from("target/vaegram-sqlite-import.session");
    let remove = || {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    };
    remove();
    match std::env::var("VAEGRAM_SQLITE_SESSION") {
        Ok(real) => {
            std::fs::copy(real, &path).expect("copying VAEGRAM_SQLITE_SESSION failed");
        }
        Err(_) => {
            let file = FileStorage::new(&session);
            let login = file.load_session().await.unwrap().expect("VAEGRAM_USER_SESSION holds no login");
            file.close().await.unwrap();
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute_batch(SCHEMA_V3).unwrap();
            let (dc_id, stored_api_id, test_mode, user_id, is_bot) =
                (login.dc_id, login.api_id, login.test_mode, login.user_id, login.is_bot);
            let key = &login.auth_key[..];
            let insert = "INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?, ?)";
            db.execute(insert, rusqlite::params![dc_id, stored_api_id, test_mode, key, 0, user_id, is_bot]).unwrap();
        }
    }
    let read = |sql: &str| -> i64 {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.query_row(sql, [], |r| r.get(0)).unwrap()
    };
    let (owner, from) = (read("SELECT user_id FROM sessions"), read("SELECT number FROM version"));

    let builder = Client::builder("sqlite-import", api_id, api_hash).storage_engine(SqliteStorage::new(path.clone()));
    let user = builder.connect().await.unwrap();
    let me = user.get_me().await;
    user.close().await;
    let to = read("SELECT number FROM version");
    remove();
    eprintln!("the session file went from schema version {from} to {to}");
    assert_eq!(me.unwrap().id, owner);
    assert_eq!(to, 8);
}

/// Item 66: `ban_chat_sender_chat` bans a channel from a supergroup and `unban_chat_sender_chat` lifts the ban. The
/// user makes both chats and deletes them after.
#[tokio::test]
#[ignore = "creates and deletes a supergroup and a channel with a logged-in user session"]
async fn ban_chat_sender_chat_bans_a_channel() {
    let Some((user, _disconnect)) = user_client().await else { return };
    let Some(group) = support::test_supergroup(&user).await else { return };
    let channel = match user.create_channel("vaegram live test", "deleted right away").await {
        Ok(channel) => channel.id,
        Err(e) => {
            support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
            if support::flood_limited(&e, "creating a channel") {
                return;
            }
            panic!("create_channel failed: {e:?}");
        }
    };

    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            user.ban_chat_sender_chat(group, channel).await.unwrap();
            let banned = user.get_chat_member(group, channel).await.unwrap();
            assert_eq!(banned.status(), ChatMemberStatus::Banned);
            assert_eq!(banned.chat.map(|c| c.id), Some(channel));

            user.unban_chat_sender_chat(group, channel).await.unwrap();
            let after = user.get_chat_member(group, channel).await.unwrap();
            assert_ne!(after.status(), ChatMemberStatus::Banned, "the channel is still banned after the unban");
        }
    })
    .await;

    support::cleanup(user.delete_channel(channel).await, "deleting the channel \"vaegram live test\"");
    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// The kind of each block, for the collage and slideshow locks.
fn kinds(blocks: Vec<RichBlock>) -> Vec<&'static str> {
    let kind = |b: &RichBlock| match b.kind() {
        RichBlockKind::Collage { .. } => "collage",
        RichBlockKind::Slideshow { .. } => "slideshow",
        _ => "other",
    };
    blocks.iter().map(kind).collect()
}

/// Item 66 (the collage and slideshow locks): a rich message with a collage and a slideshow, sent by the bot, reaches
/// the user with both kinds readable. Prints whether the pushed message held them or only `get_rich_message` did: a
/// bot can't call that, so a lock bot sees only the pushed one.
#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and VAEGRAM_PHOTO"]
async fn rich_collage_and_slideshow_arrive_as_their_kinds() {
    let Ok(photo_path) = std::env::var("VAEGRAM_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_PHOTO (an image file)");
        return;
    };
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    })
    .group(1);

    let ids = Arc::new(Mutex::new(Vec::new()));
    let steps = tokio::spawn({
        let (bot, user, ids) = (kit.bot.clone(), user.clone(), ids.clone());
        async move {
            let sent_photo = bot.send_photo(user_id, photo_path.as_str()).await.unwrap();
            let photo = sent_photo.photo().expect("the sent message has no photo");
            let shown = || RichBlock::photo(photo);
            let blocks = [RichBlock::collage([shown(), shown()]), RichBlock::slideshow([shown(), shown()])];
            bot.send_rich_message(user_id, InputRichMessage::blocks(blocks)).await.unwrap();

            // The photo comes first, then the rich message.
            let pushed = loop {
                let m = tokio::time::timeout(WAIT, rx.recv()).await.expect("timed out waiting for the rich message");
                let m = m.expect("message channel closed");
                ids.lock().unwrap().push(m.id);
                if m.rich_message.is_some() {
                    break m;
                }
            };
            let pushed_rich = pushed.rich_message.as_ref().expect("checked above");
            eprintln!("pushed: partial={} kinds={:?}", pushed_rich.is_partial, kinds(pushed_rich.blocks()));
            let fetched = user.get_rich_message(bot_id, pushed.id).await.unwrap();
            let fetched = fetched.and_then(|m| m.rich_message).expect("the message has no rich content");
            assert_eq!(kinds(fetched.blocks()), ["collage", "slideshow"]);
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    if !to_delete.is_empty() {
        support::cleanup(user.delete_messages(bot_id, &to_delete).await, "deleting the collage test messages");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// Item 66 (the story lock): a story the user forwards to the bot reaches the bot as a story message. Needs a public
/// story: the first of the user's feed, @telegram's and @durov's that Telegram lets forward.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn a_forwarded_story_reaches_the_bot_as_a_story() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;

    let mut stories: Vec<(i64, i32)> =
        user.get_all_stories(false).await.unwrap().iter().map(|s| (s.chat.id, s.id)).collect();
    for name in ["telegram", "durov"] {
        if let Ok(found) = user.get_chat_stories(name).await {
            stories.extend(found.iter().map(|s| (s.chat.id, s.id)));
        }
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    kit.bot.on_message(filters::private() & filters::user(user_id), move |m| {
        if m.story().is_some() {
            let _ = tx.send(m);
        }
        async { Ok(()) }
    });

    let mut forwarded = None;
    for &(chat, id) in stories.iter().take(10) {
        match user.forward_story(bot_id, chat, id).await {
            Ok(sent) => {
                forwarded = Some((sent.id, chat, id));
                break;
            }
            // STORY_PUBLIC_MISSING and the like: this one can't be forwarded, try the next.
            Err(e) if e.rpc().is_some_and(|r| r.name.starts_with("STORY")) => {}
            Err(e) => panic!("forward_story failed: {e:?}"),
        }
    }
    let Some((sent_id, chat, id)) = forwarded else {
        eprintln!("skipped: none of {} stories found could be forwarded", stories.len());
        return;
    };
    let got = tokio::time::timeout(WAIT, rx.recv()).await;
    support::cleanup(user.delete_messages(bot_id, &[sent_id]).await, "deleting the forwarded story");
    let got = got.expect("the bot got no story message").expect("message channel closed");
    let story = got.story().expect("checked by the handler");
    assert_eq!((story.chat.id, story.id), (chat, id));
}

/// Item 66 (the guestbot lock): the user mentions the kit bot in a supergroup the bot isn't in, the bot answers the
/// guest query, and the answer reaches the user naming its caller. Needs Guest Mode on for the kit bot in @BotFather.
#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session; needs Guest Mode on the kit bot"]
async fn a_guest_bot_answer_names_its_caller() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let bot_me = kit.bot.get_me().await.unwrap();
    if !bot_me.supports_guest_queries {
        eprintln!("skipped: turn Guest Mode on for the kit bot in @BotFather");
        return;
    }
    let mention = format!("@{} hello", bot_me.username.expect("the kit bot has no username"));
    let user = kit.user.client.clone();
    let user_id = user.get_me().await.unwrap().id;
    let Some(group) = support::test_supergroup(&user).await else { return };

    let (asked_tx, mut asked) = mpsc::unbounded_channel();
    kit.bot.on_guest_message(filters::all(), move |g| {
        let _ = asked_tx.send(g.caller_user.as_ref().map(|u| u.id));
        async move { g.answer_text("guest answer from vaegram").await }
    });
    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::chat(group), move |m| {
        if m.guest_bot_caller.is_some() {
            let _ = tx.send(m);
        }
        async { Ok(()) }
    })
    .group(1);

    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            user.send_message(group, mention.as_str()).await.unwrap();
            let caller = tokio::time::timeout(WAIT, asked.recv()).await.expect("the bot got no guest query");
            assert_eq!(caller.expect("guest query channel closed"), Some(user_id));
            let answer = tokio::time::timeout(WAIT, rx.recv()).await.expect("no message naming a guest caller came");
            let answer = answer.expect("message channel closed");
            eprintln!(
                "guest answer: from {:?}, outgoing {}, caller user {:?}",
                answer.from_user().map(|u| u.id),
                answer.outgoing,
                answer.guest_bot_caller.as_ref().map(|s| s.id())
            );
            assert_eq!(answer.guest_bot_caller.as_ref().map(|s| s.id()), Some(user_id));
        }
    })
    .await;

    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// Item 67: `ban_chat_member(..).revoke_messages(true)` in a basic group with a second user as the removed member:
/// afterwards that user can't read the group's message by id any more. Settles what
/// `ban_chat_member_revoke_messages_in_basic_group` can't with a bot.
#[tokio::test]
#[ignore = "needs VAEGRAM_SECOND_USER_SESSION; creates and leaves a basic group"]
async fn revoke_messages_hides_history_from_a_removed_user() {
    let Some((second, _second_disconnect)) = second_user().await else { return };
    let Some((user, _disconnect)) = user_client().await else { return };
    let second_me = second.get_me().await.unwrap();
    let Some(username) = second_me.username.as_deref() else {
        eprintln!("skipped: the VAEGRAM_SECOND_USER_SESSION account needs a username for the first to add it");
        return;
    };
    let removed = user.resolve_username(username).await.unwrap();
    let created = match user.create_group("vaegram live test", [removed]).await {
        Ok(created) => created,
        Err(e) if support::flood_limited(&e, "creating a basic group") => return,
        Err(e) => panic!("create_group failed: {e:?}"),
    };
    let group = created.chat.id;
    if !created.failed_members.is_empty() {
        support::cleanup(user.leave_chat(group).await, "leaving the basic group \"vaegram live test\"");
        eprintln!("skipped: the second account couldn't be added: {:?}", created.failed_members);
        return;
    }

    let steps = tokio::spawn({
        let (user, second) = (user.clone(), second.clone());
        async move {
            user.send_message(group, "revoke check").await.unwrap();
            // In a basic group each member numbers the messages itself: find it in the second account's history.
            let mut seen = None;
            for _ in 0..10 {
                let history = second.get_chat_history(group).limit(5).await.unwrap();
                seen = history.iter().find(|m| m.text() == "revoke check").map(|m| m.id);
                if seen.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let id = seen.expect("the second account never saw the message");

            user.ban_chat_member(group, removed).revoke_messages(true).await.unwrap();
            let after = second.get_messages(group, &[id]).await;
            assert!(
                match &after {
                    Err(_) => true,
                    Ok(messages) => messages.first().is_none_or(Option::is_none),
                },
                "the removed user still reads the message after revoke_messages(true): {:?}",
                after.as_ref().map(|m| m.iter().map(|m| m.as_ref().map(|m| m.id)).collect::<Vec<_>>())
            );
        }
    })
    .await;

    support::cleanup(user.leave_chat(group).await, "leaving the basic group \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// Waits up to [`WAIT`] for `rx` to bring a value `hit` accepts.
async fn pushed<T>(rx: &mut mpsc::UnboundedReceiver<T>, hit: impl Fn(&T) -> bool) -> bool {
    let found = async {
        while let Some(value) = rx.recv().await {
            if hit(&value) {
                return true;
            }
        }
        false
    };
    tokio::time::timeout(WAIT, found).await.unwrap_or(false)
}

/// Item 67: updates that carry no message, in the supergroup `VAEGRAM_CHAT_ID`, reach the user by push alone: the
/// bot deletes its own message, and pins and unpins one when it may. Nothing else is sent meanwhile, so no gap makes
/// the client fetch a difference. Prints which kinds came and fails naming the ones that didn't.
#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and a supergroup VAEGRAM_CHAT_ID"]
async fn message_less_channel_updates_reach_the_user_by_push() {
    let Some(chat_id) = supergroup_chat_id() else { return };
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let bot_me = kit.bot.get_me().await.unwrap();
    let member = user.get_chat_member(chat_id, bot_me.id).await.unwrap();
    let bot_can_pin =
        member.status() == ChatMemberStatus::Administrator && member.privileges().is_some_and(|p| p.can_pin_messages);

    let (deleted_tx, mut deleted) = mpsc::unbounded_channel();
    user.on_deleted_messages(filters::all(), move |event| {
        if event.chat.as_ref().is_some_and(|c| c.id == chat_id) {
            let _ = deleted_tx.send(event.message_ids);
        }
        async { Ok(()) }
    });
    let (pinned_tx, mut pinned) = mpsc::unbounded_channel();
    user.on_pinned_messages(filters::all(), move |event| {
        if event.chat.id == chat_id {
            let _ = pinned_tx.send((event.is_pinned, event.message_ids));
        }
        async { Ok(()) }
    });

    let mut missing = Vec::new();
    let gone = kit.bot.send_message(chat_id, "vaegram: deleted for the push check").await.unwrap().id;
    kit.bot.delete_messages(chat_id, &[gone]).await.unwrap();
    if pushed(&mut deleted, |ids: &Vec<i32>| ids.contains(&gone)).await {
        eprintln!("pushed: the deletion");
    } else {
        missing.push("the deletion");
    }

    if bot_can_pin {
        let pin = kit.bot.send_message(chat_id, "vaegram: pinned for the push check").await.unwrap().id;
        match kit.bot.pin_chat_message(chat_id, pin).disable_notification().await {
            Ok(()) => {
                if pushed(&mut pinned, |(p, ids): &(bool, Vec<i32>)| *p && ids.contains(&pin)).await {
                    eprintln!("pushed: the pin");
                } else {
                    missing.push("the pin");
                }
                kit.bot.unpin_chat_message(chat_id, pin).await.unwrap();
                if pushed(&mut pinned, |(p, ids): &(bool, Vec<i32>)| !*p && ids.contains(&pin)).await {
                    eprintln!("pushed: the unpin");
                } else {
                    missing.push("the unpin");
                }
            }
            Err(e) if support::flood_limited(&e, "pinning a message") => {}
            Err(e) => panic!("pin_chat_message failed: {e:?}"),
        }
        support::cleanup(kit.bot.delete_messages(chat_id, &[pin]).await, "deleting the pin check message");
    } else {
        eprintln!("skipped the pins: the kit bot can't pin in VAEGRAM_CHAT_ID");
    }
    assert!(missing.is_empty(), "not pushed to the user within {WAIT:?} each: {missing:?}");
}

/// Which way the relay stops moving bytes on the connection it stalls.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Stall {
    Upload,
    Download,
}

/// Bytes a connection carries the stalled way before the relay goes quiet on it.
const STALL_AFTER: u64 = 1 << 20;

/// How long a transfer may take with one dead connection in it: a part's timeout on a link at the slowest rate
/// vaegram allows for is minutes.
const TRANSFER_LIMIT: Duration = Duration::from_secs(900);

#[derive(Default)]
struct RelayState {
    opened: AtomicUsize,
    stalled: AtomicBool,
    opened_at_stall: AtomicUsize,
}

/// A SOCKS5 relay on 127.0.0.1 that stalls one connection: the first to carry `STALL_AFTER` bytes the `stall` way.
async fn start_relay(stall: Stall) -> (u16, Arc<RelayState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(RelayState::default());
    tokio::spawn({
        let state = state.clone();
        async move {
            while let Ok((client, _)) = listener.accept().await {
                state.opened.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(relay_one(client, stall, state.clone()));
            }
        }
    });
    (port, state)
}

/// Answers one SOCKS5 CONNECT without authentication, then copies both ways.
async fn relay_one(mut client: TcpStream, stall: Stall, state: Arc<RelayState>) -> std::io::Result<()> {
    let mut greeting = [0u8; 2];
    client.read_exact(&mut greeting).await?;
    let mut methods = vec![0u8; usize::from(greeting[1])];
    client.read_exact(&mut methods).await?;
    client.write_all(&[5, 0]).await?;
    let mut request = [0u8; 4];
    client.read_exact(&mut request).await?;
    let host = match request[3] {
        1 => {
            let mut ip = [0u8; 4];
            client.read_exact(&mut ip).await?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            client.read_exact(&mut len).await?;
            let mut name = vec![0u8; usize::from(len[0])];
            client.read_exact(&mut name).await?;
            String::from_utf8_lossy(&name).into_owned()
        }
        4 => {
            let mut ip = [0u8; 16];
            client.read_exact(&mut ip).await?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => return Ok(()),
    };
    let mut port = [0u8; 2];
    client.read_exact(&mut port).await?;
    let upstream = TcpStream::connect((host.as_str(), u16::from_be_bytes(port))).await?;
    client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;

    let (client_read, client_write) = client.into_split();
    let (upstream_read, upstream_write) = upstream.into_split();
    let up = pump(client_read, upstream_write, (stall == Stall::Upload).then(|| state.clone()));
    let down = pump(upstream_read, client_write, (stall == Stall::Download).then_some(state));
    let _ = tokio::join!(up, down);
    Ok(())
}

/// Copies `from` to `to`. With `stall`, the first connection of all to carry `STALL_AFTER` bytes this way goes quiet:
/// nothing more is read or written, and both halves stay open, so only the client's own checks can find it dead.
async fn pump(mut from: OwnedReadHalf, mut to: OwnedWriteHalf, stall: Option<Arc<RelayState>>) -> std::io::Result<()> {
    let mut buf = vec![0u8; 64 << 10];
    let mut carried = 0u64;
    loop {
        let n = from.read(&mut buf).await?;
        if n == 0 {
            return to.shutdown().await;
        }
        carried += n as u64;
        if let Some(state) = stall.as_ref().filter(|_| carried >= STALL_AFTER) {
            if !state.stalled.swap(true, Ordering::SeqCst) {
                state.opened_at_stall.store(state.opened.load(Ordering::SeqCst), Ordering::SeqCst);
                let _held = (from, to);
                return std::future::pending().await;
            }
        }
        to.write_all(&buf[..n]).await?;
    }
}

/// Item 82: uploads 4 MiB to Saved Messages and downloads it again through a relay that kills one connection the
/// `stall` way halfway, live against Telegram. The client must find the dead connection itself (the write-stall
/// bound or the transfer connection's silence watch), open another and send the parts again, restarting the ones
/// that timed out.
async fn a_transfer_survives_a_dead_connection(stall: Stall) {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let (port, relay) = start_relay(stall).await;
    let proxy = Proxy::Socks5 { host: "127.0.0.1".into(), port, auth: None };
    let builder = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let client = builder.proxy(proxy).no_updates().connect().await.unwrap();
    let _disconnect = support::Disconnect(vec![client.clone()]);
    let reconnects = Arc::new(Mutex::new(Vec::new()));
    client.on_event({
        let reconnects = reconnects.clone();
        move |event| {
            if let ClientEvent::Reconnected { dc, attempt, .. } = event {
                reconnects.lock().unwrap().push((dc, attempt));
            }
        }
    });

    // Not compressible, so every byte crosses the relay.
    let data: Vec<u8> = (0..(4u32 << 20)).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    let started = std::time::Instant::now();
    let upload = async { client.send_document("me", Upload::bytes("vaegram-stall.bin", &data)).await };
    let sent = tokio::time::timeout(TRANSFER_LIMIT, upload).await.expect("the upload didn't finish in time");
    let sent = sent.expect("the upload failed");
    let download = tokio::time::timeout(TRANSFER_LIMIT, async { sent.download().await }).await;
    support::cleanup(client.delete_messages("me", &[sent.id]).await, "deleting the stall test document");
    let got = download.expect("the download didn't finish in time").expect("the download failed");

    let opened = relay.opened.load(Ordering::SeqCst);
    let after = opened - relay.opened_at_stall.load(Ordering::SeqCst);
    eprintln!(
        "{stall:?} stall: {opened} connections through the relay, {after} opened after the stall, reconnects \
         (dc, attempt) {:?}, {:?} in all",
        reconnects.lock().unwrap(),
        started.elapsed()
    );
    assert!(got == data, "the file downloaded differs from the one uploaded");
    assert!(relay.stalled.load(Ordering::SeqCst), "no connection carried {STALL_AFTER} bytes the {stall:?} way");
    assert!(after >= 1, "no connection was opened after the stall");
}

#[tokio::test]
#[ignore = "uses a logged-in user session through a local SOCKS5 relay; takes minutes"]
async fn an_upload_survives_a_dead_transfer_connection() {
    a_transfer_survives_a_dead_connection(Stall::Upload).await;
}

#[tokio::test]
#[ignore = "uses a logged-in user session through a local SOCKS5 relay; takes minutes"]
async fn a_download_survives_a_dead_transfer_connection() {
    a_transfer_survives_a_dead_connection(Stall::Download).await;
}
