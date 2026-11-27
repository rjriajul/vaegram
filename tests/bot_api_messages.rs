//! A client in Bot API mode against a local HTTP stub, for the message methods past the core ones: which Bot API
//! method each friendly method sends, the fields it fills, and what the friendly method returns. One block per
//! mapped method.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::bot_api::Unsupported;
use vaegram::enums::ChatAction;
use vaegram::raw::{base, functions, types};
use vaegram::types::{FileSource, InputChecklist, InputMedia, InputRichMessage, ReplyParameters};
use vaegram::{Client, filters};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;
/// A basic group, Bot API style.
const GROUP: i64 = -42;
/// A supergroup, Bot API style.
const CHANNEL: i64 = -1_000_000_000_123;
const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";
/// The ephemeral message `editEphemeralMessageText` refuses, as a media one.
const EPHEMERAL_MEDIA: i64 = 6;

/// One request the stub got: the Bot API method and its JSON body.
type Call = (String, Value);

/// What the first `getUpdates` answers; later ones wait a little and answer nothing.
type Pending = Arc<Mutex<Option<Vec<Value>>>>;

/// A user `getChat` doesn't know.
const UNKNOWN_USER: i64 = 404;
/// A user who blocked the bot, which `getChat` refuses with a 403.
const BLOCKED_USER: i64 = 403;

struct Stub {
    url: String,
    calls: mpsc::UnboundedReceiver<Call>,
}

impl Stub {
    async fn start() -> Stub {
        Stub::with_updates(Vec::new()).await
    }

    /// A stub whose first `getUpdates` answers `updates`.
    async fn with_updates(updates: Vec<Value>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, calls) = mpsc::unbounded_channel();
        let pending: Pending = Arc::new(Mutex::new(Some(updates).filter(|list| !list.is_empty())));
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone(), pending.clone()));
            }
        });
        Stub { url, calls }
    }

    /// The next request a friendly method made, past the ones the client makes on its own: `getMe` at start,
    /// `getChat` to look a peer up, `getBusinessConnection` to look a business connection up, and `getUpdates`.
    async fn next(&mut self) -> Call {
        loop {
            let call = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .expect("no request in time")
                .expect("the stub stopped");
            if !matches!(call.0.as_str(), "getMe" | "getChat" | "getBusinessConnection" | "getUpdates") {
                return call;
            }
        }
    }

    /// The next request, which must be `method`; its body.
    async fn expect(&mut self, method: &str) -> Value {
        let (sent, body) = self.next().await;
        assert_eq!(sent, method, "body: {body}");
        body
    }
}

