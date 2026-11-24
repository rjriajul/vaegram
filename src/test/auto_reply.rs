//! The answers a test client gives on its own: valid TL for the requests bots send most, and an error naming the
//! request for the rest, never bytes its caller can't read.

use std::time::{SystemTime, UNIX_EPOCH};

use super::calls::{CallKind, RecordedCall};
use super::{State, TestError};
use crate::errors::Result;
use crate::raw::{Deserializable, RemoteCall, Serializable, base, functions, types};
use crate::types::PeerId;
use crate::utils::get_peer;

/// The access hash of every user, channel, photo and document a test client is told about.
pub(crate) const ACCESS_HASH: i64 = 0x7e57;
/// The data center of the files a test client sends.
const DC_ID: i32 = 2;

const SEND_MESSAGE: &str = functions::messages::SendMessage::NAME;
const SEND_MEDIA: &str = functions::messages::SendMedia::NAME;
const EDIT_MESSAGE: &str = functions::messages::EditMessage::NAME;
const DELETE_MESSAGES: &str = functions::messages::DeleteMessages::NAME;
const DELETE_CHANNEL_MESSAGES: &str = functions::channels::DeleteMessages::NAME;
const READ_HISTORY: &str = functions::messages::ReadHistory::NAME;
const SET_BOT_CALLBACK_ANSWER: &str = functions::messages::SetBotCallbackAnswer::NAME;
const SET_INLINE_BOT_RESULTS: &str = functions::messages::SetInlineBotResults::NAME;
const SET_TYPING: &str = functions::messages::SetTyping::NAME;
const SET_BOT_COMMANDS: &str = functions::bots::SetBotCommands::NAME;
const SEND_REACTION: &str = functions::messages::SendReaction::NAME;
const GET_STATE: &str = functions::updates::GetState::NAME;
const GET_USERS: &str = functions::users::GetUsers::NAME;
const GET_CHANNELS: &str = functions::channels::GetChannels::NAME;
const SAVE_FILE_PART: &str = functions::upload::SaveFilePart::NAME;
const SAVE_BIG_FILE_PART: &str = functions::upload::SaveBigFilePart::NAME;

/// The default answer to `call`.
pub(crate) fn answer(state: &State, call: &RecordedCall) -> Result<Vec<u8>> {
    let no_answer = || crate::Error::from(TestError::NoDefaultAnswer { method: call.method });
    Ok(match call.method {
        SEND_MESSAGE => short_sent(state, state.next_message_id(), None),
        SEND_MEDIA => {
            let media = sent_media(state, call).ok_or_else(no_answer)?;
            short_sent(state, state.next_message_id(), Some(media))
        }
        EDIT_MESSAGE => return edited(state, call, None),
        DELETE_MESSAGES | DELETE_CHANNEL_MESSAGES | READ_HISTORY => {
            let channel = match &call.kind {
                CallKind::DeleteMessages { chat_id: Some(chat_id), .. } => PeerId::from_bot_api(*chat_id).channel(),
                _ => None,
            };
            let pts = channel.map_or_else(|| state.next_pts(), |channel| state.next_channel_pts(channel));
            let affected = types::messages::AffectedMessages { pts, pts_count: 1 };
            base::messages::AffectedMessages::AffectedMessages(affected).to_bytes()
        }
        SET_BOT_CALLBACK_ANSWER | SET_INLINE_BOT_RESULTS | SET_TYPING | SET_BOT_COMMANDS => true.to_bytes(),
        SAVE_FILE_PART | SAVE_BIG_FILE_PART => true.to_bytes(),
        SEND_REACTION => updates(Vec::new()),
        GET_STATE => {
            let state = types::updates::State { pts: state.pts(), qts: 0, date: now(), seq: 0, unread_count: 0 };
            base::updates::State::State(Box::new(state)).to_bytes()
        }
        GET_USERS => return users(state, call),
        GET_CHANNELS => return channels(call),
        _ => return Err(no_answer()),
    })
}

/// The answer to `call` that sends or edits the message `id`.
pub(crate) fn sent(state: &State, call: &RecordedCall, id: i32) -> Result<Vec<u8>> {
    state.took_message_id(id);
    match call.method {
        SEND_MESSAGE => Ok(short_sent(state, id, None)),
        SEND_MEDIA => match sent_media(state, call) {
            Some(media) => Ok(short_sent(state, id, Some(media))),
            None => Err(TestError::NoDefaultAnswer { method: call.method }.into()),
        },
        EDIT_MESSAGE => edited(state, call, Some(id)),
        method => Err(TestError::UnfitReply { method }.into()),
    }
}

