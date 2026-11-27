//! Edits of media that holds no file (live locations, checklists, closing a poll) and of widget data.

use serde_json::{Map, Value};

use super::content;
use super::json::{checklist, live, place};
use super::send::CHECKLISTS;
use super::{empty_updates, updates_of};
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::client::invalid;
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::{Error, Result};
use crate::raw::{Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::utils::get_peer;

impl Transport {
    /// `messages.editMessage` with a live location, a checklist or a closed poll, through the business connection
    /// `business` if set: `editMessageLiveLocation`, `stopMessageLiveLocation`, `editMessageChecklist` or
    /// `stopPoll`. The core bridge's `messages.editMessage` hands here any media other than a link preview.
    ///
    /// `stopPoll` answers with the poll alone, so the edited message holds the poll and the keyboard and no text.
    /// A poll can only be closed, and a checklist edited only through a business connection.
    pub(crate) async fn edit_media(
        &self,
        r: f::messages::EditMessage,
        business: Option<&str>,
    ) -> Result<base::Updates> {
        let f::messages::EditMessage { peer, id, media, reply_markup, schedule_date, quick_reply_shortcut_id, .. } = r;
        if schedule_date.is_some() || quick_reply_shortcut_id.is_some() {
            return Err(unsupported("editing scheduled or quick reply messages"));
        }
        let me = self.me();
        let chat_id = self.chat_id(&peer)?;
        let mut body = Map::new();
        if let Some(id) = business {
            body.insert("business_connection_id".into(), id.into());
        }
        body.insert("chat_id".into(), chat_id.into());
        body.insert("message_id".into(), id.into());
        if let Some(markup) = &reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        let method = match &media {
            Some(base::InputMedia::InputMediaGeoLive(m)) if m.stopped => "stopMessageLiveLocation",
            Some(base::InputMedia::InputMediaGeoLive(m)) => {
                place(&mut body, &m.geo_point)?;
                live(&mut body, m);
                "editMessageLiveLocation"
            }
            Some(base::InputMedia::InputMediaTodo(m)) => {
                if business.is_none() {
                    return Err(Error::InvalidArgument(CHECKLISTS));
                }
                body.insert("checklist".into(), checklist(&m.todo, me));
                "editMessageChecklist"
            }
            Some(base::InputMedia::InputMediaPoll(m)) => {
                let base::Poll::Poll(poll) = &m.poll;
                if !poll.closed {
                    return Err(unsupported("editing a poll other than closing it"));
                }
                return self.stop_poll_request(chat_id, id, body, reply_markup).await;
            }
            _ => return Err(unsupported("editing this media")),
        };
        let edited = self.call(method, Value::Object(body)).await?;
        // An inline message's edit answers `true`; a chat message's, the message.
        let updates = match edited {
            Value::Object(_) => self.updates(vec![edited], true)?,
            _ => empty_updates(me),
        };
        Ok(base::Updates::Updates(Box::new(updates)))
    }

    /// `stopPoll`, which answers with the closed poll: the edited message is made of it and `reply_markup`.
    async fn stop_poll_request(
        &self,
        chat_id: i64,
        id: i32,
        body: Map<String, Value>,
        reply_markup: Option<base::ReplyMarkup>,
    ) -> Result<base::Updates> {
        let poll = self.call("stopPoll", Value::Object(body)).await?;
        let poll: bot::Poll = serde_json::from_value(poll).map_err(invalid)?;
        let mut peers = Peers::new(self.me());
        let mut raw = types::Message::new(id, get_peer(chat_id), int(now()), String::new());
        raw.out = true;
        raw.media = Some(content::poll(&poll, &mut peers));
        raw.reply_markup = reply_markup;
        self.saw(&raw);
        let updates = updates_of(vec![convert::message_update(raw, true)], peers);
        Ok(base::Updates::Updates(Box::new(updates)))
    }

    /// `edit_message_widget_data`: `editMessageWidgetData`. Its `true` holds no edited message.
    pub(super) async fn edit_widget_data(&self, r: f::messages::EditMessageWidgetData) -> Result<Vec<u8>> {
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("message_id".into(), r.id.into());
        if let Some(data) = r.public_data {
            body.insert("public_data".into(), data.into());
        }
        if let Some(data) = r.chat_data {
            body.insert("chat_data".into(), data.into());
        }
        self.call("editMessageWidgetData", Value::Object(body)).await?;
        Ok(empty_updates(self.me()).to_bytes())
    }

    /// `edit_shared_widget_data`: `editSharedWidgetData`.
    pub(super) async fn edit_shared_widget(&self, r: f::messages::EditWidgetData) -> Result<Vec<u8>> {
        let mut body = Map::new();
        body.insert("widget_id".into(), r.widget_id.to_string().into());
        body.insert("public_data".into(), r.public_data.into());
        if let Some(seconds) = r.cache_time {
            body.insert("cache_time".into(), seconds.into());
        }
        self.call("editSharedWidgetData", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }
}