async fn serve(mut stream: TcpStream, calls: mpsc::UnboundedSender<Call>, pending: Pending) {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") { value.trim().parse::<usize>().ok() } else { None }
        })
        .unwrap_or(0);
    while data.len() < head_end + length {
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    let path = head.split_whitespace().nth(1).unwrap_or_default().to_owned();
    let prefix = format!("/bot{TOKEN}/");
    assert!(path.starts_with(&prefix), "request to {path}");
    let method = path[prefix.len()..].to_owned();
    let body = &data[head_end..head_end + length];
    let body = if length == 0 { json!({}) } else { serde_json::from_slice(body).unwrap() };
    let _ = calls.send((method.clone(), body.clone()));
    let reply = match answer(&method, &body, &pending).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(description) => {
            let code = if description.starts_with("Forbidden") { 403 } else { 400 };
            json!({ "ok": false, "error_code": code, "description": description })
        }
    }
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn answer(method: &str, body: &Value, pending: &Pending) -> Result<Value, &'static str> {
    let id = body["message_id"].as_i64().unwrap_or(70);
    Ok(match method {
        "getMe" => json!({ "id": BOT, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getChat" if body["chat_id"] == json!(UNKNOWN_USER) => return Err("Bad Request: chat not found"),
        "getChat" if body["chat_id"] == json!(BLOCKED_USER) => return Err("Forbidden: bot was blocked by the user"),
        "getChat" => chat(body["chat_id"].as_i64().unwrap_or(0)),
        "getUpdates" => {
            let first = pending.lock().unwrap().take();
            match first {
                Some(updates) => Value::Array(updates),
                None => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    json!([])
                }
            }
        }
        "forwardMessage" => message(80, body, json!({ "text": "forwarded" })),
        "getBusinessConnection" => json!({
            "id": body["business_connection_id"],
            "user": { "id": USER, "is_bot": false, "first_name": "Ann" },
            "user_chat_id": USER,
            "date": 1,
            "is_enabled": true,
        }),
        "sendMessage" | "editMessageText" => message(id, body, json!({ "text": body["text"] })),
        "editMessageReplyMarkup" => message(id, body, json!({ "text": "bye" })),
        "sendLocation" | "editMessageLiveLocation" => message(id, body, json!({ "location": location(body) })),
        "stopMessageLiveLocation" => message(id, body, json!({ "location": { "latitude": 1.0, "longitude": 2.0 } })),
        "sendVenue" => {
            let venue = json!({
                "location": location(body),
                "title": body["title"],
                "address": body["address"],
                "foursquare_id": body["foursquare_id"],
            });
            message(id, body, json!({ "location": location(body), "venue": venue }))
        }
        "sendContact" => {
            let contact = json!({ "phone_number": body["phone_number"], "first_name": body["first_name"] });
            message(id, body, json!({ "contact": contact }))
        }
        "sendDice" => message(id, body, json!({ "dice": { "emoji": body["emoji"], "value": 6 } })),
        "sendPoll" => message(id, body, json!({ "poll": poll(body, false) })),
        "stopPoll" => poll(&json!({ "question": "Lunch?", "options": [{ "text": "Pizza" }] }), true),
        "sendChecklist" | "editMessageChecklist" => {
            let checklist = json!({ "title": body["checklist"]["title"], "tasks": body["checklist"]["tasks"] });
            message(id, body, json!({ "checklist": checklist }))
        }
        "sendGame" => message(id, body, json!({ "game": game() })),
        "setGameScore" if body["disable_edit_message"] == json!(true) => json!(true),
        "setGameScore" => message(id, body, json!({ "game": game() })),
        "sendWidget" | "sendSharedWidget" => message(id, body, json!({ "caption": body["caption"] })),
        "sendRichMessage" => message(id, body, json!({})),
        "getGameHighScores" => {
            json!([{ "position": 1, "user": { "id": USER, "is_bot": false, "first_name": "Ann" }, "score": 50 }])
        }
        "getUserProfileAudios" => {
            let audio = json!({
                "file_id": DOCUMENT, "file_unique_id": "u", "duration": 30, "title": "Song", "performer": "Band",
            });
            json!({ "total_count": 1, "audios": [audio] })
        }
        "copyMessages" => {
            let ids = body["message_ids"].as_array().map_or(0, Vec::len) as i64;
            Value::Array((0..ids).map(|i| json!({ "message_id": 80 + i })).collect())
        }
        "editEphemeralMessageText" if body["ephemeral_message_id"] == json!(EPHEMERAL_MEDIA) => {
            return Err("Bad Request: there is no text in the message to edit");
        }
        _ => json!(true),
    })
}

/// A chat as `getChat` describes it: private for a positive id, a basic group above `-10^12`, else a supergroup.
fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann" })
    } else if id > -1_000_000_000_000 {
        json!({ "id": id, "type": "group", "title": "Group" })
    } else {
        json!({ "id": id, "type": "supergroup", "title": "Supergroup" })
    }
}

/// The bot's message `id` in the request's chat, holding `content`.
fn message(id: i64, body: &Value, content: Value) -> Value {
    let mut message = json!({
        "message_id": id,
        "date": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "chat": chat(body["chat_id"].as_i64().unwrap_or(0)),
        "from": { "id": BOT, "is_bot": true, "first_name": "Bot" },
    });
    message.as_object_mut().unwrap().extend(content.as_object().cloned().unwrap_or_default());
    message
}

fn location(body: &Value) -> Value {
    json!({ "latitude": body["latitude"], "longitude": body["longitude"], "live_period": body["live_period"] })
}

/// A poll as the Bot API describes it, made of the `question` and `options` of a `sendPoll` body.
fn poll(body: &Value, closed: bool) -> Value {
    let options: Vec<Value> = body["options"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, o)| json!({ "persistent_id": i.to_string(), "text": o["text"], "voter_count": 0 }))
        .collect();
    json!({
        "id": "123",
        "question": body["question"],
        "options": options,
        "total_voter_count": 0,
        "is_closed": closed,
        "is_anonymous": body["is_anonymous"].as_bool().unwrap_or(true),
        "type": body["type"].as_str().unwrap_or("regular"),
        "allows_multiple_answers": false,
        "allows_revoting": true,
        "members_only": false,
    })
}

