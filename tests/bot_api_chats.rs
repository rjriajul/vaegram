//! Chat methods of a client in Bot API mode against a local HTTP stub: members and admins, invite links and join
//! requests, chat settings and lookups, and forum topics. Each row checks the Bot API method a friendly method
//! sends, the fields that carry its arguments, and what the friendly method reads back.
#![cfg(feature = "bot-api")]

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::Client;
use vaegram::bot_api::Unsupported;
use vaegram::enums::{ChatJoinRequestResult, ChatMemberStatus, ChatMembersFilter};
use vaegram::types::{ChatPermissions, ChatPrivileges, Upload};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;
/// A user `getChatMember` answers as banned, and one it answers as restricted.
const BANNED: i64 = 8;
const RESTRICTED: i64 = 9;
/// The bot administrator `getChatAdministrators` lists besides the owner and `USER`.
const OTHER_BOT: i64 = 55;
const OWNER: i64 = 1;
/// A basic group and a supergroup, Bot API style.
const GROUP: i64 = -42;
const CHANNEL: i64 = -1_000_000_000_123;
/// A channel that posts in `CHANNEL` as itself.
const SENDER_CHAT: i64 = -1_000_000_000_456;
const LINK: &str = "https://t.me/+abc";
const TOPIC: i64 = 77;
const EMOJI: i64 = 5_312_536_423_851_630_001;
/// File ids as the Bot API gives them out.
const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";

/// A file part of a multipart request: its field, file name and bytes.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    field: String,
    filename: String,
    data: Vec<u8>,
}

/// One request the stub got: the Bot API method, its fields and its file parts. A multipart request's fields are
/// strings, as they went.
#[derive(Debug)]
struct Call {
    method: String,
    body: Value,
    files: Vec<Part>,
}

struct Stub {
    url: String,
    calls: mpsc::UnboundedReceiver<Call>,
}

impl Stub {
    async fn start() -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, calls) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone()));
            }
        });
        Stub { url, calls }
    }

    /// The next request a friendly method made, past the ones the client makes on its own: `getMe` at start,
    /// `getChat` to look a peer up, and `getUpdates`.
    async fn next(&mut self) -> Call {
        loop {
            let call = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .expect("no such request in time")
                .expect("the stub stopped");
            if !matches!(call.method.as_str(), "getMe" | "getChat" | "getUpdates") {
                return call;
            }
        }
    }

    /// The next request, which must be `method`; its fields.
    async fn expect(&mut self, method: &str) -> Value {
        let call = self.next().await;
        assert_eq!(call.method, method, "fields: {}", call.body);
        call.body
    }

    /// No request a friendly method made is waiting.
    fn assert_idle(&mut self) {
        while let Ok(call) = self.calls.try_recv() {
            let own = matches!(call.method.as_str(), "getMe" | "getChat" | "getUpdates");
            assert!(own, "unexpected {} {}", call.method, call.body);
        }
    }
}

async fn serve(mut stream: TcpStream, calls: mpsc::UnboundedSender<Call>) {
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
    let header = |wanted: &str| {
        head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(wanted).then(|| value.trim().to_owned())
        })
    };
    let length = header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let content_type = header("content-type").unwrap_or_default();
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
    let raw = &data[head_end..head_end + length];
    let (body, files) = match content_type.split_once("boundary=") {
        Some((_, boundary)) => multipart(raw, boundary),
        None if length == 0 => (json!({}), Vec::new()),
        None => (serde_json::from_slice(raw).unwrap(), Vec::new()),
    };
    let _ = calls.send(Call { method: method.clone(), body: body.clone(), files });
    let reply = match answer(&method, &body).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(description) => json!({ "ok": false, "error_code": 400, "description": description }),
    }
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// The text fields of a `multipart/form-data` body as a JSON object of strings, and its file parts.
fn multipart(body: &[u8], boundary: &str) -> (Value, Vec<Part>) {
    let delimiter = format!("--{}", boundary.trim_matches('"'));
    let mut fields = serde_json::Map::new();
    let mut files = Vec::new();
    for part in split(body, delimiter.as_bytes()).into_iter().skip(1) {
        // The last delimiter ends with `--`.
        if part.starts_with(b"--") {
            break;
        }
        let part = part.strip_prefix(b"\r\n").unwrap_or(part);
        let part = part.strip_suffix(b"\r\n").unwrap_or(part);
        let end = part.windows(4).position(|w| w == b"\r\n\r\n").expect("a part without headers");
        let head = String::from_utf8_lossy(&part[..end]).into_owned();
        let content = &part[end + 4..];
        let name = parameter(&head, "name").expect("a part without a name");
        match parameter(&head, "filename") {
            Some(filename) => files.push(Part { field: name, filename, data: content.to_vec() }),
            None => {
                fields.insert(name, Value::String(String::from_utf8_lossy(content).into_owned()));
            }
        }
    }
    (Value::Object(fields), files)
}

