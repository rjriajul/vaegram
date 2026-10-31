mod support;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use vaegram::connection::{Options, Proxy, Transport};
use vaegram::enums::{MessageEntityType, MessagesFilter, ParseMode, PollType, StickerType, StoriesPrivacyRules};
use vaegram::raw::{Serializable, base, functions, types};
use vaegram::session::Session;
use vaegram::session::internals::data_center;
use vaegram::storage::state;
use vaegram::types::{
    ButtonStyle, FileSource, Folder, InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResultArticle, InputMedia,
    InputPollMedia, InputPollOption, InputRichMessage, InputSticker, InputStoryContent, Json, KeyboardButton, Media,
    Message, ReplyKeyboardMarkup, ReplyKeyboardRemove, RichBlock, Text, Upload,
};
use vaegram::{Client, filters};

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn creates_auth_key_and_pings() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 42 }).await.unwrap();
    assert_eq!(pong.ping_id, 42);
}

#[tokio::test]
#[ignore = "connects to Telegram's test servers"]
async fn creates_auth_key_on_test_server() {
    let dc = data_center::find(&[], 2, true, false).unwrap();
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
    assert_eq!(pong.ping_id, 7);
}

#[tokio::test]
#[ignore = "connects to Telegram's test servers"]
async fn binds_temp_key_on_test_server() {
    let dc = data_center::find(&[], 2, true, false).unwrap();
    let init = Some(functions::help::GetNearestDc {}.to_bytes());
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default(), init).await.unwrap();
    // Requests wait for auth.bindTempAuthKey to succeed, so an answer means it did.
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 11 }).await.unwrap();
    assert_eq!(pong.ping_id, 11);
}

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn handshakes_over_every_transport() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
        for obfuscated in [false, true] {
            let options = Options { transport, obfuscated, ..Options::default() };
            let (session, _updates) = Session::connect(dc, None, &options, Arc::default(), None).await.unwrap();
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
        let (session, _updates) = Session::connect(dc, None, &options, Arc::default(), None).await.unwrap();
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
    assert!(asked.reply_markup().is_some());

    let edited = asked.edit_text("Buttons work, <b>edited</b>.").html().await.unwrap();
    assert_eq!((edited.id, edited.text.as_str(), entities(&edited)), (asked.id, "Buttons work, edited.", 1));
    assert!(edited.edit_date.is_some());
    assert!(edited.reply_markup().is_none());

    // updateShortSentMessage has no rich_message field; this shows whether Telegram answers with it.
    let rich = app.send_rich_message(chat_id, InputRichMessage::html("<b>rich</b> hello")).await.unwrap();
    assert!(rich.rich_message.is_some());
    let rich =
        rich.edit_text(InputRichMessage::blocks([RichBlock::paragraph(Text::new().bold("edited"))])).await.unwrap();
    assert!(rich.rich_message.is_some() && rich.text.contains("edited"));
    rich.delete().await.unwrap();

    let reply = app.send_message(chat_id, "a reply").reply_to(hello.id).await.unwrap();
    assert_eq!(reply.reply_to_message_id, Some(hello.id));

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
    message.entities.as_ref().map_or(0, Vec::len)
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
    let Some(Media::Dice(d)) = &dice.media else { panic!("no dice in {dice:?}") };
    assert_eq!(d.emoji, "🎯");
    let venue = app.send_venue(chat_id, 51.5007, -0.1246, "Big Ben", "London SW1A 0AA").await.unwrap();
    assert!(matches!(venue.media, Some(Media::Venue(_))));
    let contact = app.send_contact(chat_id, "+15555550100", "Vae").last_name("Gram").await.unwrap();
    assert!(matches!(contact.media, Some(Media::Contact(_))));
    let location = app.send_location(chat_id, 51.5007, -0.1246).reply_to(contact.id).await.unwrap();
    assert_eq!(location.reply_to_message_id, Some(contact.id));

    let quiz = app
        .send_poll(chat_id, "2 + 2?", ["3", "4"])
        .quiz([1])
        .explanation("<b>Basic</b> math")
        .explanation_parse_mode(ParseMode::Html)
        .await
        .unwrap();
    let stopped = app.stop_poll(chat_id, quiz.id).await.unwrap();
    let Some(Media::Poll(poll)) = &stopped.media else { panic!("no poll") };
    assert!(poll.is_closed && poll.kind == PollType::Quiz);

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

    let (mut reported, mut streamed) = (0, Vec::new());
    let mut stream = sent.download().progress(|done, _| reported = done).stream();
    while let Some(chunk) = stream.next().await {
        streamed.extend_from_slice(&chunk.unwrap());
    }
    drop(stream);
    assert!(streamed == data);
    assert_eq!(reported, data.len() as u64);

    let again = app.send_document(chat_id, FileSource::file_id(&document.file_id).unwrap()).await.unwrap();
    let copy = again.document().expect("the copy has the document");
    assert_eq!((copy.id, copy.file_unique_id.as_str()), (document.id, document.file_unique_id.as_str()));
    assert!(app.download_media(&copy.file_id).await.unwrap() == data);
    again.delete().await.unwrap();
    sent.delete().await.unwrap();
}