fn now() -> i32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i32)
}

/// Telegram's short answer to a send: the id, the date, the `pts` and the media; the rest comes from the request.
fn short_sent(state: &State, id: i32, media: Option<base::MessageMedia>) -> Vec<u8> {
    let sent = types::UpdateShortSentMessage {
        out: true,
        id,
        pts: state.next_pts(),
        pts_count: 1,
        date: now(),
        media,
        entities: None,
        ttl_period: None,
    };
    base::Updates::UpdateShortSentMessage(Box::new(sent)).to_bytes()
}

/// The media a `messages.sendMedia` request sends, as Telegram shows it: a photo or a document, an uploaded one
/// under a new id. `None` for other media (a poll, a location, a contact, a web link, …).
fn sent_media(state: &State, call: &RecordedCall) -> Option<base::MessageMedia> {
    Some(match call.input_media()? {
        base::InputMedia::InputMediaUploadedPhoto(m) => {
            photo(photo_of(state.next_file_id(), ACCESS_HASH, Vec::new()), m.spoiler)
        }
        base::InputMedia::InputMediaPhoto(m) => {
            let spoiler = m.spoiler;
            let base::InputPhoto::InputPhoto(p) = m.id else { return None };
            photo(photo_of(p.id, p.access_hash, p.file_reference), spoiler)
        }
        base::InputMedia::InputMediaUploadedDocument(m) => {
            let spoiler = m.spoiler;
            document(state.next_file_id(), ACCESS_HASH, Vec::new(), m.mime_type, m.attributes, spoiler)
        }
        base::InputMedia::InputMediaDocument(m) => {
            let spoiler = m.spoiler;
            let base::InputDocument::InputDocument(d) = m.id else { return None };
            let mime_type = "application/octet-stream".to_owned();
            document(d.id, d.access_hash, d.file_reference, mime_type, Vec::new(), spoiler)
        }
        _ => return None,
    })
}

fn photo_of(id: i64, access_hash: i64, file_reference: Vec<u8>) -> base::Photo {
    let size = types::PhotoSize { r#type: "x".into(), w: 800, h: 600, size: 0 };
    let sizes = vec![base::PhotoSize::PhotoSize(Box::new(size))];
    base::Photo::Photo(Box::new(types::Photo::new(id, access_hash, file_reference, now(), sizes, DC_ID)))
}

fn photo(photo: base::Photo, spoiler: bool) -> base::MessageMedia {
    let media = types::MessageMediaPhoto { spoiler, photo: Some(photo), ..Default::default() };
    base::MessageMedia::MessageMediaPhoto(Box::new(media))
}

/// A document message, with the video, round and voice flags its attributes call for.
fn document(
    id: i64,
    access_hash: i64,
    file_reference: Vec<u8>,
    mime_type: String,
    attributes: Vec<base::DocumentAttribute>,
    spoiler: bool,
) -> base::MessageMedia {
    let (mut video, mut round, mut voice) = (false, false, false);
    for attribute in &attributes {
        match attribute {
            base::DocumentAttribute::DocumentAttributeVideo(v) => (video, round) = (!v.round_message, v.round_message),
            base::DocumentAttribute::DocumentAttributeAudio(a) => voice = a.voice,
            _ => {}
        }
    }
    let file = types::Document::new(id, access_hash, file_reference, now(), mime_type, 0, DC_ID, attributes);
    let document = Some(base::Document::Document(Box::new(file)));
    let media = types::MessageMediaDocument { spoiler, video, round, voice, document, ..Default::default() };
    base::MessageMedia::MessageMediaDocument(Box::new(media))
}

fn updates(updates: Vec<base::Update>) -> Vec<u8> {
    let updates = types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: now(), seq: 0 };
    base::Updates::Updates(Box::new(updates)).to_bytes()
}

