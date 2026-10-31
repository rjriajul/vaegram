use std::collections::BTreeMap;

use super::copy_message::{input_entities, input_media};
use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct CopyMediaGroup<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_id: i32,
    captions: BTreeMap<usize, String>,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    /// Copies the whole album that `message_id` belongs to, without a link to the original.
    pub fn copy_media_group(
        &self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> CopyMediaGroup<'_> {
        CopyMediaGroup {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_id,
            captions: BTreeMap::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}

impl CopyMediaGroup<'_> {
    /// Replaces the caption of the album item at `position`; `""` removes it. Other items keep theirs.
    pub fn caption(mut self, position: usize, caption: impl Into<String>) -> Self {
        self.captions.insert(position, caption.into());
        self
    }

    async fn send(mut self) -> Result<Vec<Message>> {
        let client = self.client;
        if self.options.suggested_post_parameters.is_some() {
            return Err(Error::InvalidArgument("an album can't be a suggested post"));
        }
        let (group, _) = client.media_group(self.from_chat_id, self.message_id).await?;
        if self.captions.last_key_value().is_some_and(|(&position, _)| position >= group.len()) {
            return Err(Error::InvalidArgument("the album has no item at this caption position"));
        }
        let invert_media = group.iter().any(|m| m.invert_media);
        let mut multi_media = Vec::with_capacity(group.len());
        for (position, source) in group.into_iter().enumerate() {
            let source = *source;
            let media = source.media.map(input_media).transpose()?.flatten();
            let media = media.ok_or(Error::Unexpected("getMessages returned an album item without media"))?;
            let (message, entities) = match self.captions.remove(&position) {
                Some(caption) => {
                    let (text, entities) = client.parse_text(&caption, self.parse_mode).await?;
                    (text, (!entities.is_empty()).then_some(entities))
                }
                None => (source.message, input_entities(client, source.entities).await?),
            };
            let random_id = i64::from_le_bytes(random());
            let single = types::InputSingleMedia { media, random_id, message, entities };
            multi_media.push(base::InputSingleMedia::InputSingleMedia(Box::new(single)));
        }
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let options = self.options;
        let request = functions::messages::SendMultiMedia {
            silent: options.disable_notification,
            background: false,
            clear_draft: false,
            noforwards: options.protect_content,
            update_stickersets_order: false,
            invert_media,
            allow_paid_floodskip: options.allow_paid_broadcast,
            reply_to: options.input_reply_to(client, chat_id, options.thread()).await?,
            peer,
            multi_media,
            schedule_date: options.schedule_date,
            send_as: None,
            quick_reply_shortcut: None,
            effect: options.message_effect_id,
            allow_paid_stars: options.paid_message_star_count,
        };
        let mut messages: Vec<_> = Message::all_from_updates(client, client.invoke(&request).await?).collect();
        if messages.is_empty() {
            return Err(Error::Unexpected("messages.sendMultiMedia returned no messages"));
        }
        messages.sort_unstable_by_key(|m| m.id);
        Ok(messages)
    }
}

caption_setters!(CopyMediaGroup);
send_builder!(CopyMediaGroup, Vec<Message>);