fn game() -> Value {
    json!({ "title": "Tetris", "description": "Blocks", "photo": [] })
}

async fn client(url: &str) -> Client {
    Client::builder("bot_api_messages", 1, "hash").bot_api(TOKEN).bot_api_url(url).catch_up(true).start().await.unwrap()
}

#[tokio::test]
async fn sends_without_files_go_out_as_their_own_methods() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    // sendLocation, live
    let sent = client.send_location(USER, 51.5, -0.12).live_period(900).heading(90).await.unwrap();
    let location = sent.location().expect("the message lost its location");
    assert_eq!((sent.id, location.latitude, location.live_period), (70, 51.5, Some(900)));
    let body = stub.expect("sendLocation").await;
    let fields = (&body["chat_id"], &body["latitude"], &body["live_period"], &body["heading"]);
    assert_eq!(fields, (&json!(USER), &json!(51.5), &json!(900), &json!(90)));

    // sendVenue
    let sent = client.send_venue(USER, 1.5, 2.5, "Cafe", "Main St").foursquare_id("4sq").await.unwrap();
    let venue = sent.venue().expect("the message lost its venue");
    assert_eq!((venue.title.as_str(), venue.foursquare_id.as_deref()), ("Cafe", Some("4sq")));
    let body = stub.expect("sendVenue").await;
    let fields = (&body["title"], &body["address"], &body["foursquare_id"]);
    assert_eq!(fields, (&json!("Cafe"), &json!("Main St"), &json!("4sq")));

    // sendContact
    let sent = client.send_contact(USER, "+100", "Ann").await.unwrap();
    assert_eq!(sent.contact().map(|c| c.phone_number.as_str()), Some("+100"));
    let body = stub.expect("sendContact").await;
    assert_eq!((&body["phone_number"], &body["first_name"]), (&json!("+100"), &json!("Ann")));
    assert!(body.get("last_name").is_none() && body.get("vcard").is_none());

    // sendDice
    let sent = client.send_dice(USER).emoji("🎯").await.unwrap();
    assert_eq!(sent.dice().map(|d| (d.emoji.as_str(), d.value)), Some(("🎯", 6)));
    assert_eq!(stub.expect("sendDice").await["emoji"], json!("🎯"));

    // sendPoll, a quiz
    let sent = client.send_poll(USER, "Lunch?", ["Pizza", "Soup"]).quiz([1]).explanation("Soup").await.unwrap();
    let poll = sent.poll().expect("the message lost its poll");
    assert_eq!((poll.question.as_str(), poll.options.len()), ("Lunch?", 2));
    let body = stub.expect("sendPoll").await;
    assert_eq!(body["options"], json!([{ "text": "Pizza" }, { "text": "Soup" }]));
    let fields = (&body["type"], &body["correct_option_ids"], &body["explanation"], &body["allows_revoting"]);
    assert_eq!(fields, (&json!("quiz"), &json!([1]), &json!("Soup"), &json!(false)));

    // sendGame
    let sent = client.send_game(USER, "tetris").await.unwrap();
    assert_eq!(sent.game().map(|g| g.title.as_str()), Some("Tetris"));
    assert_eq!(stub.expect("sendGame").await["game_short_name"], json!("tetris"));

    // sendWidget
    let sent = client.send_widget(USER, "<p>x</p>").height(25).caption("Look").await.unwrap();
    assert_eq!(sent.text(), "Look");
    let body = stub.expect("sendWidget").await;
    assert_eq!((&body["html"], &body["height"], &body["caption"]), (&json!("<p>x</p>"), &json!(25), &json!("Look")));
    assert!(body.get("width").is_none());

    // sendSharedWidget
    client.send_shared_widget(USER, 42).chat_data("d").await.unwrap();
    let body = stub.expect("sendSharedWidget").await;
    assert_eq!((&body["widget_id"], &body["chat_data"]), (&json!("42"), &json!("d")));

    // sendRichMessage: the Bot API returns no text, so the message takes the rich message's.
    let sent = client.send_rich_message(USER, InputRichMessage::html("<b>hi</b>")).await.unwrap();
    assert_eq!(sent.text(), "hi");
    assert_eq!(stub.expect("sendRichMessage").await["rich_message"], json!({ "html": "<b>hi</b>" }));

    // sendMessageDraft
    client.send_message_draft(USER, 5, "partial").can_stop().await.unwrap();
    let body = stub.expect("sendMessageDraft").await;
    assert_eq!((&body["draft_id"], &body["text"], &body["can_stop"]), (&json!(5), &json!("partial"), &json!(true)));

    // sendRichMessageDraft
    client.send_rich_message_draft(USER, 5, InputRichMessage::markdown("**hi**")).await.unwrap();
    let body = stub.expect("sendRichMessageDraft").await;
    assert_eq!((&body["draft_id"], &body["rich_message"]), (&json!(5), &json!({ "markdown": "**hi**" })));

    // setMessageReaction
    client.send_reaction(USER, 50).reaction("👍").big().await.unwrap();
    let body = stub.expect("setMessageReaction").await;
    assert_eq!((&body["reaction"], &body["is_big"]), (&json!([{ "type": "emoji", "emoji": "👍" }]), &json!(true)));

    // copyMessages: the ids go in increasing order, and the copies come back in the request's.
    let copies = client.copy_messages(USER, USER, &[3, 2]).remove_caption().await.unwrap();
    assert_eq!(copies, [81, 80]);
    let body = stub.expect("copyMessages").await;
    assert_eq!((&body["message_ids"], &body["remove_caption"]), (&json!([2, 3]), &json!(true)));

    // An option with no Bot API counterpart fails before any request, naming what isn't carried.
    let error = client.send_location(USER, 1.0, 2.0).schedule_date(1_900_000_000).await.unwrap_err();
    assert_eq!(error.downcast_ref::<Unsupported>().map(Unsupported::what), Some("scheduled messages"));
}

