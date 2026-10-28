use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use vaegram::connection::{Options, Proxy, Transport};
use vaegram::enums::ParseMode;
use vaegram::raw::{base, functions, types};
use vaegram::session::Session;
use vaegram::session::internals::data_center;
use vaegram::storage::state;
use vaegram::types::{
    ButtonStyle, InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResultArticle, InputRichMessage,
    KeyboardButton, Message, ReplyKeyboardMarkup, ReplyKeyboardRemove, Upload,
};
use vaegram::{Client, filters};

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn creates_auth_key_and_pings() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default()).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 42 }).await.unwrap();
    assert_eq!(pong.ping_id, 42);
}

#[tokio::test]
#[ignore = "connects to Telegram's test servers"]
async fn creates_auth_key_on_test_server() {
    let dc = data_center::find(&[], 2, true, false).unwrap();
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default()).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
    assert_eq!(pong.ping_id, 7);
}

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn handshakes_over_every_transport() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
        for obfuscated in [false, true] {
            let options = Options { transport, obfuscated, proxy: None };
            let (session, _updates) = Session::connect(dc, None, &options, Arc::default()).await.unwrap();
            let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
            assert_eq!(pong.ping_id, 7, "{options:?}");
        }
    }
}

#[tokio::test]
#[ignore = "connects through VAEGRAM_SOCKS5 (host:port[:user:pass]) and VAEGRAM_MTPROXY (host:port:secret)"]
async fn handshakes_through_proxies() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    let mut proxies = Vec::new();
    if let Ok(s) = std::env::var("VAEGRAM_SOCKS5") {
        let parts: Vec<&str> = s.split(':').collect();
        let auth = parts.get(2).zip(parts.get(3)).map(|(u, p)| ((*u).into(), (*p).into()));
        proxies.push(Proxy::Socks5 { host: parts[0].into(), port: parts[1].parse().unwrap(), auth });
    }
    if let Ok(s) = std::env::var("VAEGRAM_MTPROXY") {
        let parts: Vec<&str> = s.split(':').collect();
        proxies.push(Proxy::MtProxy {
            host: parts[0].into(),
            port: parts[1].parse().unwrap(),
            secret: parts[2].parse().unwrap(),
        });
    }
    if proxies.is_empty() {
        eprintln!("skipped: set VAEGRAM_SOCKS5 or VAEGRAM_MTPROXY");
        return;
    }
    for proxy in proxies {
        let options = Options { proxy: Some(proxy), ..Options::default() };
        let (session, _updates) = Session::connect(dc, None, &options, Arc::default()).await.unwrap();
        let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 9 }).await.unwrap();
        assert_eq!(pong.ping_id, 9, "{options:?}");
    }
}