fn split<'a>(data: &'a [u8], separator: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = Vec::new();
    let mut rest = data;
    while let Some(i) = rest.windows(separator.len()).position(|w| w == separator) {
        parts.push(&rest[..i]);
        rest = &rest[i + separator.len()..];
    }
    parts.push(rest);
    parts
}

/// `key="value"` of a part's `Content-Disposition`.
fn parameter(head: &str, key: &str) -> Option<String> {
    let marker = format!("; {key}=\"");
    let start = head.find(&marker)? + marker.len();
    let end = head[start..].find('"')? + start;
    Some(head[start..end].to_owned())
}

async fn answer(method: &str, body: &Value) -> Result<Value, &'static str> {
    Ok(match method {
        "getMe" => user(BOT, true),
        "getChat" => chat(body["chat_id"].as_i64().unwrap_or(0)),
        "getUpdates" => {
            tokio::time::sleep(Duration::from_millis(100)).await;
            json!([])
        }
        "getChatMember" => member(body["user_id"].as_i64().unwrap_or(0)),
        "getChatAdministrators" => json!([
            { "status": "creator", "user": user(OWNER, false), "is_anonymous": false },
            member(USER),
            { "status": "administrator", "user": user(OTHER_BOT, true), "can_be_edited": false, "is_anonymous": false,
              "can_manage_chat": true },
        ]),
        "getChatMemberCount" => json!(42),
        "exportChatInviteLink" => json!("https://t.me/+primary"),
        "createChatInviteLink" | "createChatSubscriptionInviteLink" => invite_link("https://t.me/+new", body, false),
        "editChatInviteLink" | "editChatSubscriptionInviteLink" => invite_link(LINK, body, false),
        "revokeChatInviteLink" => invite_link(LINK, body, true),
        "createForumTopic" => json!({
            "message_thread_id": TOPIC,
            "name": body["name"],
            "icon_color": body["icon_color"].as_i64().unwrap_or(0x6FB9F0),
            "icon_custom_emoji_id": body["icon_custom_emoji_id"],
        }),
        "getForumTopicIconStickers" => json!([{
            "file_id": DOCUMENT, "file_unique_id": "s", "type": "custom_emoji", "width": 512, "height": 512,
            "is_animated": true, "is_video": false, "emoji": "N", "custom_emoji_id": EMOJI.to_string(),
        }]),
        "getUserProfilePhotos" => json!({
            "total_count": 3,
            "photos": [[{ "file_id": PHOTO, "file_unique_id": "p", "width": 640, "height": 640, "file_size": 50 }]],
        }),
        "getUserChatBoosts" => json!({ "boosts": [{
            "boost_id": "b1", "add_date": 100, "expiration_date": 200,
            "source": { "source": "premium", "user": user(USER, false) },
        }]}),
        "getChatShareThemeBackdrops" => json!({ "backdrops": [{
            "id": 5, "name": "Sky",
            "colors": { "center_color": 1, "edge_color": 2, "symbol_color": 3, "text_color": 4 },
        }]}),
        "getUserPersonalChatMessages" => json!([{
            "message_id": 90,
            "date": 1_700_000_000,
            "chat": { "id": -1_000_000_000_999_i64, "type": "channel", "title": "Blog" },
            "text": "post",
        }]),
        _ => json!(true),
    })
}

fn user(id: i64, is_bot: bool) -> Value {
    let first_name = if is_bot { "Bot" } else { "Ann" };
    json!({ "id": id, "is_bot": is_bot, "first_name": first_name })
}

/// A chat as `getChat` describes it: private for a positive id, a basic group above `-10^12`, else a supergroup.
fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann" })
    } else if id > -1_000_000_000_000 {
        json!({ "id": id, "type": "group", "title": "Group" })
    } else {
        json!({ "id": id, "type": "supergroup", "title": "Supergroup", "is_forum": true })
    }
}