#[tokio::test]
async fn edits_go_out_as_their_own_methods() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    // editMessageLiveLocation
    let edited = client.edit_message_live_location(USER, 70, 1.0, 2.0).live_period(600).await.unwrap();
    assert_eq!(edited.location().and_then(|l| l.live_period), Some(600));
    let body = stub.expect("editMessageLiveLocation").await;
    let fields = (&body["message_id"], &body["latitude"], &body["live_period"]);
    assert_eq!(fields, (&json!(70), &json!(1.0), &json!(600)));

    // stopMessageLiveLocation
    client.stop_message_live_location(USER, 70).await.unwrap();
    let body = stub.expect("stopMessageLiveLocation").await;
    assert_eq!((&body["chat_id"], &body["message_id"]), (&json!(USER), &json!(70)));
    assert!(body.get("latitude").is_none());

    // stopPoll: answered with the poll alone, which the edited message holds.
    let stopped = client.stop_poll(USER, 72).await.unwrap();
    assert_eq!((stopped.id, stopped.poll().map(|p| p.is_closed)), (72, Some(true)));
    assert_eq!(stub.expect("stopPoll").await["message_id"], json!(72));

    // editMessageChecklist, through a business connection
    let checklist = InputChecklist::new("Trip").task(1, "Tickets");
    let edited = client.edit_message_checklist(USER, 73, checklist).business_connection_id("conn").await.unwrap();
    assert_eq!(edited.checklist().map(|c| c.title.as_str()), Some("Trip"));
    let body = stub.expect("editMessageChecklist").await;
    assert_eq!(body["business_connection_id"], json!("conn"));
    assert_eq!(body["checklist"], json!({ "title": "Trip", "tasks": [{ "id": 1, "text": "Tickets" }] }));

    // editMessageWidgetData
    client.edit_message_widget_data(USER, 74).public_data("p").await.unwrap();
    let body = stub.expect("editMessageWidgetData").await;
    assert_eq!((&body["message_id"], &body["public_data"]), (&json!(74), &json!("p")));
    assert!(body.get("chat_data").is_none());

    // editSharedWidgetData
    client.edit_shared_widget_data(42, "data").cache_time(60).await.unwrap();
    let body = stub.expect("editSharedWidgetData").await;
    let fields = (&body["widget_id"], &body["public_data"], &body["cache_time"]);
    assert_eq!(fields, (&json!("42"), &json!("data"), &json!(60)));

    // editEphemeralMessageText
    let edited = client.edit_ephemeral_message_text(GROUP, USER, 5, "new").await.unwrap();
    assert_eq!((edited.id, edited.text.as_str(), edited.receiver_user.id, edited.chat.id), (5, "new", USER, GROUP));
    let body = stub.expect("editEphemeralMessageText").await;
    let fields = (&body["chat_id"], &body["receiver_user_id"], &body["ephemeral_message_id"], &body["text"]);
    assert_eq!(fields, (&json!(GROUP), &json!(USER), &json!(5), &json!("new")));

    // editEphemeralMessageCaption: a media message's text is its caption, which the text edit refuses.
    client.edit_ephemeral_message_caption(GROUP, USER, EPHEMERAL_MEDIA as i32, "cap").await.unwrap();
    stub.expect("editEphemeralMessageText").await;
    let body = stub.expect("editEphemeralMessageCaption").await;
    assert_eq!((&body["caption"], &body["ephemeral_message_id"]), (&json!("cap"), &json!(EPHEMERAL_MEDIA)));

    // editEphemeralMessageReplyMarkup
    client.edit_ephemeral_message_reply_markup(GROUP, USER, 7).await.unwrap();
    assert_eq!(stub.expect("editEphemeralMessageReplyMarkup").await["ephemeral_message_id"], json!(7));

    // editEphemeralMessageMedia, with a stored file sent by its file id
    let photo = InputMedia::photo(FileSource::file_id(PHOTO).unwrap()).caption("pic");
    let edited = client.edit_ephemeral_message_media(GROUP, USER, 8, photo).await.unwrap();
    assert_eq!(edited.text, "pic");
    let body = stub.expect("editEphemeralMessageMedia").await;
    assert_eq!(body["media"], json!({ "type": "photo", "media": PHOTO, "caption": "pic" }));

    // deleteEphemeralMessage
    client.delete_ephemeral_message(GROUP, USER, 9).await.unwrap();
    let body = stub.expect("deleteEphemeralMessage").await;
    assert_eq!((&body["receiver_user_id"], &body["ephemeral_message_id"]), (&json!(USER), &json!(9)));
}

