//! Messages: sending what isn't a file upload, editing, deleting, reactions, polls, checklists, ephemeral and
//! suggested posts, rich messages and games.
//!
//! A request sent through a business connection comes wrapped in `invokeWithBusinessConnection`, which the bridge's
//! `business_call` reads; the wrapper is answered here only when the request inside belongs to this group, so the
//! other groups still get theirs.
//!
//! Some friendly methods send a TL request the core bridge answers with its own mapping or refuses, such as drafts
//! (`messages.setTyping`, a plain chat action to the core bridge), rich messages and copies of several messages:
//! those hand their request to the `pub(crate)` methods here in a branch of their own.

pub(crate) mod content;
mod direct;
mod edit;
mod ephemeral;
mod json;
mod send;

use super::{Transport, WITH_BUSINESS, business_call, constructor};
use crate::bot_api::convert::{Peers, int};
use crate::errors::Result;
use crate::raw::{Deserializable, Identifiable, base, functions as f, types};
use crate::session::internals::now;

const SEND_MEDIA: u32 = <f::messages::SendMedia as Identifiable>::CONSTRUCTOR_ID;
const SEND_REACTION: u32 = <f::messages::SendReaction as Identifiable>::CONSTRUCTOR_ID;
const DELETE_REACTION: u32 = <f::messages::DeleteParticipantReaction as Identifiable>::CONSTRUCTOR_ID;
const DELETE_REACTIONS: u32 = <f::messages::DeleteParticipantReactions as Identifiable>::CONSTRUCTOR_ID;
const SUGGESTED_POST: u32 = <f::messages::ToggleSuggestedPostApproval as Identifiable>::CONSTRUCTOR_ID;
const EDIT_WIDGET_DATA: u32 = <f::messages::EditMessageWidgetData as Identifiable>::CONSTRUCTOR_ID;
const EDIT_SHARED_WIDGET: u32 = <f::messages::EditWidgetData as Identifiable>::CONSTRUCTOR_ID;
const EDIT_EPHEMERAL: u32 = <f::ephemeral::EditMessage as Identifiable>::CONSTRUCTOR_ID;
const DELETE_EPHEMERAL: u32 = <f::ephemeral::DeleteMessage as Identifiable>::CONSTRUCTOR_ID;
const SET_GAME_SCORE: u32 = <f::messages::SetGameScore as Identifiable>::CONSTRUCTOR_ID;
const GET_HIGH_SCORES: u32 = <f::messages::GetGameHighScores as Identifiable>::CONSTRUCTOR_ID;
const GET_SAVED_MUSIC: u32 = <f::users::GetSavedMusic as Identifiable>::CONSTRUCTOR_ID;
const SET_EMOJI_STATUS: u32 = <f::bots::UpdateUserEmojiStatus as Identifiable>::CONSTRUCTOR_ID;

impl Transport {
    /// Answers a TL request of this group, `None` when `id` isn't one.
    pub(super) async fn answer_messages(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        if id != WITH_BUSINESS {
            return self.message_request(id, body, None).await.transpose();
        }
        // A wrapper that can't be read names no request of this group either.
        let (connection, inner) = business_call(body).ok()?;
        self.message_request(constructor(inner)?, inner, Some(connection.as_str())).await.transpose()
    }

