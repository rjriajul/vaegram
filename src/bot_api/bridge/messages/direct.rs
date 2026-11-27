//! Requests that map one to one onto a Bot API method: reactions, suggested posts, game scores, profile audios and
//! emoji statuses.

use serde_json::{Map, Value, json};

use super::{content, empty_updates};
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::client::invalid;
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::{Error, Result};
use crate::raw::{Serializable, base, functions as f, types};
use crate::utils::input_peer_id;

impl Transport {
    /// `send_reaction`: `setMessageReaction`. No reaction removes the bot's own; whether it goes to the recently used
    /// ones has no Bot API counterpart.
    pub(super) async fn send_reaction(&self, r: f::messages::SendReaction) -> Result<Vec<u8>> {
        let reactions: Vec<Value> = r.reaction.iter().flatten().filter_map(reaction).collect();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("message_id".into(), r.msg_id.into());
        body.insert("reaction".into(), Value::Array(reactions));
        if r.big {
            body.insert("is_big".into(), true.into());
        }
        self.call("setMessageReaction", Value::Object(body)).await?;
        Ok(empty_updates(self.me()).to_bytes())
    }

    /// `delete_message_reaction`: `deleteMessageReaction`, for the reaction of a user or of a chat.
    pub(super) async fn delete_reaction(&self, r: f::messages::DeleteParticipantReaction) -> Result<Vec<u8>> {
        let mut body = self.reactor(&r.peer, &r.participant)?;
        body.insert("message_id".into(), r.msg_id.into());
        self.call("deleteMessageReaction", Value::Object(body)).await?;
        Ok(empty_updates(self.me()).to_bytes())
    }