/// `USER` is an administrator titled "boss", `BANNED` is banned, `RESTRICTED` may only send text, anyone else is
/// a member tagged "vip".
fn member(user_id: i64) -> Value {
    match user_id {
        USER => json!({
            "status": "administrator", "user": user(USER, false), "can_be_edited": true, "is_anonymous": false,
            "can_manage_chat": true, "can_delete_messages": true, "custom_title": "boss",
        }),
        BANNED => json!({ "status": "kicked", "user": user(BANNED, false), "until_date": 0 }),
        RESTRICTED => json!({
            "status": "restricted", "user": user(RESTRICTED, false), "is_member": true, "can_send_messages": true,
            "until_date": 2_000_000_000,
        }),
        id => json!({ "status": "member", "user": user(id, false), "tag": "vip" }),
    }
}

/// The link the request names, or `fresh` for a new one, with the options it set.
fn invite_link(fresh: &str, body: &Value, revoked: bool) -> Value {
    json!({
        "invite_link": body["invite_link"].as_str().unwrap_or(fresh),
        "creator": user(BOT, true),
        "creates_join_request": body["creates_join_request"].as_bool().unwrap_or(false),
        "is_primary": false,
        "is_revoked": revoked,
        "name": body["name"],
        "expire_date": body["expire_date"],
        "member_limit": body["member_limit"],
        "subscription_period": body["subscription_period"],
        "subscription_price": body["subscription_price"],
    })
}

async fn client(url: &str) -> Client {
    Client::builder("bot_api_chats", 1, "hash").bot_api(TOKEN).bot_api_url(url).catch_up(true).start().await.unwrap()
}

fn unsupported(error: &vaegram::Error) -> Option<&'static str> {
    error.downcast_ref::<Unsupported>().map(Unsupported::what)
}

