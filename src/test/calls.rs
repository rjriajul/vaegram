use crate::raw::{Deserializable, Identifiable, base, functions};
use crate::utils::{get_channel_id, input_peer_id};

// The wrappers a client may send a request inside, with the request last.
const INVOKE_WITHOUT_UPDATES: u32 = 0xbf9459b7;
const INVOKE_WITH_TAKEOUT: u32 = 0xaca9fd2e;
const INVOKE_WITH_BUSINESS_CONNECTION: u32 = 0xdd289f8e;

const SEND_MESSAGE: u32 = functions::messages::SendMessage::CONSTRUCTOR_ID;
const SEND_MEDIA: u32 = functions::messages::SendMedia::CONSTRUCTOR_ID;
const EDIT_MESSAGE: u32 = functions::messages::EditMessage::CONSTRUCTOR_ID;
const DELETE_MESSAGES: u32 = functions::messages::DeleteMessages::CONSTRUCTOR_ID;
const DELETE_CHANNEL_MESSAGES: u32 = functions::channels::DeleteMessages::CONSTRUCTOR_ID;
const SET_BOT_CALLBACK_ANSWER: u32 = functions::messages::SetBotCallbackAnswer::CONSTRUCTOR_ID;
const SEND_REACTION: u32 = functions::messages::SendReaction::CONSTRUCTOR_ID;

/// A request a test client sent, as its [`Recorder`](super::Recorder) saw it.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RecordedCall {
    /// The request's TL name, such as `"messages.sendMessage"`. A request sent inside wrappers (no updates, a
    /// business connection, a takeout) has the name of the request inside them.
    pub method: &'static str,
    /// The tag of the innermost [`tagged`](crate::observe::tagged) the request was sent inside, if any.
    pub tag: Option<u64>,
    /// What the request does, read from it for the common ones.
    pub kind: CallKind,
    body: Vec<u8>,
    // Where the request inside the wrappers starts in `body`.
    query: usize,
}

/// What a recorded request does, for the requests bots send most; [`Other`](Self::Other) for the rest. Chat ids
/// are Bot-API style: users positive, basic groups negative, channels and supergroups `-100…`.
///
/// Each variant may gain fields (entities, keyboards, the reaction itself), so match them with `..`:
/// `CallKind::SendMessage { chat_id: 42, text, .. }`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallKind {
    /// A text message sent (`messages.sendMessage`).
    #[non_exhaustive]
    SendMessage {
        /// The chat it went to.
        chat_id: i64,
        /// Its text, as sent: after the parse mode took the formatting out.
        text: String,
        /// The message it replies to, in the same chat or another one.
        reply_to_message_id: Option<i32>,
    },
    /// A photo, file or other media sent (`messages.sendMedia`).
    #[non_exhaustive]
    SendMedia {
        /// The chat it went to.
        chat_id: i64,
        /// Its caption, as sent; empty for none.
        caption: String,
        /// The message it replies to, in the same chat or another one.
        reply_to_message_id: Option<i32>,
    },
    /// A message edited (`messages.editMessage`).
    #[non_exhaustive]
    EditMessage {
        /// The chat the message is in.
        chat_id: i64,
        /// The message edited.
        message_id: i32,
        /// The new text; `None` when only the media or keyboard changed.
        text: Option<String>,
    },
    /// Messages deleted (`messages.deleteMessages` or `channels.deleteMessages`).
    #[non_exhaustive]
    DeleteMessages {
        /// The channel or supergroup; `None` for private chats and basic groups, whose messages are deleted by id
        /// alone.
        chat_id: Option<i64>,
        /// The messages deleted.
        message_ids: Vec<i32>,
    },
    /// A callback query answered (`messages.setBotCallbackAnswer`).
    #[non_exhaustive]
    AnswerCallbackQuery {
        /// The query answered.
        query_id: i64,
        /// The text shown to the user, if any.
        text: Option<String>,
        /// Whether the text is shown as an alert rather than a toast.
        alert: bool,
    },
    /// Reactions set on a message, or taken off it (`messages.sendReaction`).
    #[non_exhaustive]
    SendReaction {
        /// The chat the message is in.
        chat_id: i64,
        /// The message reacted to.
        message_id: i32,
    },
    /// Any other request.
    #[non_exhaustive]
    Other {
        /// Its TL name, as in [`RecordedCall::method`].
        method: &'static str,
    },
}