    /// `delete_all_message_reactions`: `deleteAllMessageReactions`.
    pub(super) async fn delete_reactions(&self, r: f::messages::DeleteParticipantReactions) -> Result<Vec<u8>> {
        let body = self.reactor(&r.peer, &r.participant)?;
        self.call("deleteAllMessageReactions", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    /// The chat, and the user or chat whose reactions go: `user_id` for a user, `actor_chat_id` for a chat.
    fn reactor(&self, peer: &base::InputPeer, participant: &base::InputPeer) -> Result<Map<String, Value>> {
        let actor = input_peer_id(participant, self.me()).ok_or(Error::InvalidArgument("the request names no user"))?;
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(peer)?.into());
        let key = if actor > 0 { "user_id" } else { "actor_chat_id" };
        body.insert(key.into(), actor.into());
        Ok(body)
    }

    /// `approve_suggested_post` and `decline_suggested_post`: `approveSuggestedPost` or `declineSuggestedPost`.
    pub(super) async fn suggested_post(&self, r: f::messages::ToggleSuggestedPostApproval) -> Result<Vec<u8>> {
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("message_id".into(), r.msg_id.into());
        let method = if r.reject {
            if let Some(comment) = r.reject_comment {
                body.insert("comment".into(), comment.into());
            }
            "declineSuggestedPost"
        } else {
            if let Some(date) = r.schedule_date {
                body.insert("send_date".into(), date.into());
            }
            "approveSuggestedPost"
        };
        self.call(method, Value::Object(body)).await?;
        Ok(empty_updates(self.me()).to_bytes())
    }

    /// `set_game_score`: `setGameScore`. Unless the edit is turned off, the answer holds the game message.
    pub(super) async fn set_game_score(&self, r: f::messages::SetGameScore, business: Option<&str>) -> Result<Vec<u8>> {
        if business.is_some() {
            return Err(unsupported("game scores in business chats"));
        }
        let me = self.me();
        let user_id = convert::input_user_id(&r.user_id, me).ok_or(Error::InvalidArgument("no such user"))?;
        let mut body = Map::new();
        body.insert("user_id".into(), user_id.into());
        body.insert("score".into(), r.score.into());
        if r.force {
            body.insert("force".into(), true.into());
        }
        if !r.edit_message {
            body.insert("disable_edit_message".into(), true.into());
        }
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("message_id".into(), r.id.into());
        let edited = self.call("setGameScore", Value::Object(body)).await?;
        let updates = match edited {
            Value::Object(_) => self.updates(vec![edited], true)?,
            _ => empty_updates(me),
        };
        Ok(updates.to_bytes())
    }

    /// `get_game_high_scores`: `getGameHighScores`, with the users it names as `min` users.
    pub(super) async fn get_high_scores(&self, r: f::messages::GetGameHighScores) -> Result<Vec<u8>> {
        let me = self.me();
        let user_id = convert::input_user_id(&r.user_id, me).ok_or(Error::InvalidArgument("no such user"))?;
        let body = json!({ "user_id": user_id, "chat_id": self.chat_id(&r.peer)?, "message_id": r.id });
        let scores = self.call("getGameHighScores", body).await?;
        let scores: Vec<bot::GameHighScore> = serde_json::from_value(scores).map_err(invalid)?;
        let mut peers = Peers::new(me);
        let scores: Vec<base::HighScore> = scores
            .iter()
            .map(|s| {
                peers.user(&s.user);
                let score = types::HighScore { pos: int(s.position), user_id: s.user.id, score: int(s.score) };
                base::HighScore::HighScore(Box::new(score))
            })
            .collect();
        let scores = types::messages::HighScores { scores, users: peers.users };
        Ok(base::messages::HighScores::HighScores(Box::new(scores)).to_bytes())
    }

    /// `get_user_profile_audios`: `getUserProfileAudios`. An audio whose file id doesn't decode is left out, and the
    /// total still counts it.
    pub(super) async fn get_saved_music(&self, r: f::users::GetSavedMusic) -> Result<Vec<u8>> {
        let user_id = convert::input_user_id(&r.id, self.me()).ok_or(Error::InvalidArgument("no such user"))?;
        let mut body = Map::new();
        body.insert("user_id".into(), user_id.into());
        if r.offset > 0 {
            body.insert("offset".into(), r.offset.into());
        }
        body.insert("limit".into(), r.limit.into());
        let audios = self.call("getUserProfileAudios", Value::Object(body)).await?;
        let audios: bot::UserProfileAudios = serde_json::from_value(audios).map_err(invalid)?;
        let documents = audios.audios.iter().filter_map(content::audio_document).collect();
        let music = types::users::SavedMusic { count: int(audios.total_count), documents };
        Ok(base::users::SavedMusic::SavedMusic(Box::new(music)).to_bytes())
    }

    /// `set_user_emoji_status`: `setUserEmojiStatus`. A collectible status has no Bot API counterpart.
    pub(super) async fn set_emoji_status(&self, r: f::bots::UpdateUserEmojiStatus) -> Result<Vec<u8>> {
        let user_id = convert::input_user_id(&r.user_id, self.me()).ok_or(Error::InvalidArgument("no such user"))?;
        let mut body = Map::new();
        body.insert("user_id".into(), user_id.into());
        match &r.emoji_status {
            base::EmojiStatus::EmojiStatusEmpty => {}
            base::EmojiStatus::EmojiStatus(status) => {
                body.insert("emoji_status_custom_emoji_id".into(), status.document_id.to_string().into());
                if let Some(until) = status.until {
                    body.insert("emoji_status_expiration_date".into(), until.into());
                }
            }
            _ => return Err(unsupported("collectible emoji statuses")),
        }
        self.call("setUserEmojiStatus", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }
}

/// A reaction as the Bot API's `ReactionType`; `None` for the empty one.
fn reaction(reaction: &base::Reaction) -> Option<Value> {
    Some(match reaction {
        base::Reaction::ReactionEmoji(r) => json!({ "type": "emoji", "emoji": r.emoticon }),
        base::Reaction::ReactionCustomEmoji(r) => {
            json!({ "type": "custom_emoji", "custom_emoji_id": r.document_id.to_string() })
        }
        base::Reaction::ReactionPaid => json!({ "type": "paid" }),
        base::Reaction::ReactionEmpty => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactions_become_bot_api_reaction_types() {
        let emoji = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let custom = base::Reaction::ReactionCustomEmoji(types::ReactionCustomEmoji { document_id: 5 });
        assert_eq!(reaction(&emoji), Some(json!({ "type": "emoji", "emoji": "👍" })));
        assert_eq!(reaction(&custom), Some(json!({ "type": "custom_emoji", "custom_emoji_id": "5" })));
        assert_eq!(reaction(&base::Reaction::ReactionPaid), Some(json!({ "type": "paid" })));
        assert_eq!(reaction(&base::Reaction::ReactionEmpty), None);
    }
}