#[tokio::test]
async fn members_and_administrators() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    client.ban_chat_member(CHANNEL, USER).until_date(2_000_000_000).await.unwrap();
    let body = stub.expect("banChatMember").await;
    let fields = (&body["chat_id"], &body["user_id"], &body["until_date"]);
    assert_eq!(fields, (&json!(CHANNEL), &json!(USER), &json!(2_000_000_000)));

    // A basic group removes the user; the bot removing itself leaves.
    client.ban_chat_member(GROUP, USER).revoke_messages(true).await.unwrap();
    let body = stub.expect("banChatMember").await;
    assert_eq!((&body["chat_id"], &body["revoke_messages"]), (&json!(GROUP), &json!(true)));
    client.leave_chat(GROUP).await.unwrap();
    assert_eq!(stub.expect("leaveChat").await["chat_id"], json!(GROUP));
    client.leave_chat(CHANNEL).await.unwrap();
    assert_eq!(stub.expect("leaveChat").await["chat_id"], json!(CHANNEL));

    client.ban_chat_sender_chat(CHANNEL, SENDER_CHAT).await.unwrap();
    let body = stub.expect("banChatSenderChat").await;
    assert_eq!((&body["chat_id"], &body["sender_chat_id"]), (&json!(CHANNEL), &json!(SENDER_CHAT)));
    client.unban_chat_sender_chat(CHANNEL, SENDER_CHAT).await.unwrap();
    assert_eq!(stub.expect("unbanChatSenderChat").await["sender_chat_id"], json!(SENDER_CHAT));

    // An unban never removes a member: a banned user is unbanned only if banned, a restricted one gets every
    // permission back, and a plain member is left alone.
    client.unban_chat_member(CHANNEL, BANNED).await.unwrap();
    assert_eq!(stub.expect("getChatMember").await["user_id"], json!(BANNED));
    let body = stub.expect("unbanChatMember").await;
    assert_eq!((&body["user_id"], &body["only_if_banned"]), (&json!(BANNED), &json!(true)));
    client.unban_chat_member(CHANNEL, RESTRICTED).await.unwrap();
    stub.expect("getChatMember").await;
    let body = stub.expect("restrictChatMember").await;
    assert_eq!(body["user_id"], json!(RESTRICTED));
    assert!(body["permissions"].as_object().unwrap().values().all(|allowed| allowed == &json!(true)));
    client.unban_chat_member(CHANNEL, 10).await.unwrap();
    stub.expect("getChatMember").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    stub.assert_idle();

    let permissions = ChatPermissions::new().can_send_messages(true);
    client.restrict_chat_member(CHANNEL, USER, permissions).until_date(2_000_000_000).await.unwrap();
    let body = stub.expect("restrictChatMember").await;
    assert_eq!((&body["user_id"], &body["until_date"]), (&json!(USER), &json!(2_000_000_000)));
    let allowed = (&body["permissions"]["can_send_messages"], &body["permissions"]["can_send_photos"]);
    assert_eq!(allowed, (&json!(true), &json!(false)));
    assert_eq!(body["use_independent_chat_permissions"], json!(true));

    client.set_chat_permissions(GROUP, ChatPermissions::ALL).await.unwrap();
    let body = stub.expect("setChatPermissions").await;
    assert_eq!(body["chat_id"], json!(GROUP));
    assert!(body["permissions"].as_object().unwrap().values().all(|allowed| allowed == &json!(true)));

    // The title goes apart from the rights.
    let privileges = ChatPrivileges::new().can_delete_messages(true);
    client.promote_chat_member(CHANNEL, USER, privileges).title("boss").await.unwrap();
    let body = stub.expect("promoteChatMember").await;
    let rights = (&body["user_id"], &body["can_delete_messages"], &body["can_manage_chat"]);
    assert_eq!(rights, (&json!(USER), &json!(true), &json!(false)));
    let body = stub.expect("setChatAdministratorCustomTitle").await;
    assert_eq!((&body["user_id"], &body["custom_title"]), (&json!(USER), &json!("boss")));
    // A basic group's admin has no separate rights.
    client.promote_chat_member(GROUP, USER, privileges).await.unwrap();
    let body = stub.expect("promoteChatMember").await;
    assert_eq!((&body["chat_id"], &body["can_manage_chat"]), (&json!(GROUP), &json!(true)));

    // The title alone, without reading and resending the rights.
    client.set_chat_administrator_custom_title(CHANNEL, USER, "chief").await.unwrap();
    let body = stub.expect("setChatAdministratorCustomTitle").await;
    assert_eq!((&body["chat_id"], &body["custom_title"]), (&json!(CHANNEL), &json!("chief")));

    client.set_chat_member_tag(CHANNEL, USER, "vip").await.unwrap();
    let body = stub.expect("setChatMemberTag").await;
    assert_eq!((&body["user_id"], &body["tag"]), (&json!(USER), &json!("vip")));

    let admin = client.get_chat_member(CHANNEL, USER).await.unwrap();
    assert_eq!((admin.user.id, admin.status), (USER, ChatMemberStatus::Administrator));
    assert_eq!((admin.custom_title.as_deref(), admin.can_be_edited), (Some("boss"), true));
    assert!(admin.privileges.is_some_and(|p| p.can_delete_messages && !p.can_promote_members));
    // The Bot API names no promoter: none, not a user with id 0.
    assert_eq!((admin.promoted_by.as_ref(), admin.invited_by.as_ref()), (None, None));
    stub.expect("getChatMember").await;
    // A basic group's member is read the same way, where TL would list the whole group.
    let member = client.get_chat_member(GROUP, 10).await.unwrap();
    let read = (member.user.id, member.status, member.custom_title.as_deref());
    assert_eq!(read, (10, ChatMemberStatus::Member, Some("vip")));
    assert_eq!(stub.expect("getChatMember").await["chat_id"], json!(GROUP));
    let banned = client.get_chat_member(CHANNEL, BANNED).await.unwrap();
    assert_eq!((banned.status, banned.is_member), (ChatMemberStatus::Banned, false));
    assert!(banned.restricted_by.is_none());
    stub.expect("getChatMember").await;
    let restricted = client.get_chat_member(CHANNEL, RESTRICTED).await.unwrap();
    assert_eq!((restricted.status, restricted.until_date), (ChatMemberStatus::Restricted, Some(2_000_000_000)));
    assert!(restricted.permissions.is_some_and(|p| p.can_send_messages && !p.can_send_photos));
    assert!(restricted.restricted_by.is_none());
    stub.expect("getChatMember").await;

    // Other bots are left out unless asked for; the owner and the admins are kept.
    let admins = client.get_chat_administrators(GROUP).await.unwrap();
    let ids: Vec<_> = admins.iter().map(|m| (m.user.id, m.status)).collect();
    assert_eq!(ids, [(OWNER, ChatMemberStatus::Owner), (USER, ChatMemberStatus::Administrator)]);
    let body = stub.expect("getChatAdministrators").await;
    assert_eq!((&body["chat_id"], &body["return_bots"]), (&json!(GROUP), &json!(true)));
    let admins = client.get_chat_administrators(CHANNEL).return_bots().await.unwrap();
    assert_eq!(admins.iter().map(|m| m.user.id).collect::<Vec<_>>(), [OWNER, USER, OTHER_BOT]);
    stub.expect("getChatAdministrators").await;

    // A channel's administrators through the member list; the Bot API lists no one else.
    let admins = client.get_chat_members(CHANNEL).filter(ChatMembersFilter::Administrators).await.unwrap();
    assert_eq!(admins.len(), 3);
    stub.expect("getChatAdministrators").await;
    let error = client.get_chat_members(CHANNEL).await.unwrap_err();
    assert_eq!(unsupported(&error), Some("listing members other than the administrators"));
    let error = client.get_chat_members(GROUP).await.unwrap_err();
    assert_eq!(unsupported(&error), Some("listing a basic group's members"));

    assert_eq!(client.get_chat_members_count(GROUP).await.unwrap(), 42);
    assert_eq!(stub.expect("getChatMemberCount").await["chat_id"], json!(GROUP));
}