#[tokio::test]
async fn the_rest_map_one_to_one() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    // approveSuggestedPost
    client.approve_suggested_post(CHANNEL, 5).send_date(1_900_000_000).await.unwrap();
    let body = stub.expect("approveSuggestedPost").await;
    assert_eq!((&body["chat_id"], &body["send_date"]), (&json!(CHANNEL), &json!(1_900_000_000)));

    // declineSuggestedPost
    client.decline_suggested_post(CHANNEL, 6).comment("no").await.unwrap();
    let body = stub.expect("declineSuggestedPost").await;
    assert_eq!((&body["message_id"], &body["comment"]), (&json!(6), &json!("no")));

    // deleteMessageReaction, of a user
    client.delete_message_reaction(GROUP, 50, USER).await.unwrap();
    let body = stub.expect("deleteMessageReaction").await;
    assert_eq!((&body["chat_id"], &body["message_id"], &body["user_id"]), (&json!(GROUP), &json!(50), &json!(USER)));

    // deleteAllMessageReactions, of a chat
    client.delete_all_message_reactions(GROUP, CHANNEL).await.unwrap();
    let body = stub.expect("deleteAllMessageReactions").await;
    assert_eq!((&body["actor_chat_id"], body.get("user_id")), (&json!(CHANNEL), None));

    // setGameScore, with the scoreboard edit and without
    let edited = client.set_game_score(USER, 75, USER, 50).force().await.unwrap();
    assert_eq!(edited.map(|m| m.id), Some(75));
    let body = stub.expect("setGameScore").await;
    assert_eq!((&body["user_id"], &body["score"], &body["force"]), (&json!(USER), &json!(50), &json!(true)));
    assert!(client.set_game_score(USER, 75, USER, 60).disable_edit_message().await.unwrap().is_none());
    assert_eq!(stub.expect("setGameScore").await["disable_edit_message"], json!(true));

    // getGameHighScores
    let scores = client.get_game_high_scores(USER, 75, USER).await.unwrap();
    assert_eq!(scores.iter().map(|s| (s.position, s.user.id, s.score)).collect::<Vec<_>>(), [(1, USER, 50)]);
    let body = stub.expect("getGameHighScores").await;
    assert_eq!((&body["user_id"], &body["message_id"]), (&json!(USER), &json!(75)));

    // getUserProfileAudios
    let audios = client.get_user_profile_audios(USER).limit(5).await.unwrap();
    assert_eq!(audios.iter().map(|a| a.title.as_deref()).collect::<Vec<_>>(), [Some("Song")]);
    let body = stub.expect("getUserProfileAudios").await;
    assert_eq!((&body["user_id"], &body["limit"]), (&json!(USER), &json!(5)));

    // setUserEmojiStatus
    client.set_user_emoji_status(USER).custom_emoji_id(5).until(9).await.unwrap();
    let body = stub.expect("setUserEmojiStatus").await;
    let fields = (&body["emoji_status_custom_emoji_id"], &body["emoji_status_expiration_date"]);
    assert_eq!(fields, (&json!("5"), &json!(9)));

    // sendChecklist: bots send checklists through a business connection only, so the request comes wrapped.
    let title = |text: &str| {
        let text = types::TextWithEntities { text: text.into(), entities: Vec::new() };
        base::TextWithEntities::TextWithEntities(Box::new(text))
    };
    let item = types::TodoItem { id: 1, title: title("Tickets") };
    let todo = types::TodoList {
        others_can_append: true,
        others_can_complete: false,
        title: title("Trip"),
        list: vec![base::TodoItem::TodoItem(Box::new(item))],
    };
    let media = types::InputMediaTodo { todo: base::TodoList::TodoList(Box::new(todo)) };
    let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 0 }));
    let send = functions::messages::SendMedia {
        silent: true,
        background: false,
        clear_draft: false,
        noforwards: false,
        update_stickersets_order: false,
        invert_media: false,
        allow_paid_floodskip: false,
        peer: user,
        reply_to: None,
        media: base::InputMedia::InputMediaTodo(Box::new(media)),
        message: String::new(),
        random_id: 1,
        reply_markup: None,
        entities: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        allow_paid_stars: None,
        suggested_post: None,
    };
    let wrapped = functions::InvokeWithBusinessConnection { connection_id: "conn".into(), query: send };
    client.invoke(&wrapped).await.unwrap();
    let body = stub.expect("sendChecklist").await;
    assert_eq!((&body["business_connection_id"], &body["disable_notification"]), (&json!("conn"), &json!(true)));
    let expected = json!({ "title": "Trip", "tasks": [{ "id": 1, "text": "Tickets" }], "others_can_add_tasks": true });
    assert_eq!(body["checklist"], expected);
}