#[tokio::test]
#[ignore = "logs in as a bot and sends a message"]
async fn bot_sends_and_edits_messages() {
    if std::env::var("VAEGRAM_BOT_TOKEN").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-test", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();

    let base::User::User(me) = app.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert!(me.bot);

    let chat_id = env("VAEGRAM_CHAT_ID").parse().unwrap();
    let hello = app.send_message(chat_id, "hello from vaegram").await.unwrap();
    assert_eq!((hello.chat.id, hello.text.as_str(), hello.outgoing), (chat_id, "hello from vaegram", true));
    assert_eq!(hello.from_user.as_ref().map(|u| u.id), Some(me.id));

    let sent = app.send_message(chat_id, "<b>bold</b>, <i>italic</i> and <code>code</code>").html().await.unwrap();
    assert_eq!((sent.text.as_str(), entities(&sent)), ("bold, italic and code", 3));
    let sent =
        app.send_message(chat_id, "**bold**, __italic__ and [a link](https://github.com)").markdown().await.unwrap();
    assert_eq!((sent.text.as_str(), entities(&sent)), ("bold, italic and a link", 3));

    let keyboard = InlineKeyboardMarkup::new([
        vec![
            InlineKeyboardButton::callback("Yes", "yes").style(ButtonStyle::Success),
            InlineKeyboardButton::callback("No", "no").style(ButtonStyle::Danger),
        ],
        vec![InlineKeyboardButton::url("Source", "https://github.com/rjriajul/vaegram")],
    ]);
    let asked = app.send_message(chat_id, "Do buttons work?").reply_markup(keyboard).await.unwrap();
    assert!(asked.raw.reply_markup.is_some());

    let edited = asked.edit_text("Buttons work, <b>edited</b>.").html().await.unwrap();
    assert_eq!((edited.id, edited.text.as_str(), entities(&edited)), (asked.id, "Buttons work, edited.", 1));
    assert!(edited.edit_date.is_some());
    assert!(edited.raw.reply_markup.is_none());

    // updateShortSentMessage has no rich_message field; this shows whether Telegram answers with it.
    let rich = app.send_rich_message(chat_id, InputRichMessage::html("<b>rich</b> hello")).await.unwrap();
    assert!(rich.rich_message.is_some());
    rich.delete().await.unwrap();

    let reply = app.send_message(chat_id, "a reply").reply_to(hello.id).await.unwrap();
    assert!(reply.raw.reply_to.is_some());

    let keyboard = ReplyKeyboardMarkup::new([vec![KeyboardButton::new("One"), KeyboardButton::new("Two")]]);
    app.send_message(chat_id, "a reply keyboard").reply_markup(keyboard).await.unwrap();
    let removed = app.send_message(chat_id, "keyboard removed").reply_markup(ReplyKeyboardRemove::default()).await;
    removed.unwrap().delete().await.unwrap();
    reply.delete().await.unwrap();

    let again = Client::builder("unused", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .session_string(app.export_session_string())
        .start()
        .await
        .unwrap();
    let base::User::User(me_again) = again.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert_eq!(me_again.id, me.id);
}

fn entities(message: &Message) -> usize {
    message.raw.entities.as_ref().map_or(0, Vec::len)
}

#[tokio::test]
#[ignore = "logs in as a bot and sends polls, dice, venues, contacts and locations"]
async fn bot_sends_polls_and_media() {
    if std::env::var("VAEGRAM_BOT_TOKEN").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-sends", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let chat_id: i64 = env("VAEGRAM_CHAT_ID").parse().unwrap();

    let dice = app.send_dice(chat_id).emoji("🎯").disable_notification().await.unwrap();
    let Some(base::MessageMedia::MessageMediaDice(d)) = &dice.raw.media else { panic!("no dice in {dice:?}") };
    assert_eq!(d.emoticon, "🎯");
    let venue = app.send_venue(chat_id, 51.5007, -0.1246, "Big Ben", "London SW1A 0AA").await.unwrap();
    assert!(matches!(venue.raw.media, Some(base::MessageMedia::MessageMediaVenue(_))));
    let contact = app.send_contact(chat_id, "+15555550100", "Vae").last_name("Gram").await.unwrap();
    assert!(matches!(contact.raw.media, Some(base::MessageMedia::MessageMediaContact(_))));
    let location = app.send_location(chat_id, 51.5007, -0.1246).reply_to(contact.id).await.unwrap();
    assert!(location.raw.reply_to.is_some());

    let quiz = app
        .send_poll(chat_id, "2 + 2?", ["3", "4"])
        .quiz([1])
        .explanation("<b>Basic</b> math")
        .explanation_parse_mode(ParseMode::Html)
        .await
        .unwrap();
    let stopped = app.stop_poll(chat_id, quiz.id).await.unwrap();
    let Some(base::MessageMedia::MessageMediaPoll(media)) = &stopped.raw.media else { panic!("no poll") };
    let base::Poll::Poll(poll) = &media.poll;
    assert!(poll.closed && poll.quiz);

    for message in [dice, venue, contact, location, stopped] {
        message.delete().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "logs in as a bot and uploads a file"]
async fn bot_uploads_and_downloads_a_document() {
    if std::env::var("VAEGRAM_BOT_TOKEN").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-files", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let chat_id: i64 = env("VAEGRAM_CHAT_ID").parse().unwrap();

    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
    let sent = app.send_document(chat_id, Upload::bytes("vaegram.bin", &data)).caption("a file").await.unwrap();
    let document = sent.document().expect("the message has the document");
    assert_eq!((document.file_name.as_deref(), document.file_size), (Some("vaegram.bin"), data.len() as i64));

    assert!(sent.download().await.unwrap() == data);
    let path = std::path::Path::new("target/vaegram-live-download/vaegram.bin");
    app.download_media(document).file_name(path).await.unwrap();
    assert!(std::fs::read(path).unwrap() == data);
    sent.delete().await.unwrap();
}

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn telegram_errors_come_back_as_rpc_errors() {
    match Client::builder("target/vaegram-live-bad-id", 0, "").start().await {
        Err(vaegram::Error::Rpc(e)) => assert_eq!(e.name, "CONNECTION_API_ID_INVALID"),
        other => panic!("expected CONNECTION_API_ID_INVALID, got {:?}", other.err()),
    }
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot"]
async fn user_account_talks_to_bot() {
    let (Ok(session), Ok(token)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name) and VAEGRAM_BOT_TOKEN");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let user = Client::builder(session, api_id, api_hash.clone()).connect().await.unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert!(!me.bot);

    let saved = user.send_message(me.id, "**saved** by vaegram").markdown().await.unwrap();
    assert_eq!((saved.chat.id, saved.text.as_str(), entities(&saved)), (me.id, "saved by vaegram", 1));
    let saved = saved.edit_text("edited by vaegram").await.unwrap();
    assert_eq!(saved.text, "edited by vaegram");
    saved.delete().await.unwrap();

    let again = Client::builder("unused", api_id, api_hash.clone())
        .session_string(user.export_session_string())
        .connect()
        .await
        .unwrap();
    let base::User::User(me_again) = again.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert_eq!(me_again.id, me.id);

    let bot = Client::builder("target/vaegram-live-bot-chat", api_id, api_hash).bot_token(token).start().await.unwrap();
    let base::User::User(bot_me) = bot.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let bot_id = user.resolve_username(bot_me.username.as_deref().unwrap()).await.unwrap();
    assert_eq!(bot_id, bot_me.id);

    let mention =
        user.send_message(me.id, format!("hi <a href=\"tg://user?id={bot_id}\">bot</a>")).html().await.unwrap();
    assert!(matches!(mention.raw.entities.as_deref(), Some([base::MessageEntity::MessageEntityMentionName(_)])));
    mention.delete().await.unwrap();

    let raw_updates = Arc::new(AtomicUsize::new(0));
    let counter = raw_updates.clone();
    bot.on_raw_update(move |_, _| {
        counter.fetch_add(1, Ordering::Relaxed);
        async { Ok(()) }
    });
    bot.on_message(filters::private() & filters::text(), |msg| async move {
        let tap = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Tap", "tap")]]);
        msg.reply(format!("pong: {}", msg.text)).reply_markup(tap).await?;
        Ok(())
    });
    bot.on_edited_message(filters::private(), |msg| async move {
        msg.reply(format!("edited: {}", msg.text)).await?;
        Ok(())
    });
    bot.on_callback_query(filters::data("tap"), |query| async move {
        query.answer().text("tapped").await?;
        query.edit_message_text("tapped!").await?;
        Ok(())
    });
    bot.on_inline_query(filters::all(), |query| async move {
        let result = InlineQueryResultArticle::new("Result", format!("inline: {}", query.query));
        query.answer([result]).cache_time(0).await?;
        Ok(())
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let from_bot = filters::user(bot_id) & filters::incoming();
    let sent = tx.clone();
    user.on_message(from_bot.clone(), move |msg| {
        let tx = sent.clone();
        async move {
            let _ = tx.send(("new", msg));
            Ok(())
        }
    });
    user.on_edited_message(from_bot, move |msg| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(("edit", msg));
            Ok(())
        }
    });
    let (bot_runner, user_runner) = (bot.clone(), user.clone());
    tokio::spawn(async move { bot_runner.run().await });
    tokio::spawn(async move { user_runner.run().await });
    tokio::time::sleep(Duration::from_secs(2)).await;

    let ping = user.send_message(bot_id, "ping").await.unwrap();
    let pong = next(&mut rx, "new").await;
    assert_eq!(pong.text, "pong: ping");
    assert!(pong.raw.reply_markup.is_some());

    let ping = ping.edit_text("ping 2").await.unwrap();
    assert_eq!(next(&mut rx, "new").await.text, "edited: ping 2");

    let peer = user.resolve_peer(bot_id).await.unwrap();
    let request = functions::messages::GetBotCallbackAnswer {
        game: false,
        peer: peer.clone(),
        msg_id: pong.id,
        data: Some(b"tap".to_vec()),
        password: None,
    };
    let base::messages::BotCallbackAnswer::BotCallbackAnswer(answer) = user.invoke(&request).await.unwrap();
    assert_eq!(answer.message.as_deref(), Some("tapped"));
    let tapped = next(&mut rx, "edit").await;
    assert_eq!((tapped.id, tapped.text.as_str()), (pong.id, "tapped!"));

    let base::InputPeer::InputPeerUser(bot_peer) = &peer else { panic!("the bot is not a user peer") };
    let bot_user =
        base::InputUser::InputUser(Box::new(types::InputUser { user_id: bot_id, access_hash: bot_peer.access_hash }));
    let request = functions::messages::GetInlineBotResults {
        bot: bot_user,
        peer,
        geo_point: None,
        query: "hi".into(),
        offset: String::new(),
    };
    let base::messages::BotResults::BotResults(results) = user.invoke(&request).await.unwrap();
    assert!(
        matches!(results.results.first(), Some(base::BotInlineResult::BotInlineResult(r)) if r.title.as_deref() == Some("Result"))
    );

    assert!(raw_updates.load(Ordering::Relaxed) >= 3);
    user.delete_messages(bot_id, &[ping.id, pong.id]).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_keeps_peers_across_restarts() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let name = "target/vaegram-live-restart";
    let state_path = format!("{name}.state");
    std::fs::copy(format!("{session}.session"), format!("{name}.session")).unwrap();
    let _ = std::fs::remove_file(&state_path);

    let user = Client::builder(name, api_id, api_hash.clone()).connect().await.unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let id = user.resolve_username("BotFather").await.unwrap();
    drop(user);

    let (peers, _) = state::load(Path::new(&state_path), me.id).unwrap();
    let saved = peers.get(&id).cloned().expect("BotFather was saved on drop");
    let user = Client::builder(name, api_id, api_hash).connect().await.unwrap();
    assert_eq!(user.resolve_peer(id).await.unwrap(), saved);
    drop(user);
    std::fs::remove_file(format!("{name}.session")).unwrap();
    std::fs::remove_file(state_path).unwrap();
}

async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<(&'static str, Message)>, kind: &str) -> Message {
    loop {
        let (k, msg) = tokio::time::timeout(Duration::from_secs(30), rx.recv())
            .await
            .unwrap_or_else(|_| panic!("no {kind} message from the bot within 30s"))
            .unwrap();
        if k == kind {
            return msg;
        }
    }
}
