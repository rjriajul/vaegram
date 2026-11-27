//! Sends that carry no file: places, contacts, dice, polls, checklists, games and widgets (`messages.sendMedia`),
//! drafts, rich messages and copies of several messages at once.

use serde_json::{Map, Value};

use super::json::{checklist, flag, live, place, put_text, rich_message, text, venue};
use super::updates_of;
use crate::Client;
use crate::bot_api::bridge::{Transport, options, reply_parameters, unsupported};
use crate::bot_api::client::invalid;
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::{Error, Result};
use crate::methods::SendOptions;
use crate::raw::{Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::types::{Message, PeerRef, ReplyMarkup, RichMessage};
use crate::utils::get_peer;

/// Why a checklist without a business connection fails before any request.
pub(super) const CHECKLISTS: &str =
    "over the Bot API, bots send and edit checklists only through a business connection";

impl Transport {
    /// `messages.sendMedia` with media that holds no file: `sendLocation`, `sendVenue`, `sendContact`, `sendDice`,
    /// `sendPoll`, `sendChecklist`, `sendGame`, `sendWidget` and `sendSharedWidget`. `None` for other media, which
    /// another group or a friendly method's own branch sends.
    ///
    /// Not carried: a poll's files (a place or a link is), a game named by id, and the options the Bot API lacks
    /// (see [`media_send_fields`](Self::media_send_fields)).
    pub(super) async fn send_media_request(
        &self,
        r: f::messages::SendMedia,
        business: Option<&str>,
    ) -> Result<Option<Vec<u8>>> {
        use base::InputMedia as M;
        let me = self.me();
        let mut body = Map::new();
        let method = match &r.media {
            M::InputMediaGeoPoint(m) => {
                place(&mut body, &m.geo_point)?;
                "sendLocation"
            }
            M::InputMediaGeoLive(m) => {
                place(&mut body, &m.geo_point)?;
                live(&mut body, m);
                "sendLocation"
            }
            M::InputMediaVenue(m) => {
                venue(&mut body, m)?;
                "sendVenue"
            }
            M::InputMediaContact(m) => {
                body.insert("phone_number".into(), m.phone_number.clone().into());
                body.insert("first_name".into(), m.first_name.clone().into());
                if !m.last_name.is_empty() {
                    body.insert("last_name".into(), m.last_name.clone().into());
                }
                if !m.vcard.is_empty() {
                    body.insert("vcard".into(), m.vcard.clone().into());
                }
                "sendContact"
            }
            M::InputMediaDice(m) => {
                body.insert("emoji".into(), m.emoticon.clone().into());
                "sendDice"
            }
            M::InputMediaPoll(m) => {
                poll(&mut body, m, me)?;
                "sendPoll"
            }
            M::InputMediaTodo(m) => {
                if business.is_none() {
                    return Err(Error::InvalidArgument(CHECKLISTS));
                }
                body.insert("checklist".into(), checklist(&m.todo, me));
                "sendChecklist"
            }
            M::InputMediaGame(m) => {
                let base::InputGame::InputGameShortName(game) = &m.id else {
                    return Err(unsupported("games named by id"));
                };
                body.insert("game_short_name".into(), game.short_name.clone().into());
                "sendGame"
            }
            M::InputMediaWidget(m) => {
                body.insert("html".into(), m.html.clone().into());
                if m.width > 0 {
                    body.insert("width".into(), m.width.into());
                }
                if m.height > 0 {
                    body.insert("height".into(), m.height.into());
                }
                if let Some(data) = &m.public_data {
                    body.insert("public_data".into(), data.clone().into());
                }
                if let Some(data) = &m.chat_data {
                    body.insert("chat_data".into(), data.clone().into());
                }
                flag(&mut body, "allow_queries", m.allow_queries);
                flag(&mut body, "is_shared", m.shared_widget);
                widget_caption(&mut body, &r, me);
                "sendWidget"
            }
            M::InputMediaWidgetId(m) => {
                body.insert("widget_id".into(), m.widget_id.to_string().into());
                if let Some(data) = &m.chat_data {
                    body.insert("chat_data".into(), data.clone().into());
                }
                widget_caption(&mut body, &r, me);
                "sendSharedWidget"
            }
            _ => return Ok(None),
        };
        self.media_send_fields(&mut body, &r, business)?;
        let sent = self.call(method, Value::Object(body)).await?;
        Ok(Some(self.updates(vec![sent], false)?.to_bytes()))
    }

    /// The fields every send of `messages.sendMedia` shares: the chat, business connection, flags, reply, keyboard
    /// and effect. Scheduling, sending as another chat or to quick replies, paid messages and suggested posts have
    /// no Bot API counterpart here and fail.
    fn media_send_fields(
        &self,
        body: &mut Map<String, Value>,
        r: &f::messages::SendMedia,
        business: Option<&str>,
    ) -> Result<()> {
        if r.schedule_date.is_some() {
            return Err(unsupported("scheduled messages"));
        }
        if r.send_as.is_some() || r.quick_reply_shortcut.is_some() {
            return Err(unsupported("sending as another chat or to quick replies"));
        }
        if r.allow_paid_stars.is_some() || r.suggested_post.is_some() {
            return Err(unsupported("paid messages and suggested posts"));
        }
        let me = self.me();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        if let Some(id) = business {
            body.insert("business_connection_id".into(), id.into());
        }
        options(body, r.silent, r.noforwards, r.allow_paid_floodskip);
        if let Some(reply) = &r.reply_to {
            reply_parameters(body, reply, me)?;
        }
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        if let Some(effect) = r.effect {
            body.insert("message_effect_id".into(), effect.to_string().into());
        }
        Ok(())
    }

    /// `send_message_draft` and `send_rich_message_draft`: `sendMessageDraft` or `sendRichMessageDraft` for the draft
    /// action of `messages.setTyping`, which the core bridge sends as a plain chat action.
    pub(crate) async fn send_message_draft(&self, r: f::messages::SetTyping) -> Result<()> {
        use base::SendMessageAction as A;
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        if let Some(top) = r.top_msg_id {
            body.insert("message_thread_id".into(), top.into());
        }
        let (method, can_stop, keep_on_stop) = match &r.action {
            A::SendMessageTextDraftAction(a) => {
                body.insert("draft_id".into(), a.random_id.into());
                let (draft, entities) = text(&a.text, me);
                body.insert("text".into(), draft);
                if let Some(entities) = entities {
                    body.insert("entities".into(), entities);
                }
                ("sendMessageDraft", a.can_stop, a.keep_on_stop)
            }
            A::InputSendMessageRichMessageDraftAction(a) => {
                body.insert("draft_id".into(), a.random_id.into());
                body.insert("rich_message".into(), rich_message(&a.rich_message)?);
                ("sendRichMessageDraft", a.can_stop, a.keep_on_stop)
            }
            _ => return Err(unsupported("this chat action")),
        };
        flag(&mut body, "can_stop", can_stop);
        flag(&mut body, "keep_on_stop", keep_on_stop);
        self.call(method, Value::Object(body)).await?;
        Ok(())
    }

    /// `send_rich_message`: `sendRichMessage`. The core bridge refuses a `messages.sendMessage` with a rich message,
    /// so the friendly method comes here with its options. The message returned carries the text of the rich
    /// message; a rich message written in HTML or Markdown isn't parsed back into blocks, as over MTProto.
    pub(crate) async fn send_rich_message(
        &self,
        client: &Client,
        chat: &PeerRef,
        rich: base::InputRichMessage,
        options: SendOptions,
    ) -> Result<Message> {
        let chat_id = match chat {
            PeerRef::Id(id) => *id,
            other => client.resolve(other).await?.0,
        };
        let mut body = self.friendly_send_fields(client, chat_id, &options).await?;
        body.insert("rich_message".into(), rich_message(&rich)?);
        let sent = self.call("sendRichMessage", Value::Object(body)).await?;
        let (text, sent_rich) = RichMessage::sent(rich);
        let mut peers = Peers::new(self.me());
        let mut raw = self.sent_message(sent, &mut peers)?;
        if raw.message.is_empty() {
            raw.message = text;
        }
        let updates = updates_of(vec![convert::message_update(raw, false)], peers);
        let updates = base::Updates::Updates(Box::new(updates));
        let mut message = Message::from_updates(client, updates, Message::request(chat_id, String::new(), None))?;
        if message.rich_message.is_none() {
            message.rich_message = sent_rich;
        }
        Ok(message)
    }

    /// The fields a friendly send's options give: the chat, topic, business connection, flags, reply, keyboard and
    /// effect. The ones with no Bot API counterpart here fail.
    async fn friendly_send_fields(
        &self,
        client: &Client,
        chat_id: i64,
        options: &SendOptions,
    ) -> Result<Map<String, Value>> {
        if options.schedule_date.is_some() {
            return Err(unsupported("scheduled messages"));
        }
        if options.paid_message_star_count.is_some() || options.suggested_post_parameters.is_some() {
            return Err(unsupported("paid messages and suggested posts"));
        }
        if options.direct_messages_topic_id.is_some() {
            return Err(unsupported("sending into a direct messages topic"));
        }
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        if let Some(id) = &options.business_connection_id {
            body.insert("business_connection_id".into(), id.clone().into());
        }
        if let Some(thread) = options.thread() {
            body.insert("message_thread_id".into(), thread.into());
        }
        let (silent, protect, paid) =
            (options.disable_notification, options.protect_content, options.allow_paid_broadcast);
        self::options(&mut body, silent, protect, paid);
        if let Some(reply) = &options.reply_to {
            reply_parameters(&mut body, &reply.write(client, chat_id).await?, me)?;
        }
        if let Some(markup) = ReplyMarkup::write_option(options.reply_markup.as_ref(), client).await? {
            body.insert("reply_markup".into(), convert::markup_json(&markup, me)?);
        }
        if let Some(effect) = options.message_effect_id {
            body.insert("message_effect_id".into(), effect.to_string().into());
        }
        Ok(body)
    }

    /// `copy_messages`: one `copyMessages` for all of them, where the TL forward that drops the author would copy
    /// them one by one and answer with no ids. The Bot API wants the ids in increasing order and leaves out the
    /// messages it can't copy; the copies' ids come back in the order of `r.id` when none was left out, else in
    /// increasing order of the originals.
    pub(crate) async fn copy_messages(&self, r: &f::messages::ForwardMessages) -> Result<Vec<i32>> {
        if r.schedule_date.is_some() || r.send_as.is_some() || r.quick_reply_shortcut.is_some() {
            return Err(unsupported("scheduled copies, copies as another chat or to quick replies"));
        }
        if r.reply_to.is_some() || r.effect.is_some() || r.allow_paid_floodskip || r.video_timestamp.is_some() {
            return Err(unsupported("replies, effects, paid broadcasts and video start times in batch copies"));
        }
        if r.allow_paid_stars.is_some() || r.suggested_post.is_some() {
            return Err(unsupported("paid messages and suggested posts"));
        }
        let to = self.chat_id(&r.to_peer)?;
        let mut ids = r.id.clone();
        ids.sort_unstable();
        ids.dedup();
        let mut body = Map::new();
        body.insert("chat_id".into(), to.into());
        body.insert("from_chat_id".into(), self.chat_id(&r.from_peer)?.into());
        body.insert("message_ids".into(), ids.clone().into());
        if let Some(top) = r.top_msg_id {
            body.insert("message_thread_id".into(), top.into());
        }
        options(&mut body, r.silent, r.noforwards, false);
        flag(&mut body, "remove_caption", r.drop_media_captions);
        let copied = self.call("copyMessages", Value::Object(body)).await?;
        let copied: Vec<bot::MessageId> = serde_json::from_value(copied).map_err(invalid)?;
        let copies: Vec<i32> = copied.iter().map(|m| int(m.message_id)).collect();
        for &id in &copies {
            let mut raw = types::Message::new(id, get_peer(to), int(now()), String::new());
            raw.out = true;
            self.saw(&raw);
        }
        if copies.len() != ids.len() {
            return Ok(copies);
        }
        Ok(r.id.iter().filter_map(|id| copies.get(ids.binary_search(id).ok()?).copied()).collect())
    }
}

/// A widget's caption, from the text of the request, and whether it shows above the widget.
fn widget_caption(body: &mut Map<String, Value>, r: &f::messages::SendMedia, me: i64) {
    if !r.message.is_empty() {
        put_text(body, ("caption", "caption_entities"), &r.message, r.entities.as_deref(), me);
    }
    flag(body, "show_caption_above_media", r.invert_media);
}

/// A poll's question, options and settings, as `sendPoll` takes them.
fn poll(body: &mut Map<String, Value>, m: &types::InputMediaPoll, me: i64) -> Result<()> {
    let base::Poll::Poll(p) = &m.poll;
    let (question, entities) = text(&p.question, me);
    body.insert("question".into(), question);
    if let Some(entities) = entities {
        body.insert("question_entities".into(), entities);
    }
    let options = p.answers.iter().map(|answer| poll_option(answer, me)).collect::<Result<Vec<_>>>()?;
    body.insert("options".into(), Value::Array(options));
    // The Bot API makes a poll anonymous and lets voters change their vote unless told otherwise.
    body.insert("is_anonymous".into(), (!p.public_voters).into());
    body.insert("type".into(), if p.quiz { "quiz" } else { "regular" }.into());
    body.insert("allows_revoting".into(), (!p.revoting_disabled).into());
    flag(body, "allows_multiple_answers", p.multiple_choice);
    flag(body, "shuffle_options", p.shuffle_answers);
    flag(body, "allow_adding_options", p.open_answers);
    flag(body, "hide_results_until_closes", p.hide_results_until_close);
    flag(body, "members_only", p.subscribers_only);
    flag(body, "is_closed", p.closed);
    if let Some(codes) = &p.countries_iso2 {
        body.insert("country_codes".into(), codes.clone().into());
    }
    if let Some(period) = p.close_period {
        body.insert("open_period".into(), period.into());
    }
    if let Some(date) = p.close_date {
        body.insert("close_date".into(), date.into());
    }
    if let Some(ids) = &m.correct_answers {
        body.insert("correct_option_ids".into(), ids.clone().into());
    }
    if let Some(solution) = &m.solution {
        let entities = m.solution_entities.as_deref();
        put_text(body, ("explanation", "explanation_entities"), solution, entities, me);
    }
    if let Some(media) = &m.attached_media {
        body.insert("media".into(), poll_media(media)?);
    }
    if let Some(media) = &m.solution_media {
        body.insert("explanation_media".into(), poll_media(media)?);
    }
    Ok(())
}

fn poll_option(answer: &base::PollAnswer, me: i64) -> Result<Value> {
    let (option, media) = match answer {
        base::PollAnswer::InputPollAnswer(a) => (&a.text, a.media.as_ref()),
        base::PollAnswer::PollAnswer(a) => (&a.text, None),
    };
    let mut out = Map::new();
    let (option, entities) = text(option, me);
    out.insert("text".into(), option);
    if let Some(entities) = entities {
        out.insert("text_entities".into(), entities);
    }
    if let Some(media) = media {
        out.insert("media".into(), poll_media(media)?);
    }
    Ok(Value::Object(out))
}

/// Media of a poll's description, explanation or option: a place. A file would have to be uploaded over MTProto
/// first, which a client in Bot API mode can't do, and the Bot API's `InputPollMedia` has no link.
fn poll_media(media: &base::InputMedia) -> Result<Value> {
    let mut out = Map::new();
    match media {
        base::InputMedia::InputMediaGeoPoint(m) => {
            out.insert("type".into(), "location".into());
            place(&mut out, &m.geo_point)?;
        }
        base::InputMedia::InputMediaVenue(m) => {
            out.insert("type".into(), "venue".into());
            venue(&mut out, m)?;
        }
        base::InputMedia::InputMediaWebPage(_) => return Err(unsupported("links as poll media")),
        _ => return Err(unsupported("files in polls")),
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::bot_api::bridge::Unsupported;

    fn text(text: &str) -> base::TextWithEntities {
        let text = types::TextWithEntities { text: text.into(), entities: Vec::new() };
        base::TextWithEntities::TextWithEntities(Box::new(text))
    }

    #[test]
    fn a_quiz_keeps_its_settings_and_explanation() {
        let answer = |t: &str| {
            base::PollAnswer::InputPollAnswer(Box::new(types::InputPollAnswer { text: text(t), media: None }))
        };
        let poll = types::Poll {
            id: 0,
            closed: false,
            public_voters: true,
            multiple_choice: false,
            quiz: true,
            open_answers: false,
            revoting_disabled: true,
            shuffle_answers: true,
            hide_results_until_close: false,
            creator: false,
            subscribers_only: false,
            question: text("2 + 2?"),
            answers: vec![answer("3"), answer("4")],
            close_period: Some(30),
            close_date: None,
            countries_iso2: None,
            hash: 0,
        };
        let media = types::InputMediaPoll {
            poll: base::Poll::Poll(Box::new(poll)),
            correct_answers: Some(vec![1]),
            attached_media: None,
            solution: Some("Math".into()),
            solution_entities: Some(Vec::new()),
            solution_media: None,
        };
        let mut body = Map::new();
        super::poll(&mut body, &media, 9).unwrap();
        let expected = json!({
            "question": "2 + 2?",
            "options": [{ "text": "3" }, { "text": "4" }],
            "is_anonymous": false,
            "type": "quiz",
            "allows_revoting": false,
            "shuffle_options": true,
            "open_period": 30,
            "correct_option_ids": [1],
            "explanation": "Math",
        });
        assert_eq!(Value::Object(body), expected);
    }

    #[test]
    fn poll_media_takes_places_only() {
        let page = types::InputMediaWebPage {
            force_large_media: false,
            force_small_media: false,
            optional: false,
            url: "https://example.com".into(),
        };
        let link = base::InputMedia::InputMediaWebPage(Box::new(page));
        let what = |media: &base::InputMedia| {
            poll_media(media).unwrap_err().downcast_ref::<Unsupported>().map(Unsupported::what)
        };
        assert_eq!(what(&link), Some("links as poll media"));
        let dice = base::InputMedia::InputMediaDice(Box::new(types::InputMediaDice { emoticon: "🎲".into() }));
        assert_eq!(what(&dice), Some("files in polls"));
    }
}