#[tokio::test]
async fn text_sends_and_edits_through_a_business_connection_name_it() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    // sendMessage
    let sent = client.send_message(USER, "hi").business_connection_id("biz").await.unwrap();
    assert_eq!(sent.text(), "hi");
    let body = stub.expect("sendMessage").await;
    let fields = (&body["business_connection_id"], &body["chat_id"], &body["text"]);
    assert_eq!(fields, (&json!("biz"), &json!(USER), &json!("hi")));

    // editMessageText
    let edited = client.edit_message_text(USER, 70, "bye").business_connection_id("biz").await.unwrap();
    assert_eq!((edited.id, edited.text()), (70, "bye"));
    let body = stub.expect("editMessageText").await;
    assert_eq!((&body["business_connection_id"], &body["message_id"]), (&json!("biz"), &json!(70)));

    // editMessageReplyMarkup
    client.edit_message_reply_markup(USER, 70, None).business_connection_id("biz").await.unwrap();
    assert_eq!(stub.expect("editMessageReplyMarkup").await["business_connection_id"], json!("biz"));

    // A live location edit goes through the core's `messages.editMessage` too, connection and all.
    client.edit_message_live_location(USER, 71, 1.0, 2.0).business_connection_id("biz").await.unwrap();
    let body = stub.expect("editMessageLiveLocation").await;
    assert_eq!((&body["business_connection_id"], &body["message_id"]), (&json!("biz"), &json!(71)));

    // sendChatAction and pinChatMessage, which answer `true` and change no message
    client.send_chat_action(USER, ChatAction::Typing).business_connection_id("biz").await.unwrap();
    let body = stub.expect("sendChatAction").await;
    assert_eq!((&body["business_connection_id"], &body["action"]), (&json!("biz"), &json!("typing")));
    client.pin_chat_message(USER, 70).business_connection_id("biz").await.unwrap();
    let body = stub.expect("pinChatMessage").await;
    assert_eq!((&body["business_connection_id"], &body["message_id"]), (&json!("biz"), &json!(70)));

    // Without a connection, nothing names one.
    client.send_message(USER, "plain").await.unwrap();
    assert!(stub.expect("sendMessage").await.get("business_connection_id").is_none());
}