#[tokio::test]
async fn invite_links_and_join_requests() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let link = client.export_chat_invite_link(CHANNEL).await.unwrap();
    assert_eq!(link, "https://t.me/+primary");
    assert_eq!(stub.expect("exportChatInviteLink").await["chat_id"], json!(CHANNEL));

    let link = client.create_chat_invite_link(CHANNEL).name("team").member_limit(5).await.unwrap();
    let read = (link.invite_link.as_str(), link.name.as_deref(), link.member_limit);
    assert_eq!(read, ("https://t.me/+new", Some("team"), Some(5)));
    assert_eq!(link.creator.id, BOT);
    let body = stub.expect("createChatInviteLink").await;
    assert_eq!((&body["chat_id"], &body["name"], &body["member_limit"]), (&json!(CHANNEL), &json!("team"), &json!(5)));

    let link = client.create_chat_invite_link(CHANNEL).creates_join_request().await.unwrap();
    assert!(link.creates_join_request);
    assert_eq!(stub.expect("createChatInviteLink").await["creates_join_request"], json!(true));

    let link = client.edit_chat_invite_link(CHANNEL, LINK).expire_date(2_000_000_000).await.unwrap();
    assert_eq!((link.invite_link.as_str(), link.expire_date), (LINK, Some(2_000_000_000)));
    let body = stub.expect("editChatInviteLink").await;
    assert_eq!((&body["invite_link"], &body["expire_date"]), (&json!(LINK), &json!(2_000_000_000)));

    let link = client.create_chat_subscription_invite_link(CHANNEL, 2_592_000, 50).await.unwrap();
    assert_eq!((link.subscription_period, link.subscription_price), (Some(2_592_000), Some(50)));
    let body = stub.expect("createChatSubscriptionInviteLink").await;
    let fields = (&body["subscription_period"], &body["subscription_price"]);
    assert_eq!(fields, (&json!(2_592_000), &json!(50)));

    // A subscription link is renamed with its own method, which TL doesn't tell apart from any other edit.
    let link = client.edit_chat_subscription_invite_link(CHANNEL, LINK, "renamed").await.unwrap();
    assert_eq!(link.name.as_deref(), Some("renamed"));
    let body = stub.expect("editChatSubscriptionInviteLink").await;
    assert_eq!((&body["invite_link"], &body["name"]), (&json!(LINK), &json!("renamed")));

    let link = client.revoke_chat_invite_link(CHANNEL, LINK).await.unwrap();
    assert!(link.is_revoked);
    assert_eq!(stub.expect("revokeChatInviteLink").await["invite_link"], json!(LINK));

    client.approve_chat_join_request(CHANNEL, USER).await.unwrap();
    let body = stub.expect("approveChatJoinRequest").await;
    assert_eq!((&body["chat_id"], &body["user_id"]), (&json!(CHANNEL), &json!(USER)));
    client.decline_chat_join_request(CHANNEL, USER).await.unwrap();
    assert_eq!(stub.expect("declineChatJoinRequest").await["user_id"], json!(USER));

    client.answer_chat_join_request_query(5, ChatJoinRequestResult::Queue).await.unwrap();
    let body = stub.expect("answerChatJoinRequestQuery").await;
    assert_eq!((&body["chat_join_request_query_id"], &body["result"]), (&json!("5"), &json!("queue")));
    client.send_chat_join_request_web_app(5, "https://example.com/app").await.unwrap();
    let body = stub.expect("sendChatJoinRequestWebApp").await;
    let fields = (&body["chat_join_request_query_id"], &body["web_app_url"]);
    assert_eq!(fields, (&json!("5"), &json!("https://example.com/app")));
}

