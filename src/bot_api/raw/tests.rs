use serde_json::json;

use super::ChatId;
use super::methods::SendMessage;
use super::types::{MaybeInaccessibleMessage, MessageOrigin, Update};

#[test]
fn get_updates_result_with_entities_reply_and_inaccessible_message() {
    let result = json!([
        {
            "update_id": 870001,
            "message": {
                "message_id": 77,
                "from": {"id": 1001, "is_bot": false, "first_name": "Ada", "username": "ada", "language_code": "en"},
                "chat": {"id": 1001, "first_name": "Ada", "username": "ada", "type": "private"},
                "date": 1795000000,
                "reply_to_message": {
                    "message_id": 76,
                    "from": {"id": 42, "is_bot": true, "first_name": "Echo", "username": "echo_bot"},
                    "chat": {"id": 1001, "first_name": "Ada", "username": "ada", "type": "private"},
                    "date": 1794999990,
                    "text": "Send /start"
                },
                "text": "/start hello",
                "entities": [
                    {"offset": 0, "length": 6, "type": "bot_command"},
                    {"offset": 7, "length": 5, "type": "text_link", "url": "https://example.com"}
                ]
            }
        },
        {
            "update_id": 870002,
            "callback_query": {
                "id": "4382",
                "from": {"id": 1001, "is_bot": false, "first_name": "Ada"},
                "message": {"chat": {"id": -1002, "title": "Group", "type": "supergroup"}, "message_id": 5, "date": 0},
                "chat_instance": "-81",
                "data": "ok"
            }
        }
    ]);

    let updates: Vec<Update> = serde_json::from_value(result).unwrap();
    assert_eq!(updates.len(), 2);

    let message = updates[0].message.as_deref().unwrap();
    assert_eq!((message.message_id, message.chat.id, message.chat.r#type.as_str()), (77, 1001, "private"));
    assert_eq!(message.from.as_ref().unwrap().first_name, "Ada");
    assert_eq!(message.text.as_deref(), Some("/start hello"));
    let entities = message.entities.as_ref().unwrap();
    assert_eq!((entities[0].r#type.as_str(), entities[0].offset, entities[0].length), ("bot_command", 0, 6));
    assert_eq!(entities[1].url.as_deref(), Some("https://example.com"));
    let reply = message.reply_to_message.as_deref().unwrap();
    assert_eq!((reply.message_id, reply.text.as_deref()), (76, Some("Send /start")));
    assert!(reply.from.as_ref().unwrap().is_bot);

    let query = updates[1].callback_query.as_ref().unwrap();
    match query.message.as_ref().unwrap() {
        MaybeInaccessibleMessage::InaccessibleMessage(m) => assert_eq!((m.chat.id, m.message_id), (-1002, 5)),
        other => panic!("a message dated 0 is inaccessible, got {other:?}"),
    }
}

#[test]
fn message_origin_of_each_kind() {
    let user = json!({"id": 1, "is_bot": false, "first_name": "Ada"});
    let chat = json!({"id": -1003, "title": "News", "type": "channel"});
    let origins = [
        json!({"type": "user", "date": 1, "sender_user": user}),
        json!({"type": "hidden_user", "date": 2, "sender_user_name": "Anonymous"}),
        json!({"type": "chat", "date": 3, "sender_chat": chat, "author_signature": "Ed"}),
        json!({"type": "channel", "date": 4, "chat": chat, "message_id": 9}),
        json!({"type": "from_the_future", "date": 5}),
    ];
    let origins: Vec<MessageOrigin> = origins.into_iter().map(|o| serde_json::from_value(o).unwrap()).collect();

    match &origins[0] {
        MessageOrigin::User(o) => assert_eq!((o.date, o.sender_user.first_name.as_str()), (1, "Ada")),
        other => panic!("expected user, got {other:?}"),
    }
    match &origins[1] {
        MessageOrigin::HiddenUser(o) => assert_eq!((o.date, o.sender_user_name.as_str()), (2, "Anonymous")),
        other => panic!("expected hidden_user, got {other:?}"),
    }
    match &origins[2] {
        MessageOrigin::Chat(o) => assert_eq!((o.sender_chat.id, o.author_signature.as_deref()), (-1003, Some("Ed"))),
        other => panic!("expected chat, got {other:?}"),
    }
    match &origins[3] {
        MessageOrigin::Channel(o) => assert_eq!((o.chat.id, o.message_id), (-1003, 9)),
        other => panic!("expected channel, got {other:?}"),
    }
    assert_eq!(origins[4], MessageOrigin::Unknown);

    // The tag comes back on serialization even though the struct has no `type` field.
    let back = serde_json::to_value(&origins[1]).unwrap();
    assert_eq!(back, json!({"type": "hidden_user", "date": 2, "sender_user_name": "Anonymous"}));
}

#[test]
fn send_message_serializes_only_set_fields() {
    let send = SendMessage { chat_id: Some(ChatId::Id(42)), text: "hi".into(), ..Default::default() };
    assert_eq!(serde_json::to_value(&send).unwrap(), json!({"chat_id": 42, "text": "hi"}));

    let by_name = SendMessage { chat_id: Some("@news".into()), text: "hi".into(), ..Default::default() };
    assert_eq!(serde_json::to_string(&by_name).unwrap(), r#"{"chat_id":"@news","text":"hi"}"#);
}