#[tokio::test]
async fn a_business_handler_answers_a_polled_business_message_through_its_connection() {
    let message = json!({
        "message_id": 10,
        "date": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "business_connection_id": "biz",
        "chat": { "id": USER, "type": "private", "first_name": "Ann" },
        "from": { "id": USER, "is_bot": false, "first_name": "Ann" },
        "text": "hi bot",
    });
    let mut stub = Stub::with_updates(vec![json!({ "update_id": 1, "business_message": message })]).await;
    let client = client(&stub.url).await;
    client.on_business_message(filters::all(), |m| async move { m.reply("hello back").await });
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    // The reply goes back through the connection the message came in.
    let body = stub.expect("sendMessage").await;
    let fields = (&body["business_connection_id"], &body["chat_id"], &body["text"]);
    assert_eq!(fields, (&json!("biz"), &json!(USER), &json!("hello back")));

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn paid_sends_fail_and_forwards_keep_their_effect() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;
    let what = |error: vaegram::Error| error.downcast_ref::<Unsupported>().map(Unsupported::what);

    // A text send or a forward the Bot API would make free fails instead.
    let error = client.send_message(USER, "pay").paid_message_star_count(5).await.unwrap_err();
    assert_eq!(what(error), Some("paid messages and suggested posts"));
    let error = client.forward_messages(USER, GROUP, &[3]).paid_message_star_count(5).await.unwrap_err();
    assert_eq!(what(error), Some("paid messages and suggested posts"));

    // forwardMessage, with its effect
    let forwarded = client.forward_messages(USER, GROUP, &[3]).message_effect_id(77).await.unwrap();
    assert_eq!(forwarded.len(), 1);
    let body = stub.expect("forwardMessage").await;
    let fields = (&body["from_chat_id"], &body["message_id"], &body["message_effect_id"]);
    assert_eq!(fields, (&json!(GROUP), &json!(3), &json!("77")));
}

#[tokio::test]
async fn replies_and_topic_sends_name_what_they_answer() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    // A reply with a quote
    let reply = ReplyParameters::new(5).quote("hi").quote_position(2);
    client.send_message(GROUP, "re").reply_to(reply).await.unwrap();
    let body = stub.expect("sendMessage").await;
    assert_eq!(body["reply_parameters"], json!({ "message_id": 5, "quote": "hi", "quote_position": 2 }));
    assert!(body.get("message_thread_id").is_none());

    // A send to a topic is MTProto's reply to the topic's first message, which files it in the topic.
    client.send_message(CHANNEL, "in topic").message_thread_id(9).await.unwrap();
    let body = stub.expect("sendMessage").await;
    assert_eq!(body["reply_parameters"], json!({ "message_id": 9 }));
    assert!(body.get("message_thread_id").is_none());

    // A reply to another message of the topic names both.
    client.send_message(CHANNEL, "re in topic").message_thread_id(9).reply_to(12).await.unwrap();
    let body = stub.expect("sendMessage").await;
    assert_eq!((&body["reply_parameters"], &body["message_thread_id"]), (&json!({ "message_id": 12 }), &json!(9)));
}

#[tokio::test]
async fn only_a_chat_telegram_doesnt_know_is_left_out() {
    let stub = Stub::start().await;
    let client = client(&stub.url).await;
    let users = client.get_users([UNKNOWN_USER]).await.unwrap();
    assert!(users[0].is_none());
    // A refusal isn't an unknown chat: the caller learns why.
    let error = client.get_users([BLOCKED_USER]).await.unwrap_err();
    assert_eq!(error.rpc().map(|e| e.code), Some(403));
}