#[tokio::test]
async fn chat_settings_and_lookups() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    client.set_chat_title(GROUP, "Team").await.unwrap();
    let body = stub.expect("setChatTitle").await;
    assert_eq!((&body["chat_id"], &body["title"]), (&json!(GROUP), &json!("Team")));
    client.set_chat_title(CHANNEL, "News").await.unwrap();
    assert_eq!(stub.expect("setChatTitle").await["chat_id"], json!(CHANNEL));

    client.set_chat_description(CHANNEL, "about us").await.unwrap();
    assert_eq!(stub.expect("setChatDescription").await["description"], json!("about us"));

    // The photo goes up in the request itself, as a file part.
    client.set_chat_photo(CHANNEL, Upload::bytes("photo.jpg", b"jpeg bytes")).await.unwrap();
    let call = stub.next().await;
    assert_eq!((call.method.as_str(), call.body["chat_id"].as_str()), ("setChatPhoto", Some("-1000000000123")));
    let part = &call.files[..];
    assert_eq!(part.len(), 1);
    let read = (part[0].field.as_str(), part[0].filename.as_str(), &part[0].data[..]);
    assert_eq!(read, ("photo", "photo.jpg", &b"jpeg bytes"[..]));
    // The Bot API sets still photos only; nothing is sent.
    let error = client.set_chat_photo(CHANNEL, Upload::bytes("clip.mp4", b"mp4")).video().await.unwrap_err();
    assert_eq!(unsupported(&error), Some("video chat photos"));
    client.delete_chat_photo(GROUP).await.unwrap();
    assert_eq!(stub.expect("deleteChatPhoto").await["chat_id"], json!(GROUP));

    client.set_chat_sticker_set(CHANNEL, "pack").await.unwrap();
    assert_eq!(stub.expect("setChatStickerSet").await["sticker_set_name"], json!("pack"));
    client.delete_chat_sticker_set(CHANNEL).await.unwrap();
    assert_eq!(stub.expect("deleteChatStickerSet").await["chat_id"], json!(CHANNEL));

    // A user is verified with `verifyUser`, any other chat with `verifyChat`.
    client.verify_user(USER).custom_description("Staff").await.unwrap();
    let body = stub.expect("verifyUser").await;
    assert_eq!((&body["user_id"], &body["custom_description"]), (&json!(USER), &json!("Staff")));
    client.verify_chat(CHANNEL).await.unwrap();
    let body = stub.expect("verifyChat").await;
    assert_eq!((&body["chat_id"], body.get("custom_description")), (&json!(CHANNEL), None));
    client.remove_user_verification(USER).await.unwrap();
    assert_eq!(stub.expect("removeUserVerification").await["user_id"], json!(USER));
    client.remove_chat_verification(CHANNEL).await.unwrap();
    assert_eq!(stub.expect("removeChatVerification").await["chat_id"], json!(CHANNEL));

    let boosts = client.get_user_chat_boosts(CHANNEL, USER).await.unwrap();
    assert_eq!(boosts.len(), 1);
    assert_eq!((boosts[0].boost_id.as_str(), boosts[0].add_date, boosts[0].expiration_date), ("b1", 100, 200));
    assert_eq!(boosts[0].source.user().map(|u| u.id), Some(USER));
    let body = stub.expect("getUserChatBoosts").await;
    assert_eq!((&body["chat_id"], &body["user_id"]), (&json!(CHANNEL), &json!(USER)));

    let photos = client.get_chat_photos(USER).limit(1).await.unwrap();
    assert_eq!(photos.iter().map(|p| (p.width, p.height)).collect::<Vec<_>>(), [(640, 640)]);
    let body = stub.expect("getUserProfilePhotos").await;
    assert_eq!((&body["user_id"], &body["offset"], &body["limit"]), (&json!(USER), &json!(0), &json!(1)));

    let messages = client.get_user_personal_chat_messages(USER, 5).await.unwrap();
    assert_eq!(messages.iter().map(|m| (m.id, m.text())).collect::<Vec<_>>(), [(90, "post")]);
    let body = stub.expect("getUserPersonalChatMessages").await;
    assert_eq!((&body["user_id"], &body["limit"]), (&json!(USER), &json!(5)));

    let backdrops = client.get_share_theme_backdrops().await.unwrap();
    assert_eq!(backdrops.len(), 1);
    let sky = &backdrops[0];
    let colors = (sky.center_color, sky.edge_color, sky.pattern_color, sky.text_color);
    assert_eq!((sky.id, sky.name.as_str(), colors), (5, "Sky", (1, 2, 3, 4)));
    stub.expect("getChatShareThemeBackdrops").await;
}

