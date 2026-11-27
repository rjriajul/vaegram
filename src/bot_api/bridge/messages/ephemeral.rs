//! Ephemeral messages: edits of their text, caption, keyboard and media, and deletes.
//!
//! The Bot API answers an ephemeral edit with `true`, so the edited message the friendly method returns is made from
//! the request: its id, chat, receiver, text or caption, entities and keyboard.

use serde_json::{Map, Value};

use super::json::{flag, put_text, rich_message};
use super::updates_of;
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::media::Files;
use crate::enums::MediaKind;
use crate::errors::{Error, Result};
use crate::raw::{Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::types::InputMedia;
use crate::utils::get_peer;

impl Transport {
    /// `ephemeral.editMessage` without media: `editEphemeralMessageText` for a text or a rich message, or
    /// `editEphemeralMessageCaption` when the message turns out to be a media one; `editEphemeralMessageReplyMarkup`
    /// without either. Welcome messages have no Bot API counterpart.
    pub(super) async fn edit_ephemeral(&self, r: f::ephemeral::EditMessage) -> Result<Vec<u8>> {
        if r.welcome {
            return Err(unsupported("welcome messages"));
        }
        if r.media.is_some() {
            return Err(unsupported("editing an ephemeral message's media with MTProto media"));
        }
        let me = self.me();
        let (mut body, chat_id, receiver) = self.ephemeral_target(r.peer.as_ref(), &r.receiver_id, r.id)?;
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        if let Some(rich) = &r.rich_message {
            body.insert("rich_message".into(), rich_message(rich)?);
            self.call("editEphemeralMessageText", Value::Object(body)).await?;
        } else if let Some(text) = &r.message {
            let mut caption = body.clone();
            put_text(&mut body, ("text", "entities"), text, r.entities.as_deref(), me);
            match self.call("editEphemeralMessageText", Value::Object(body)).await {
                Ok(_) => {}
                Err(Error::Rpc(e)) if e.name == "THERE_IS_NO_TEXT_IN_THE_MESSAGE_TO_EDIT" => {
                    put_text(&mut caption, ("caption", "caption_entities"), text, r.entities.as_deref(), me);
                    flag(&mut caption, "show_caption_above_media", r.invert_media);
                    self.call("editEphemeralMessageCaption", Value::Object(caption)).await?;
                }
                Err(e) => return Err(e),
            }
        } else {
            self.call("editEphemeralMessageReplyMarkup", Value::Object(body)).await?;
        }
        let mut edited = ephemeral(me, chat_id, receiver, r.id, r.message.unwrap_or_default());
        edited.entities = r.entities.filter(|list| !list.is_empty());
        edited.reply_markup = r.reply_markup;
        edited.invert_media = r.invert_media;
        Ok(ephemeral_updates(edited, me).to_bytes())
    }

    /// `edit_ephemeral_message_media`: `editEphemeralMessageMedia`, with an upload in the same request. The friendly
    /// method hands over its request without media, and the media to send, as the TL media it would make needs an
    /// MTProto upload. The caption comes from `r`; the edited message holds no media, as the Bot API returns none.
    pub(crate) async fn edit_ephemeral_media(
        &self,
        r: f::ephemeral::EditMessage,
        media: &InputMedia<'_>,
    ) -> Result<base::Updates> {
        let me = self.me();
        let (mut body, chat_id, receiver) = self.ephemeral_target(r.peer.as_ref(), &r.receiver_id, r.id)?;
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        let mut item = Map::new();
        item.insert("type".into(), media_type(media.kind)?.into());
        let mut files = Files::default();
        item.insert("media".into(), files.add(&media.file, media.file_name.as_deref()).await?);
        if let Some(caption) = r.message.as_deref().filter(|caption| !caption.is_empty()) {
            put_text(&mut item, ("caption", "caption_entities"), caption, r.entities.as_deref(), me);
        }
        flag(&mut item, "show_caption_above_media", r.invert_media);
        flag(&mut item, "has_spoiler", media.spoiler);
        body.insert("media".into(), Value::Object(item));
        let _: Value = self.api.call_with_files("editEphemeralMessageMedia", Value::Object(body), files.0).await?;
        let mut edited = ephemeral(me, chat_id, receiver, r.id, r.message.unwrap_or_default());
        edited.entities = r.entities.filter(|list| !list.is_empty());
        edited.reply_markup = r.reply_markup;
        edited.invert_media = r.invert_media;
        Ok(base::Updates::Updates(Box::new(ephemeral_updates(edited, me))))
    }

    /// `delete_ephemeral_message`: `deleteEphemeralMessage`.
    pub(super) async fn delete_ephemeral(&self, r: f::ephemeral::DeleteMessage) -> Result<Vec<u8>> {
        let (body, _, _) = self.ephemeral_target(r.peer.as_ref(), &r.receiver_id, r.id)?;
        self.call("deleteEphemeralMessage", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    /// The chat, receiver and id that name an ephemeral message, with the chat and receiver's ids. Without a chat,
    /// the message is in the private chat with its receiver.
    fn ephemeral_target(
        &self,
        peer: Option<&base::InputPeer>,
        receiver: &base::InputUser,
        id: i32,
    ) -> Result<(Map<String, Value>, i64, i64)> {
        let receiver = convert::input_user_id(receiver, self.me()).ok_or(Error::InvalidArgument("no such user"))?;
        let chat_id = match peer {
            Some(peer) => self.chat_id(peer)?,
            None => receiver,
        };
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        body.insert("receiver_user_id".into(), receiver.into());
        body.insert("ephemeral_message_id".into(), id.into());
        Ok((body, chat_id, receiver))
    }
}

/// The bot's outgoing ephemeral message `id` in `chat_id`, seen by `receiver`.
fn ephemeral(me: i64, chat_id: i64, receiver: i64, id: i32, message: String) -> types::EphemeralMessage {
    types::EphemeralMessage {
        out: true,
        welcome_template: false,
        invert_media: false,
        noforwards: false,
        id,
        from_id: get_peer(me),
        peer_id: Some(get_peer(chat_id)),
        receiver_id: receiver,
        top_msg_id: None,
        date: int(now()),
        message,
        entities: None,
        media: None,
        reply_markup: None,
        reply_to: None,
        rich_message: None,
        chat_instance: None,
        anchor_msg_id: None,
    }
}

fn ephemeral_updates(message: types::EphemeralMessage, me: i64) -> types::Updates {
    let message = base::EphemeralMessage::EphemeralMessage(Box::new(message));
    let update = base::Update::UpdateEditEphemeralMessage(Box::new(types::UpdateEditEphemeralMessage { message }));
    updates_of(vec![update], Peers::new(me))
}

/// The Bot API's `InputMedia` type of a message's new media.
fn media_type(kind: MediaKind) -> Result<&'static str> {
    Ok(match kind {
        MediaKind::Photo => "photo",
        MediaKind::Video => "video",
        MediaKind::Animation => "animation",
        MediaKind::Audio => "audio",
        MediaKind::Document => "document",
        _ => return Err(Error::InvalidArgument("an ephemeral message's media can't be of this kind")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_answer_names_the_chat_receiver_and_text() {
        let edited = ephemeral(9, -5, 7, 3, "hi".into());
        let updates = ephemeral_updates(edited, 9);
        let base::Update::UpdateEditEphemeralMessage(update) = &updates.updates[0] else { panic!("not an edit") };
        let base::EphemeralMessage::EphemeralMessage(message) = &update.message;
        assert_eq!((message.id, message.receiver_id, message.message.as_str()), (3, 7, "hi"));
        assert_eq!((message.peer_id.clone(), message.from_id.clone()), (Some(get_peer(-5)), get_peer(9)));
        assert!(message.out);
        assert!(media_type(MediaKind::Voice).is_err());
    }
}