    /// The answer to the request `body`, sent through the business connection `business` if set; `None` when the
    /// request isn't one of this group.
    async fn message_request(&self, id: u32, body: &[u8], business: Option<&str>) -> Result<Option<Vec<u8>>> {
        let answer = match id {
            SEND_MEDIA => return self.send_media_request(f::messages::SendMedia::from_bytes(body)?, business).await,
            SEND_REACTION => self.send_reaction(f::messages::SendReaction::from_bytes(body)?).await?,
            DELETE_REACTION => self.delete_reaction(f::messages::DeleteParticipantReaction::from_bytes(body)?).await?,
            DELETE_REACTIONS => {
                self.delete_reactions(f::messages::DeleteParticipantReactions::from_bytes(body)?).await?
            }
            SUGGESTED_POST => self.suggested_post(f::messages::ToggleSuggestedPostApproval::from_bytes(body)?).await?,
            EDIT_WIDGET_DATA => self.edit_widget_data(f::messages::EditMessageWidgetData::from_bytes(body)?).await?,
            EDIT_SHARED_WIDGET => self.edit_shared_widget(f::messages::EditWidgetData::from_bytes(body)?).await?,
            EDIT_EPHEMERAL => self.edit_ephemeral(f::ephemeral::EditMessage::from_bytes(body)?).await?,
            DELETE_EPHEMERAL => self.delete_ephemeral(f::ephemeral::DeleteMessage::from_bytes(body)?).await?,
            SET_GAME_SCORE => self.set_game_score(f::messages::SetGameScore::from_bytes(body)?, business).await?,
            GET_HIGH_SCORES => self.get_high_scores(f::messages::GetGameHighScores::from_bytes(body)?).await?,
            GET_SAVED_MUSIC => self.get_saved_music(f::users::GetSavedMusic::from_bytes(body)?).await?,
            SET_EMOJI_STATUS => self.set_emoji_status(f::bots::UpdateUserEmojiStatus::from_bytes(body)?).await?,
            _ => return Ok(None),
        };
        Ok(Some(answer))
    }
}

/// The `Updates` of a request that changes no message the bridge can show, such as a reaction.
fn empty_updates(me: i64) -> types::Updates {
    updates_of(Vec::new(), Peers::new(me))
}

fn updates_of(updates: Vec<base::Update>, peers: Peers) -> types::Updates {
    types::Updates { updates, users: peers.users, chats: peers.chats, date: int(now()), seq: 0 }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::*;
    use crate::bot_api::bridge::{Unsupported, reply_parameters};
    use crate::errors::Error;
    use crate::raw::Serializable;

    #[test]
    fn a_business_wrapper_names_its_connection_and_request() {
        let inner = f::messages::SendReaction {
            big: false,
            add_to_recent: true,
            peer: base::InputPeer::InputPeerSelf,
            msg_id: 4,
            reaction: None,
        };
        let quiet = f::InvokeWithoutUpdates { query: inner.clone() };
        let wrapped = f::InvokeWithBusinessConnection { connection_id: "conn".into(), query: quiet }.to_bytes();
        let (connection, request) = business_call(&wrapped).unwrap();
        assert_eq!((connection.as_str(), request), ("conn", &inner.to_bytes()[..]));
        assert!(business_call(&wrapped[..6]).is_err());
    }

    #[test]
    fn replies_name_poll_options_and_refuse_what_has_no_counterpart() {
        let reply = |edit: fn(&mut types::InputReplyToMessage)| {
            let mut r = types::InputReplyToMessage {
                reply_to_msg_id: 5,
                top_msg_id: Some(9),
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
                quote_rich: None,
            };
            edit(&mut r);
            let mut body = Map::new();
            let input = base::InputReplyTo::InputReplyToMessage(Box::new(r));
            reply_parameters(&mut body, &input, 99).map(|()| Value::Object(body))
        };
        let body = reply(|r| r.poll_option = Some(b"opt".to_vec())).unwrap();
        let parameters = json!({ "message_id": 5, "poll_option_id": "opt" });
        assert_eq!(body, json!({ "reply_parameters": parameters, "message_thread_id": 9 }));
        assert!(matches!(reply(|r| r.poll_option = Some(vec![0xff])), Err(Error::InvalidArgument(_))));
        let what = |result: Result<Value>| result.unwrap_err().downcast_ref::<Unsupported>().map(Unsupported::what);
        let rich = |r: &mut types::InputReplyToMessage| r.quote_rich = Some(vec![base::RichText::TextEmpty]);
        assert_eq!(what(reply(rich)), Some("rich quotes in replies"));
        let topic = |r: &mut types::InputReplyToMessage| r.monoforum_peer_id = Some(base::InputPeer::InputPeerSelf);
        assert_eq!(what(reply(topic)), Some("sends in direct messages topics"));
    }
}