#[tokio::test]
#[ignore = "logs in as a bot and uploads a photo and a video; the chat must be a channel the bot posts in"]
async fn bot_sends_live_photos_paid_media_and_poll_media() {
    if std::env::var("VAEGRAM_PHOTO").is_err() || std::env::var("VAEGRAM_VIDEO").is_err() {
        eprintln!("skipped: set the bot variables plus VAEGRAM_PHOTO (a JPEG) and VAEGRAM_VIDEO (a short MP4)");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-photo", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let chat_id: i64 = env("VAEGRAM_CHAT_ID").parse().unwrap();
    let (photo, video) = (env("VAEGRAM_PHOTO"), env("VAEGRAM_VIDEO"));

    let sent = app.send_live_photo(chat_id, photo.as_str(), video.as_str()).caption("live").await.unwrap();
    let live = sent.live_photo().expect("the message has the live photo");
    assert!(live.photo.is_some() && live.file_size > 0);
    sent.delete().await.unwrap();

    let media = [
        InputMedia::photo(photo.as_str()),
        InputMedia::video(video.as_str()),
        InputMedia::live_photo(photo.as_str(), video.as_str()),
    ];
    let sent = app.send_paid_media(chat_id, 5, media).payload("order-7").caption("paid").await.unwrap();
    assert!(matches!(sent.media, Some(Media::PaidMedia(ref m)) if m.star_count == 5));
    sent.delete().await.unwrap();

    let options = [
        InputPollOption::new("Here").media(InputPollMedia::location(48.85, 2.35)),
        InputPollOption::new("This").media(InputMedia::photo(photo.as_str())),
    ];
    let sent = app
        .send_poll(chat_id, "Where?", options)
        .description_media(InputPollMedia::link("https://telegram.org"))
        .await
        .unwrap();
    assert!(sent.poll().is_some());
    sent.delete().await.unwrap();
}

#[tokio::test]
#[ignore = "logs in as a bot and sends audio, voice, an album and a media edit"]
async fn bot_sends_typed_files_and_albums() {
    if std::env::var("VAEGRAM_BOT_TOKEN").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-media", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let chat_id: i64 = env("VAEGRAM_CHAT_ID").parse().unwrap();
    let data: Vec<u8> = (0..200_000u32).map(|i| (i * 13 + i / 7) as u8).collect();

    let song = Upload::bytes("song.mp3", &data);
    let song = app.send_audio(chat_id, song).duration(3).performer("vaegram").title("Song").await.unwrap();
    let audio = song.audio().expect("the song is audio");
    assert_eq!((audio.performer.as_deref(), audio.title.as_deref()), (Some("vaegram"), Some("Song")));

    let voice = app.send_voice(chat_id, Upload::bytes("voice.ogg", &data)).duration(3).await.unwrap();
    assert!(voice.voice().is_some());

    let album = [
        InputMedia::document(Upload::bytes("a.bin", &data)).caption("<b>first</b>").html(),
        InputMedia::document(Upload::bytes("b.bin", &data)),
    ];
    let album = app.send_media_group(chat_id, album).await.unwrap();
    assert_eq!(album.len(), 2);
    assert!(album[0].id < album[1].id && album[0].media_group_id.is_some());
    assert_eq!(album[0].caption.as_deref(), Some("first"));

    let replaced = InputMedia::document(Upload::bytes("c.bin", &data)).caption("replaced");
    let edited = app.edit_message_media(chat_id, album[1].id, replaced).await.unwrap();
    let name = edited.document().and_then(|d| d.file_name.as_deref());
    assert_eq!((edited.caption.as_deref(), name), (Some("replaced"), Some("c.bin")));

    let first = album[0].document().expect("the album has documents");
    let again = app.send_document(chat_id, first).await.unwrap();
    assert_eq!(again.document().map(|d| d.id), Some(first.id));

    app.delete_messages(chat_id, &[song.id, voice.id, album[0].id, album[1].id, again.id]).await.unwrap();
}

#[tokio::test]
#[ignore = "logs in as a bot and manages a sticker set"]
async fn bot_manages_a_sticker_set() {
    let (Ok(_), Ok(owner), Ok(file)) = (
        std::env::var("VAEGRAM_BOT_TOKEN"),
        std::env::var("VAEGRAM_STICKER_OWNER"),
        std::env::var("VAEGRAM_STICKER_FILE"),
    ) else {
        eprintln!(
            "skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN, VAEGRAM_STICKER_OWNER (a user who \
             started the bot) and VAEGRAM_STICKER_FILE (a 512 px .webp or .png)"
        );
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-sets", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let (owner, file) = (owner.parse::<i64>().unwrap(), file.as_str());
    let base::User::User(me) = app.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let name = format!("vaegram_{now}_by_{}", me.username.expect("bots have a username"));

    let uploaded = app.upload_sticker_file(owner, file).await.unwrap();
    let first = vec![InputSticker::existing(&uploaded, "😀")];
    let set = app.create_new_sticker_set(owner, &name, "vaegram test", first).await.unwrap();
    assert_eq!((set.name.as_str(), set.sticker_type, set.stickers.len()), (name.as_str(), StickerType::Regular, 1));

    let party = InputSticker::new(file, "🎉").keywords(["party", "fun"]);
    let set = app.add_sticker_to_set(owner, &name, party).await.unwrap();
    let added = set.stickers[1].clone();
    let set = app.set_sticker_position_in_set(&added, 0).await.unwrap();
    assert_eq!(set.stickers[0].id, added.id);
    app.set_sticker_emoji_list(&added, "😎🔥").await.unwrap();
    app.set_sticker_keywords(&added, ["cool"]).await.unwrap();
    let set = app.set_sticker_set_title(&name, "vaegram renamed").await.unwrap();
    assert_eq!(set.title, "vaegram renamed");

    let set = app.replace_sticker_in_set(owner, &set.stickers[1], InputSticker::new(file, "🙂")).await.unwrap();
    assert_eq!(set.stickers.len(), 2);
    let set = app.delete_sticker_from_set(&set.stickers[1]).await.unwrap();
    assert_eq!(set.stickers.len(), 1);
    app.set_sticker_set_thumbnail(&name, owner, None).await.unwrap();
    let fetched = app.get_sticker_set(&name).await.unwrap();
    assert_eq!((fetched.id, fetched.stickers[0].id), (set.id, added.id));
    app.delete_sticker_set(&name).await.unwrap();
    assert!(app.get_sticker_set(&name).await.is_err());

    let icons = app.get_forum_topic_icon_stickers().await.unwrap();
    let ids: Vec<i64> = icons.iter().map(|icon| icon.id).take(3).collect();
    let emoji = app.get_custom_emoji_stickers(&ids).await.unwrap();
    assert_eq!(emoji.len(), ids.len());
    for sticker in &emoji {
        assert_eq!(sticker.kind, StickerType::CustomEmoji);
        assert!(sticker.custom_emoji_id.is_some_and(|id| ids.contains(&id)));
    }
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
    assert!(matches!(mention.entities.as_deref(), Some([e]) if e.kind == MessageEntityType::TextMention));
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
    assert!(pong.reply_markup().is_some());

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

    let (peers, _, _) = state::load(Path::new(&state_path), me.id).unwrap();
    let saved = peers.get(&id).cloned().expect("BotFather was saved on drop");
    let user = Client::builder(name, api_id, api_hash).connect().await.unwrap();
    assert_eq!(user.resolve_peer(id).await.unwrap(), saved);
    drop(user);
    std::fs::remove_file(format!("{name}.session")).unwrap();
    std::fs::remove_file(state_path).unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot"]
async fn user_gets_the_bots_reply_through_the_kit() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        m.reply(format!("echo: {}", m.text)).await?;
        Ok(())
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, "hi from the kit").await.unwrap();
    assert_eq!((reply.text.as_str(), reply.outgoing), ("echo: hi from the kit", false));
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

#[tokio::test]
#[ignore = "logs in as a bot and changes its own and a chat's photo"]
async fn bot_sets_and_removes_photos() {
    let vars = ["VAEGRAM_API_ID", "VAEGRAM_API_HASH", "VAEGRAM_BOT_TOKEN", "VAEGRAM_PHOTO_CHAT_ID", "VAEGRAM_PHOTO"];
    if vars.iter().any(|k| std::env::var(k).is_err()) {
        eprintln!("skipped: set {}; the bot must be an admin of the chat and the photo a JPEG", vars.join(", "));
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-photos", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();
    let (chat_id, photo) = (env("VAEGRAM_PHOTO_CHAT_ID").parse::<i64>().unwrap(), env("VAEGRAM_PHOTO"));

    app.set_profile_photo(photo.as_str()).await.unwrap();
    app.remove_profile_photo().await.unwrap();
    app.set_chat_photo(chat_id, photo.as_str()).await.unwrap();
    app.delete_chat_photo(chat_id).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_lists_active_sessions() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .device_model("vaegram live test")
        .connect()
        .await
        .unwrap();
    let sessions = user.get_active_sessions().await.unwrap();
    let current = sessions.active_sessions.iter().find(|s| s.is_current).expect("no current session listed");
    assert_eq!(current.device_model, "vaegram live test");
}

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(client.connect().await.unwrap())
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn account_settings_round_trip() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };

    // Every call writes back what it read, so the account is left as it was.
    let days = user.get_account_ttl().await.unwrap();
    assert!(user.set_account_ttl(days).await.unwrap());

    let key = vaegram::enums::PrivacyKey::Bio;
    let rules = user.get_privacy(key).await.unwrap();
    assert_eq!(user.set_privacy(key, &rules).await.unwrap(), rules);

    let global = user.get_global_privacy_settings().await.unwrap();
    assert_eq!(user.set_global_privacy_settings(global).await.unwrap(), global);

    let notify = user.get_chat_notifications(me.id).await.unwrap();
    assert!(user.update_chat_notifications(me.id).mute_until(notify.mute_until.unwrap_or(0)).await.unwrap());

    let first_name = me.first_name.unwrap_or_default();
    assert_eq!(user.update_profile().first_name(first_name.as_str()).await.unwrap().first_name, first_name);
    assert!(user.update_status(false).await.unwrap());
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session"]
async fn user_creates_and_manages_a_supergroup() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();

    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap();
    assert_eq!(group.kind, vaegram::enums::ChatType::Supergroup);
    user.set_slow_mode(group.id, Some(10)).await.unwrap();
    user.set_chat_protected_content(group.id, true).await.unwrap();
    user.toggle_forum_topics(group.id, true, false).await.unwrap();
    assert!(!user.get_send_as_chats(group.id).await.unwrap().is_empty());
    user.delete_chat_history(group.id).revoke().await.unwrap();
    user.delete_supergroup(group.id).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_lists_dialogs_and_contacts() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };

    let mut dialogs = user.get_dialogs().limit(150);
    let mut seen = Vec::new();
    while let Some(dialog) = dialogs.next().await.unwrap() {
        assert!(!seen.contains(&dialog.chat.id), "chat {} listed twice", dialog.chat.id);
        seen.push(dialog.chat.id);
    }
    let total = user.get_dialogs_count(false).await.unwrap();
    assert!(seen.len() <= 150 && seen.len() as i32 <= total);

    user.pin_chat(me.id).await.unwrap();
    user.unpin_chat(me.id).await.unwrap();
    user.mark_chat_unread(me.id).await.unwrap();
    let peer = types::InputDialogPeer { peer: user.resolve_peer(me.id).await.unwrap() };
    let peer = base::InputDialogPeer::InputDialogPeer(Box::new(peer));
    user.invoke(&functions::messages::MarkDialogUnread { unread: false, parent_peer: None, peer }).await.unwrap();
    assert!(user.get_chat_history_count(me.id).await.unwrap() >= 0);

    let contacts = user.get_contacts().await.unwrap();
    assert_eq!(contacts.len() as i32, user.get_contacts_count().await.unwrap());
    user.search_contacts("telegram", 5).await.unwrap();
    user.get_blocked_message_senders(0, 10).await.unwrap();
    if let Some(phone) = &me.phone {
        assert_eq!(user.resolve_phone(phone).await.unwrap(), me.id);
    }

    let botfather = user.resolve_username("BotFather").await.unwrap();
    user.block_user(botfather).await.unwrap();
    let blocked = user.get_blocked_message_senders(0, 100).await;
    // Unblock before asserting so a failure doesn't leave BotFather blocked.
    user.unblock_user(botfather).await.unwrap();
    assert!(blocked.unwrap().iter().any(|c| c.id == botfather));
    user.get_common_chats(botfather).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_stories() {
    let Some(user) = user_client().await else { return };
    let stories = user.get_all_stories(false).await.unwrap();
    let Some(story) = stories.first() else { return };
    let chat_id = story.chat.id;
    let fetched = user.get_stories(chat_id, &[story.id]).await.unwrap();
    assert_eq!(fetched.first().map(|s| s.id), Some(story.id));
    assert!(user.get_chat_stories(chat_id).await.unwrap().iter().any(|s| s.id == story.id));
    user.view_stories(chat_id, &[story.id]).await.unwrap();
    user.read_stories(chat_id, story.id).await.unwrap();
}

#[tokio::test]
#[ignore = "posts, edits, pins and deletes a story with a logged-in user session"]
async fn user_posts_and_edits_a_story() {
    let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a JPEG path) and VAEGRAM_USER_SESSION");
        return;
    };
    let Some(user) = user_client().await else { return };
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let story = user
        .send_story(me.id, InputStoryContent::photo(photo.as_str()))
        .caption("<b>vaegram</b>")
        .html()
        .privacy(StoriesPrivacyRules::CloseFriends)
        .protect_content()
        .await
        .unwrap();
    assert_eq!((story.chat.id, story.caption.as_deref(), story.is_close_friends), (me.id, Some("vaegram"), true));

    let edited = user.edit_story_caption(me.id, story.id, "edited").await.unwrap();
    assert_eq!(edited.caption.as_deref(), Some("edited"));
    let only_me = StoriesPrivacyRules::SelectedUsers(vec![me.id]);
    assert!(!user.edit_story_privacy(me.id, story.id, only_me).await.unwrap().is_close_friends);

    assert_eq!(user.pin_chat_stories(me.id, &[story.id]).await.unwrap(), [story.id]);
    assert_eq!(user.get_pinned_stories(me.id).limit(10).next().await.unwrap().map(|s| s.id), Some(story.id));
    assert_eq!(user.unpin_chat_stories(me.id, &[story.id]).await.unwrap(), [story.id]);
    assert_eq!(user.get_stories_archive(me.id).limit(10).next().await.unwrap().map(|s| s.id), Some(story.id));
    assert_eq!(user.delete_stories(me.id, &[story.id]).await.unwrap(), [story.id]);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_manages_folders() {
    let Some(user) = user_client().await else { return };
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let folder = Folder { title: "vaegram".into(), included_chats: vec![me.id], ..Folder::default() };
    let mut folder = user.create_folder(folder).await.unwrap();
    assert!(user.get_folders().await.unwrap().iter().any(|f| f.id == folder.id && f.title == "vaegram"));

    folder.title = "vaegram 2".into();
    user.edit_folder(&folder).await.unwrap();
    let ids: Vec<i32> = user.get_folders().await.unwrap().iter().map(|f| f.id).collect();
    user.reorder_folders(&ids).await.unwrap();
    user.delete_folder(folder.id).await.unwrap();
    assert!(!user.get_folders().await.unwrap().iter().any(|f| f.id == folder.id));
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_shares_a_folder() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a channel or supergroup you're in)");
        return;
    };
    let Some(user) = user_client().await else { return };
    let chat_id: i64 = chat_id.parse().unwrap();
    let folder = Folder { title: "vaegram".into(), included_chats: vec![chat_id], ..Folder::default() };
    let folder = user.create_folder(folder).await.unwrap();

    let link = user.create_folder_invite_link(folder.id, [chat_id]).name("vaegram").await.unwrap();
    assert_eq!((link.name.as_str(), link.chat_ids.as_slice()), ("vaegram", [chat_id].as_slice()));
    let info = user.check_folder_invite_link(&link.invite_link).await.unwrap();
    assert_eq!((info.folder_id, info.added_chats.len()), (Some(folder.id), 1));
    let link = user.edit_folder_invite_link(folder.id, &link.invite_link).name("edited").await.unwrap();
    assert_eq!(link.name, "edited");
    assert!(user.get_folder_invite_links(folder.id).await.unwrap().iter().any(|l| l.invite_link == link.invite_link));
    user.delete_folder_invite_link(folder.id, &link.invite_link).await.unwrap();
    user.delete_folder(folder.id).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_reaction_and_effect_lists() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();

    let available = user.get_available_reactions().await.unwrap();
    assert!(available.iter().any(|r| r.emoji == "👍" && !r.premium));
    let effects = user.get_available_effects().await.unwrap();
    assert!(effects.iter().all(|e| e.id != 0 && !e.emoji.is_empty()));
    assert!(user.get_top_reactions(10).await.unwrap().len() <= 10);
    user.get_recent_reactions(10).await.unwrap();
    user.get_saved_reaction_tags().await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session and the public @gif bot"]