impl RecordedCall {
    /// The request `name` with the TL bytes `body`, sent by the account `me`.
    pub(crate) fn new(name: &'static str, tag: Option<u64>, body: Vec<u8>, me: i64) -> RecordedCall {
        let query = query_start(&body);
        let inner = &body[query..];
        let id = constructor(inner);
        let method = if query == 0 { name } else { id.and_then(crate::raw::function_name).unwrap_or(name) };
        let kind = id.and_then(|id| read(id, inner.get(4..)?, me)).unwrap_or(CallKind::Other { method });
        RecordedCall { method, tag, kind, body, query }
    }

    /// The request inside the wrappers, its constructor id first.
    pub(crate) fn query(&self) -> &[u8] {
        &self.body[self.query..]
    }

    /// The media of a `messages.sendMedia` request.
    pub(crate) fn input_media(&self) -> Option<base::InputMedia> {
        let buf = &mut self.query().get(4..)?;
        let flags: u32 = next(buf)?;
        next::<base::InputPeer>(buf)?;
        if flags & 1 != 0 {
            next::<base::InputReplyTo>(buf)?;
        }
        next(buf)
    }

    /// The request as JSON in the shape of [`ToJson`](crate::raw::ToJson), with credentials cut as a
    /// [recording](crate::storage::recording) cuts them.
    #[cfg(feature = "json")]
    pub fn request_json(&self) -> String {
        crate::storage::recording::request_json(&self.body)
    }

    /// The request's TL bytes, wrappers included.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

fn constructor(body: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(body.get(..4)?.try_into().ok()?))
}

/// Where the request inside `body`'s wrappers starts.
fn query_start(body: &[u8]) -> usize {
    let mut rest = body;
    loop {
        let inner = match constructor(rest) {
            Some(INVOKE_WITHOUT_UPDATES) => rest.get(4..),
            Some(INVOKE_WITH_TAKEOUT) => rest.get(12..),
            Some(INVOKE_WITH_BUSINESS_CONNECTION) => rest.get(4..).and_then(|mut tail| {
                String::deserialize(&mut tail).ok()?;
                Some(tail)
            }),
            _ => None,
        };
        match inner {
            Some(inner) => rest = inner,
            None => return body.len() - rest.len(),
        }
    }
}

fn next<T: Deserializable>(buf: &mut &[u8]) -> Option<T> {
    T::deserialize(buf).ok()
}

