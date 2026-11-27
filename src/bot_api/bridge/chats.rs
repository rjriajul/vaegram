//! Chats: members, admins, permissions, invite links, join requests, chat settings, forum topics, boosts and
//! verification.
//!
//! The Bot API reads members one at a time and can't list them, gives no dates for joins, promotions or links, and
//! names no one who promoted or restricted a member: the TL answers built here leave those fields at 0 or empty.
//! Friendly methods that would read a basic group's member list over MTProto take their own branch to the Bot API
//! method that answers them directly (`get_chat_member`, `get_chat_administrators`, `get_chat_members_count`).

mod invites;
mod members;
mod settings;
mod topics;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::{Transport, updates_bytes};
use crate::bot_api::client::invalid;
use crate::bot_api::convert::{Peers, input_user_id};
use crate::errors::{Error, Result};
use crate::raw::base;

impl Transport {
    /// Answers a TL request of this group, `None` when `id` isn't one.
    pub(super) async fn answer_chats(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        if let Some(answer) = self.answer_chat_members(id, body).await {
            return Some(answer);
        }
        if let Some(answer) = self.answer_chat_invites(id, body).await {
            return Some(answer);
        }
        if let Some(answer) = self.answer_chat_settings(id, body).await {
            return Some(answer);
        }
        self.answer_chat_topics(id, body).await
    }
}

/// The user an input user names; `me` stands for `inputUserSelf`.
fn user_of(user: &base::InputUser, me: i64) -> Result<i64> {
    input_user_id(user, me).ok_or(Error::InvalidArgument("the request names no user"))
}

/// A Bot API answer read as `T`.
fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(invalid)
}

/// What TL answers a change with when the Bot API only says it was made: `Updates` that tell of nothing.
fn no_updates(me: i64) -> Vec<u8> {
    updates_bytes(Vec::new(), Peers::new(me))
}

/// A Bot API boolean, `false` when it is missing.
fn flag(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap_or(false)
}

/// Sets `key` when `value` is set: the Bot API reads a missing optional field as unset.
fn put<T: Into<Value>>(body: &mut Map<String, Value>, key: &str, value: Option<T>) {
    if let Some(value) = value {
        body.insert(key.into(), value.into());
    }
}