async fn user_uses_inline_bot() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let gif = user.resolve_username("gif").await.unwrap();

    let results = user.get_inline_bot_results(gif, "cat").await.unwrap();
    let id = results.results.first().expect("@gif found no cats").id.clone();
    let sent = user.send_inline_bot_result(me.id, results.query_id, id).await.unwrap();
    assert_eq!(sent.chat.id, me.id);
    sent.delete().await.unwrap();

    let started = user.start_bot(gif, gif, "vaegram").await.unwrap();
    assert_eq!((started.chat.id, started.outgoing), (gif, true));
    started.delete().await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_searches_and_translates() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();

    let marker = format!("vaegram search {}", std::process::id());
    let saved = user.send_message("me", marker.clone()).await.unwrap();
    // Search indexing lags a little behind sending, so counts and results may not include the marker yet.
    user.search_messages_count("me").query(marker.as_str()).await.unwrap();
    let mut found = user.search_global(marker.as_str()).limit(5);
    while let Some(message) = found.next().await.unwrap() {
        assert!(message.text.contains(&marker));
    }
    user.search_global_count("telegram", MessagesFilter::Url).await.unwrap();
    saved.delete().await.unwrap();

    let spanish = user.translate_text("es", "<b>Good morning</b>").html().await.unwrap();
    assert_eq!((spanish.text.is_empty(), spanish.entities.map(|e| e.len())), (false, Some(1)));
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_premium_and_boosts() {
    let Some(user) = user_client().await else { return };
    user.get_premium_promo().await.unwrap();
    let boosts = user.get_my_boosts().await.unwrap();
    assert!(boosts.iter().all(|b| b.expire_date >= b.date));
    let err = user.check_gift_code("https://t.me/giftcode/vaegram-no-such-code").await.unwrap_err();
    assert!(matches!(err, vaegram::Error::Rpc(_)), "{err}");
}