#[tokio::test]
async fn forum_topics() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let topic = client.create_forum_topic(CHANNEL, "News").icon_color(0xFFD67E).icon_emoji_id(EMOJI).await.unwrap();
    let read = (topic.id, topic.title.as_str(), topic.icon_color, topic.icon_emoji_id);
    assert_eq!(read, (77, "News", 0xFFD67E, Some(EMOJI)));
    let body = stub.expect("createForumTopic").await;
    let fields = (&body["chat_id"], &body["name"], &body["icon_color"], &body["icon_custom_emoji_id"]);
    assert_eq!(fields, (&json!(CHANNEL), &json!("News"), &json!(0xFFD67E), &json!(EMOJI.to_string())));

    // TL's icon id 0 removes the icon, as the Bot API's empty string does.
    client.edit_forum_topic(CHANNEL, 77).title("Updates").icon_emoji_id(0).await.unwrap();
    let body = stub.expect("editForumTopic").await;
    let fields = (&body["message_thread_id"], &body["name"], &body["icon_custom_emoji_id"]);
    assert_eq!(fields, (&json!(TOPIC), &json!("Updates"), &json!("")));

    client.close_forum_topic(CHANNEL, 77).await.unwrap();
    assert_eq!(stub.expect("closeForumTopic").await["message_thread_id"], json!(TOPIC));
    client.reopen_forum_topic(CHANNEL, 77).await.unwrap();
    assert_eq!(stub.expect("reopenForumTopic").await["message_thread_id"], json!(TOPIC));
    client.delete_forum_topic(CHANNEL, 77).await.unwrap();
    let body = stub.expect("deleteForumTopic").await;
    assert_eq!((&body["chat_id"], &body["message_thread_id"]), (&json!(CHANNEL), &json!(TOPIC)));

    // The General topic has methods of its own.
    client.edit_general_forum_topic(CHANNEL, "Lobby").await.unwrap();
    assert_eq!(stub.expect("editGeneralForumTopic").await["name"], json!("Lobby"));
    client.close_general_forum_topic(CHANNEL).await.unwrap();
    assert_eq!(stub.expect("closeGeneralForumTopic").await["chat_id"], json!(CHANNEL));
    client.reopen_general_forum_topic(CHANNEL).await.unwrap();
    stub.expect("reopenGeneralForumTopic").await;
    client.hide_general_forum_topic(CHANNEL).await.unwrap();
    stub.expect("hideGeneralForumTopic").await;
    client.unhide_general_forum_topic(CHANNEL).await.unwrap();
    stub.expect("unhideGeneralForumTopic").await;
    client.unpin_all_general_forum_topic_messages(CHANNEL).await.unwrap();
    assert_eq!(stub.expect("unpinAllGeneralForumTopicMessages").await["chat_id"], json!(CHANNEL));

    // A topic icon's document id is its custom emoji id.
    let icons = client.get_forum_topic_icon_stickers().await.unwrap();
    assert_eq!(icons.iter().map(|d| d.id).collect::<Vec<_>>(), [EMOJI]);
    stub.expect("getForumTopicIconStickers").await;
}