/// The edited message, with the chat, id and text of the request, or the id `id`: in a channel or supergroup, as
/// Telegram sends it there, under the channel's own `pts`.
fn edited(state: &State, call: &RecordedCall, id: Option<i32>) -> Result<Vec<u8>> {
    let CallKind::EditMessage { chat_id, message_id, text } = &call.kind else {
        return Err(TestError::NoDefaultAnswer { method: call.method }.into());
    };
    let date = now();
    let message = types::Message {
        id: id.unwrap_or(*message_id),
        date,
        edit_date: Some(date),
        out: true,
        from_id: Some(get_peer(state.me.id)),
        ..crate::types::Message::request(*chat_id, text.clone().unwrap_or_default(), None)
    };
    let message = base::Message::Message(Box::new(message));
    let update = match PeerId::from_bot_api(*chat_id).channel() {
        Some(channel) => {
            let pts = state.next_channel_pts(channel);
            let update = types::UpdateEditChannelMessage { message, pts, pts_count: 1 };
            base::Update::UpdateEditChannelMessage(Box::new(update))
        }
        None => {
            let update = types::UpdateEditMessage { message, pts: state.next_pts(), pts_count: 1 };
            base::Update::UpdateEditMessage(Box::new(update))
        }
    };
    Ok(updates(vec![update]))
}

/// Each user asked for, with [`ACCESS_HASH`]; the account itself as it is.
fn users(state: &State, call: &RecordedCall) -> Result<Vec<u8>> {
    let me = &state.me;
    let mut fields = call.query().get(4..).unwrap_or_default();
    let users: Vec<base::User> = Vec::<base::InputUser>::deserialize(&mut fields)?
        .iter()
        .filter_map(|user| match user {
            base::InputUser::InputUserSelf => Some(me.id),
            base::InputUser::InputUser(u) => Some(u.user_id),
            base::InputUser::InputUserFromMessage(u) => Some(u.user_id),
            base::InputUser::InputUserEmpty => None,
        })
        .map(|id| {
            let user = types::User { access_hash: Some(ACCESS_HASH), ..types::User::new(id) };
            let user = if id == me.id {
                types::User {
                    is_self: true,
                    bot: me.bot,
                    // `bot` and `bot_info_version` share flag 14: a bot without the version would be cut short.
                    bot_info_version: me.bot.then_some(1),
                    premium: me.premium,
                    username: me.username.clone(),
                    ..user
                }
            } else {
                user
            };
            base::User::User(Box::new(user))
        })
        .collect();
    Ok(users.to_bytes())
}