#[tokio::test]
#[ignore = "reads the admin log and statistics of a chat the user administers"]
async fn admin_reads_event_log_and_statistics() {
    let Some(user) = user_client().await else { return };
    let Ok(chat_id) = std::env::var("VAEGRAM_ADMIN_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_ADMIN_CHAT_ID (a chat you administer)");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();

    let mut log = user.get_chat_event_log(chat_id).limit(150);
    let mut ids = Vec::new();
    while let Some(event) = log.next().await.unwrap() {
        ids.push(event.id);
    }
    assert!(ids.len() <= 150 && ids.windows(2).all(|w| w[0] > w[1]), "events come newest first, without repeats");

    // Chats too small for statistics are refused with CHAT_ADMIN_REQUIRED; big ones may live on another DC.
    let statistics = match user.get_channel_statistics(chat_id).await {
        Err(vaegram::Error::Rpc(e)) if e.name == "BROADCAST_REQUIRED" => {
            user.get_supergroup_statistics(chat_id).await.map(drop)
        }
        other => other.map(drop),
    };
    let refused = |e: &vaegram::Error| matches!(e, vaegram::Error::Rpc(e) if matches!(e.name.as_str(), "CHAT_ADMIN_REQUIRED" | "STATS_MIGRATE"));
    assert!(statistics.as_ref().err().is_none_or(refused), "{statistics:?}");
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_browses_app_bots() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name), optionally VAEGRAM_BOT_APP=bot/app");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let user = Client::builder(session, api_id, api_hash).connect().await.unwrap();

    let mut popular = user.get_popular_app_bots().limit(3);
    let mut bots = Vec::new();
    while let Some(bot) = popular.next().await.unwrap() {
        bots.push(bot);
    }
    assert_eq!((bots.len(), bots.iter().all(|b| b.is_bot)), (3, true));
    let similar = user.get_similar_bots(bots[0].id).await.unwrap();
    assert!(similar.iter().all(|b| b.is_bot && b.id != bots[0].id));

    if let Some(added) = user.get_attach_menu_bots().await.unwrap().first() {
        let again = user.get_attach_menu_bot(added.bot.id).await.unwrap();
        assert_eq!((again.bot.id, again.short_name.as_str()), (added.bot.id, added.short_name.as_str()));
    }

    if let Some((bot, short_name)) = std::env::var("VAEGRAM_BOT_APP").ok().as_deref().and_then(|l| l.split_once('/')) {
        let app = user.get_bot_app(bot, short_name).await.unwrap();
        assert_eq!(app.short_name, short_name);
    }
}