/// The `CallKind` of the request with constructor `id` and fields `buf`, sent by the account `me`.
fn read(id: u32, mut buf: &[u8], me: i64) -> Option<CallKind> {
    let buf = &mut buf;
    // A peer that names nobody leaves the request as `Other`, not in chat 0.
    let chat = |peer: &base::InputPeer| input_peer_id(peer, me);
    let reply = |reply_to: Option<base::InputReplyTo>| match reply_to {
        Some(base::InputReplyTo::InputReplyToMessage(r)) => Some(r.reply_to_msg_id),
        _ => None,
    };
    Some(match id {
        SEND_MESSAGE | SEND_MEDIA => {
            let flags: u32 = next(buf)?;
            let peer: base::InputPeer = next(buf)?;
            let reply_to = if flags & 1 != 0 { Some(next::<base::InputReplyTo>(buf)?) } else { None };
            if id == SEND_MEDIA {
                next::<base::InputMedia>(buf)?;
            }
            let text: String = next(buf)?;
            let (chat_id, reply_to_message_id) = (chat(&peer)?, reply(reply_to));
            if id == SEND_MEDIA {
                CallKind::SendMedia { chat_id, caption: text, reply_to_message_id }
            } else {
                CallKind::SendMessage { chat_id, text, reply_to_message_id }
            }
        }
        EDIT_MESSAGE => {
            let flags: u32 = next(buf)?;
            let peer: base::InputPeer = next(buf)?;
            let message_id: i32 = next(buf)?;
            let text = if flags & (1 << 11) != 0 { Some(next::<String>(buf)?) } else { None };
            CallKind::EditMessage { chat_id: chat(&peer)?, message_id, text }
        }
        DELETE_MESSAGES => {
            next::<u32>(buf)?;
            CallKind::DeleteMessages { chat_id: None, message_ids: next(buf)? }
        }
        DELETE_CHANNEL_MESSAGES => {
            let chat_id = match next::<base::InputChannel>(buf)? {
                base::InputChannel::InputChannel(c) => Some(get_channel_id(c.channel_id)),
                base::InputChannel::InputChannelFromMessage(c) => Some(get_channel_id(c.channel_id)),
                base::InputChannel::InputChannelEmpty => None,
            };
            CallKind::DeleteMessages { chat_id, message_ids: next(buf)? }
        }
        SET_BOT_CALLBACK_ANSWER => {
            let flags: u32 = next(buf)?;
            let query_id: i64 = next(buf)?;
            let text = if flags & 1 != 0 { Some(next::<String>(buf)?) } else { None };
            CallKind::AnswerCallbackQuery { query_id, text, alert: flags & 2 != 0 }
        }
        SEND_REACTION => {
            next::<u32>(buf)?;
            let peer: base::InputPeer = next(buf)?;
            CallKind::SendReaction { chat_id: chat(&peer)?, message_id: next(buf)? }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::{RemoteCall, Serializable, types};

    fn user(id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash: 1 }))
    }

    fn reply_to(id: i32) -> base::InputReplyTo {
        base::InputReplyTo::InputReplyToMessage(Box::new(types::InputReplyToMessage::new(id)))
    }

    #[test]
    fn a_text_message_is_read() {
        let request = functions::messages::SendMessage {
            reply_to: Some(reply_to(3)),
            ..functions::messages::SendMessage::new(user(42), "hi".into(), 7)
        };
        let call = RecordedCall::new(functions::messages::SendMessage::NAME, None, request.to_bytes(), 1);
        let kind = CallKind::SendMessage { chat_id: 42, text: "hi".into(), reply_to_message_id: Some(3) };
        assert_eq!((call.method, call.kind), ("messages.sendMessage", kind));
    }

    #[test]
    fn a_request_in_wrappers_keeps_its_name() {
        let answer = functions::messages::SetBotCallbackAnswer {
            alert: true,
            query_id: 9,
            message: Some("ok".into()),
            url: None,
            cache_time: 0,
        };
        let business = functions::InvokeWithBusinessConnection { connection_id: "c".into(), query: answer };
        let request = functions::InvokeWithTakeout { takeout_id: 5, query: business };
        let call = RecordedCall::new("invokeWithTakeout", Some(4), request.to_bytes(), 1);
        let kind = CallKind::AnswerCallbackQuery { query_id: 9, text: Some("ok".into()), alert: true };
        assert_eq!((call.method, call.tag, call.kind), ("messages.setBotCallbackAnswer", Some(4), kind));

        let delete = functions::messages::DeleteMessages { revoke: true, id: vec![4, 5] };
        let request = functions::InvokeWithoutUpdates { query: delete };
        let call = RecordedCall::new("invokeWithoutUpdates", None, request.to_bytes(), 1);
        let kind = CallKind::DeleteMessages { chat_id: None, message_ids: vec![4, 5] };
        assert_eq!((call.method, call.kind), ("messages.deleteMessages", kind));

        // Any function, not only the ones the kit answers.
        let request = functions::InvokeWithoutUpdates { query: functions::help::GetConfig {} };
        let call = RecordedCall::new("invokeWithoutUpdates", None, request.to_bytes(), 1);
        assert_eq!((call.method, call.kind), ("help.getConfig", CallKind::Other { method: "help.getConfig" }));
    }

    #[test]
    fn unknown_and_broken_requests_are_other() {
        let call = RecordedCall::new("help.getConfig", None, functions::help::GetConfig {}.to_bytes(), 1);
        assert_eq!(call.kind, CallKind::Other { method: "help.getConfig" });
        let call = RecordedCall::new("messages.sendMessage", None, SEND_MESSAGE.to_le_bytes().to_vec(), 1);
        assert_eq!(call.kind, CallKind::Other { method: "messages.sendMessage" });
        let call = RecordedCall::new("x", None, vec![1], 1);
        assert_eq!((call.method, call.query()), ("x", &[1u8][..]));
        let nobody = functions::messages::SendMessage::new(base::InputPeer::InputPeerEmpty, "hi".into(), 7);
        let call = RecordedCall::new("messages.sendMessage", None, nobody.to_bytes(), 1);
        assert_eq!(call.kind, CallKind::Other { method: "messages.sendMessage" }, "not chat 0");
    }
}