/// Each channel asked for, as a supergroup with [`ACCESS_HASH`].
fn channels(call: &RecordedCall) -> Result<Vec<u8>> {
    let mut fields = call.query().get(4..).unwrap_or_default();
    let chats = Vec::<base::InputChannel>::deserialize(&mut fields)?
        .iter()
        .filter_map(|channel| match channel {
            base::InputChannel::InputChannel(c) => Some(c.channel_id),
            base::InputChannel::InputChannelFromMessage(c) => Some(c.channel_id),
            base::InputChannel::InputChannelEmpty => None,
        })
        .map(|id| {
            let photo = base::ChatPhoto::ChatPhotoEmpty;
            let channel = types::Channel {
                access_hash: Some(ACCESS_HASH),
                megagroup: true,
                ..types::Channel::new(id, String::new(), photo, 0)
            };
            base::Chat::Channel(Box::new(channel))
        })
        .collect();
    Ok(base::messages::Chats::Chats(Box::new(types::messages::Chats { chats })).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::TestBuilder;

    #[tokio::test]
    async fn every_default_answer_reads_as_its_request_returns() {
        let (client, _recorder) = TestBuilder::new().bot(7, Some("bot")).recording();
        let user = |id| base::InputUser::InputUser(Box::new(types::InputUser { user_id: id, access_hash: 0 }));
        let request = functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf, user(42)] };
        let users = client.invoke(&request).await.unwrap();
        let ids: Vec<_> = users
            .iter()
            .map(|u| match u {
                base::User::User(u) => (u.id, u.is_self, u.access_hash),
                other => panic!("not a user: {other:?}"),
            })
            .collect();
        assert_eq!(ids, [(7, true, Some(ACCESS_HASH)), (42, false, Some(ACCESS_HASH))]);

        let state = client.invoke(&functions::updates::GetState {}).await.unwrap();
        let base::updates::State::State(state) = state;
        assert_eq!(state.pts, 0);
        let delete = functions::messages::DeleteMessages { revoke: true, id: vec![1] };
        let base::messages::AffectedMessages::AffectedMessages(affected) = client.invoke(&delete).await.unwrap();
        assert_eq!(affected.pts, 1);
    }

    /// Sends every request with a default answer and reads the answer as the type the request returns.
    async fn every_answer_reads_back(client: &crate::Client) {
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 1 }));
        let peer = types::InputPeerChannel { channel_id: 5, access_hash: 1 };
        let channel = base::InputPeer::InputPeerChannel(Box::new(peer));
        let input_channel =
            || base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 5, access_hash: 1 }));
        let file = types::InputFile { id: 1, parts: 1, name: "a.jpg".into(), md5_checksum: String::new() };
        let media = types::InputMediaUploadedPhoto::new(base::InputFile::InputFile(Box::new(file)));
        let media = base::InputMedia::InputMediaUploadedPhoto(Box::new(media));

        client.invoke(&functions::messages::SendMessage::new(user.clone(), "hi".into(), 1)).await.unwrap();
        client.invoke(&functions::messages::SendMedia::new(user.clone(), media, String::new(), 2)).await.unwrap();
        client.invoke(&functions::messages::EditMessage::new(user.clone(), 1)).await.unwrap();
        client.invoke(&functions::messages::EditMessage::new(channel, 1)).await.unwrap();
        client.invoke(&functions::messages::DeleteMessages { revoke: true, id: vec![1] }).await.unwrap();
        client.invoke(&functions::channels::DeleteMessages { channel: input_channel(), id: vec![1] }).await.unwrap();
        client.invoke(&functions::messages::ReadHistory { peer: user.clone(), max_id: 1 }).await.unwrap();
        let answer = functions::messages::SetBotCallbackAnswer::new(9, 0);
        assert!(client.invoke(&answer).await.unwrap());
        assert!(client.invoke(&functions::messages::SetInlineBotResults::new(9, Vec::new(), 0)).await.unwrap());
        let typing = base::SendMessageAction::SendMessageTypingAction;
        let typing = functions::messages::SetTyping { peer: user.clone(), top_msg_id: None, action: typing };
        assert!(client.invoke(&typing).await.unwrap());
        let scope = base::BotCommandScope::BotCommandScopeDefault;
        let commands = functions::bots::SetBotCommands { scope, lang_code: String::new(), commands: Vec::new() };
        assert!(client.invoke(&commands).await.unwrap());
        let part = functions::upload::SaveFilePart { file_id: 1, file_part: 0, bytes: vec![1] };
        assert!(client.invoke(&part).await.unwrap());
        let big = functions::upload::SaveBigFilePart { file_id: 2, file_part: 0, file_total_parts: 1, bytes: vec![1] };
        assert!(client.invoke(&big).await.unwrap());
        client.invoke(&functions::messages::SendReaction::new(user, 1)).await.unwrap();
        client.invoke(&functions::updates::GetState {}).await.unwrap();
        client.invoke(&functions::channels::GetChannels { id: vec![input_channel()] }).await.unwrap();
        let users = vec![base::InputUser::InputUserSelf, base::InputUser::InputUserEmpty];
        assert_eq!(client.invoke(&functions::users::GetUsers { id: users }).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn default_answers_read_back_for_a_bot_and_a_user() {
        for builder in [TestBuilder::new().bot(7, Some("bot")), TestBuilder::new().user(8, true)] {
            let (client, _recorder) = builder.recording();
            every_answer_reads_back(&client).await;
        }
        // The account alone, the vector's last element: a short read can't hide in the next one.
        let (client, _recorder) = TestBuilder::new().bot(7, None).recording();
        let request = functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] };
        let users = client.invoke(&request).await.unwrap();
        let base::User::User(me) = &users[0] else { panic!("not a user") };
        assert_eq!((me.bot, me.bot_info_version), (true, Some(1)));
    }

    #[tokio::test]
    async fn a_sent_photo_comes_back_with_a_photo() {
        let (client, _recorder) = TestBuilder::new().recording();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 1 }));
        let file = types::InputFile { id: 1, parts: 1, name: "a.jpg".into(), md5_checksum: String::new() };
        let media = types::InputMediaUploadedPhoto::new(base::InputFile::InputFile(Box::new(file)));
        let media = base::InputMedia::InputMediaUploadedPhoto(Box::new(media));
        let sent = client.invoke(&functions::messages::SendMedia::new(user, media, String::new(), 2)).await.unwrap();
        let base::Updates::UpdateShortSentMessage(sent) = sent else { panic!("not a short sent message") };
        assert!(matches!(sent.media, Some(base::MessageMedia::MessageMediaPhoto(_))));
    }

    #[tokio::test]
    async fn a_request_without_a_default_answer_names_itself() {
        let (client, _recorder) = TestBuilder::new().recording();
        let error = client.invoke(&functions::help::GetConfig {}).await.unwrap_err();
        assert!(error.to_string().starts_with("help.getConfig has no default answer"), "{error}");
    }
}