#[tokio::test]
#[ignore = "uses a logged-in user session with Telegram Business"]
async fn user_manages_business_links_and_quick_replies() {
    let Some(user) = user_client().await else { return };
    let replies = user.get_quick_replies().await.unwrap();
    assert!(replies.iter().all(|q| !q.shortcut.is_empty()));
    user.get_connected_bots().await.unwrap();

    let link = user.create_business_chat_link("**hi** from vaegram").markdown().title("vaegram").await.unwrap();
    let got = (link.text.as_str(), link.title.as_deref(), link.entities.as_ref().map(Vec::len));
    assert_eq!(got, ("hi from vaegram", Some("vaegram"), Some(1)));
    let link = user.edit_business_chat_link(link.slug(), "edited").await.unwrap();
    assert!(user.get_business_chat_links().await.unwrap().iter().any(|l| l.link == link.link && l.text == "edited"));
    assert_eq!(user.resolve_business_chat_link(link.slug()).await.unwrap().text, "edited");
    assert!(user.delete_business_chat_link(link.slug()).await.unwrap());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_app_config_and_languages() {
    let Some(user) = user_client().await else { return };
    assert!((1..=5).contains(&user.get_nearest_dc().await.unwrap().this_dc));
    assert!(matches!(user.get_app_config().await.unwrap(), Json::Object(_)));
    assert!(user.get_countries_list("en").await.unwrap().iter().any(|c| c.iso2 == "US"));
    assert!(!user.get_timezones_list().await.unwrap().is_empty());
    user.get_terms_of_service_update().await.unwrap();
    user.get_promo_data().await.unwrap();

    let languages = user.get_languages("android").await.unwrap();
    assert!(languages.iter().any(|l| l.lang_code == "en" && l.official));
    let en = user.get_language("android", "en").await.unwrap();
    assert_eq!((en.lang_code.as_str(), en.rtl), ("en", false));
    let pack = user.get_lang_pack("android", "en").await.unwrap();
    assert!(!pack.strings.is_empty());
    let diff = user.get_lang_pack_difference("android", "en", pack.version).await.unwrap();
    assert!(diff.version >= pack.version);
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session"]
async fn user_lists_and_pins_forum_topics() {
    let Some(user) = user_client().await else { return };
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap();
    user.toggle_forum_topics(group.id, true, false).await.unwrap();
    let first = user.create_forum_topic(group.id, "first").await.unwrap();
    let second = user.create_forum_topic(group.id, "second").await.unwrap();

    let mut topics = user.get_forum_topics(group.id).limit(10);
    let mut ids = Vec::new();
    while let Some(topic) = topics.next().await.unwrap() {
        ids.push(topic.id);
    }
    assert!(ids.contains(&first.id) && ids.contains(&second.id), "{ids:?}");

    user.pin_forum_topic(group.id, first.id).await.unwrap();
    assert!(user.get_forum_topics_by_id(group.id, &[first.id]).await.unwrap()[0].is_pinned);
    user.unpin_forum_topic(group.id, first.id).await.unwrap();
    assert!(!user.get_forum_topics_by_id(group.id, &[first.id]).await.unwrap()[0].is_pinned);
    user.delete_supergroup(group.id).await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_copies_and_forwards_an_album() {
    let Some(user) = user_client().await else { return };
    let data: Vec<u8> = (0..200_000u32).map(|i| (i * 13 + i / 7) as u8).collect();
    let album =
        [InputMedia::document(Upload::bytes("a.bin", &data)), InputMedia::document(Upload::bytes("b.bin", &data))];
    let album = user.send_media_group("me", album).await.unwrap();
    assert_eq!(user.get_media_group("me", album[1].id).await.unwrap().len(), 2);

    let forwarded = user.forward_media_group("me", "me", album[0].id).await.unwrap();
    assert_eq!(forwarded.len(), 2);
    assert!(forwarded[0].media_group_id.is_some() && forwarded[0].media_group_id == forwarded[1].media_group_id);
    assert_ne!(forwarded[0].media_group_id, album[0].media_group_id);

    let copied = user.copy_media_group("me", "me", album[0].id).caption(0, "copied").await.unwrap();
    assert_eq!((copied.len(), copied[0].caption.as_deref()), (2, Some("copied")));

    let ids: Vec<i32> = [album, forwarded, copied].iter().flatten().map(|m| m.id).collect();
    user.delete_messages("me", &ids).await.unwrap();
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session"]
async fn user_adds_and_deletes_poll_options() {
    let Some(user) = user_client().await else { return };
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap();
    let sent = user.send_poll(group.id, "Lunch?", ["A", "B"]).allow_adding_options().await.unwrap();
    user.add_poll_option(group.id, sent.id, "C").await.unwrap();
    let options = |messages: Vec<Option<Message>>| {
        messages[0].as_ref().and_then(Message::poll).map(|p| p.options.len()).expect("the poll is gone")
    };
    assert_eq!(options(user.get_messages(group.id, &[sent.id]).await.unwrap()), 3);
    user.delete_poll_option(group.id, sent.id, 2).await.unwrap();
    assert_eq!(options(user.get_messages(group.id, &[sent.id]).await.unwrap()), 2);
    user.delete_supergroup(group.id).await.unwrap();
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session"]
async fn user_manages_invite_links() {
    let Some(user) = user_client().await else { return };
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap();
    let link = user.create_chat_invite_link(group.id).name("t").await.unwrap().invite_link;

    assert_eq!(user.get_chat_invite_link(group.id, &link).await.unwrap().name.as_deref(), Some("t"));
    assert!(user.get_chat_invite_link_joiners(group.id, &link).await.unwrap().is_empty());
    assert_eq!(user.get_chat_invite_link_joiners_count(group.id, &link).await.unwrap(), 0);

    let active = user.get_chat_admin_invite_links(group.id, "me").await.unwrap();
    assert!(active.iter().any(|l| l.invite_link == link));
    assert!(user.get_chat_admin_invite_links_count(group.id, "me", false).await.unwrap() >= 1);
    let admins = user.get_chat_admins_with_invite_links(group.id).await.unwrap();
    assert!(admins.iter().any(|a| a.admin.id == me.id));

    user.revoke_chat_invite_link(group.id, &link).await.unwrap();
    let revoked = user.get_chat_admin_invite_links(group.id, "me").revoked().await.unwrap();
    assert!(revoked.iter().any(|l| l.invite_link == link));
    user.delete_chat_invite_link(group.id, &link).await.unwrap();
    user.delete_chat_admin_invite_links(group.id, "me").await.unwrap();
    user.delete_supergroup(group.id).await.unwrap();
}
