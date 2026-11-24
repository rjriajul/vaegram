//! The Bot API's types and methods, generated from its published spec by `tools/botapi/gen.py`.

pub mod methods;
pub mod types;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Deserializer, Serialize};

use types::{InaccessibleMessage, MaybeInaccessibleMessage, Message};

/// A chat: its numeric id, or the `@username` of a public chat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatId {
    /// The chat's numeric id.
    Id(i64),
    /// The chat's `@username`.
    Username(String),
}

impl From<i64> for ChatId {
    fn from(id: i64) -> Self {
        ChatId::Id(id)
    }
}

impl From<String> for ChatId {
    fn from(username: String) -> Self {
        ChatId::Username(username)
    }
}

impl From<&str> for ChatId {
    fn from(username: &str) -> Self {
        ChatId::Username(username.to_owned())
    }
}

impl<'de> Deserialize<'de> for MaybeInaccessibleMessage {
    // Both kinds have `chat`, `message_id` and `date`; only `date` being 0 marks the inaccessible one.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let parsed = if value.get("date").and_then(serde_json::Value::as_i64) == Some(0) {
            InaccessibleMessage::deserialize(&value).map(MaybeInaccessibleMessage::InaccessibleMessage)
        } else {
            Message::deserialize(&value).map(|m| MaybeInaccessibleMessage::Message(Box::new(m)))
        };
        Ok(parsed.unwrap_or(MaybeInaccessibleMessage::Unknown(value)))
    }
}
