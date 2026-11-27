//! The bot itself and what it sells or manages: bot settings, business accounts, gifts, Stars and payments,
//! stickers, stories, inline and web app answers, managed bots and Passport.
//!
//! Most of it answers TL requests, as the rest of the bridge does. A few friendly methods take their own branch
//! here instead ([`uploads`]): an upload goes in the request of the Bot API method that sends it, and a sticker is
//! named by its Bot API file id, which a TL input document can't give back.

mod business;
mod gifts;
mod payments;
mod settings;
mod stickers;
mod uploads;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::{Transport, WITH_BUSINESS, business_call, constructor};
use crate::bot_api::client::invalid;
use crate::bot_api::convert;
use crate::errors::{Error, Result};
use crate::raw::{Serializable, base, types};
use crate::types::InlineMessageId;

impl Transport {
    /// Answers a TL request of this group, `None` when `id` isn't one. A request through a business connection is
    /// this group's only when the request inside it is.
    pub(super) async fn answer_bots(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        if id != WITH_BUSINESS {
            if let Some(answer) = self.answer_settings(id, body).await {
                return Some(answer);
            }
            if let Some(answer) = self.answer_payments(id, body, None).await {
                return Some(answer);
            }
            if let Some(answer) = self.answer_gifts(id, body, None).await {
                return Some(answer);
            }
            if let Some(answer) = self.answer_stickers(id, body).await {
                return Some(answer);
            }
            return self.answer_connection(id, body).await;
        }
        let (connection, inner) = match business_call(body) {
            Ok(call) => call,
            Err(e) => return Some(Err(e)),
        };
        let id = constructor(inner)?;
        if let Some(answer) = self.answer_business(id, inner, &connection).await {
            return Some(answer);
        }
        if let Some(answer) = self.answer_payments(id, inner, Some(&connection)).await {
            return Some(answer);
        }
        self.answer_gifts(id, inner, Some(&connection)).await
    }

    /// The user an input user names, as the Bot API's `user_id`.
    fn user_id(&self, user: &base::InputUser) -> Result<i64> {
        convert::input_user_id(user, self.me()).ok_or(Error::InvalidArgument("the request names no user"))
    }

    /// The Bot API's `BotCommandScope` of a TL one.
    fn scope_json(&self, scope: &base::BotCommandScope) -> Result<Value> {
        use base::BotCommandScope as S;
        Ok(match scope {
            S::BotCommandScopeDefault => json!({ "type": "default" }),
            S::BotCommandScopeUsers => json!({ "type": "all_private_chats" }),
            S::BotCommandScopeChats => json!({ "type": "all_group_chats" }),
            S::BotCommandScopeChatAdmins => json!({ "type": "all_chat_administrators" }),
            S::BotCommandScopePeer(s) => json!({ "type": "chat", "chat_id": self.chat_id(&s.peer)? }),
            S::BotCommandScopePeerAdmins(s) => {
                json!({ "type": "chat_administrators", "chat_id": self.chat_id(&s.peer)? })
            }
            S::BotCommandScopePeerUser(s) => {
                json!({ "type": "chat_member", "chat_id": self.chat_id(&s.peer)?, "user_id": self.user_id(&s.user_id)? })
            }
        })
    }

    /// The text and entities a gift or a Premium gift carries, as `text` and `text_entities`.
    fn gift_text(&self, out: &mut Map<String, Value>, message: Option<&base::TextWithEntities>) {
        if let Some(base::TextWithEntities::TextWithEntities(m)) = message {
            out.insert("text".into(), m.text.clone().into());
            if !m.entities.is_empty() {
                out.insert("text_entities".into(), convert::entities_json(&m.entities, self.me()));
            }
        }
    }
}

/// A Bot API answer read as `T`, which names only the fields the bridge uses.
fn parse<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(invalid)
}

/// What a TL method answers with when all it says is that it worked.
fn done() -> Vec<u8> {
    true.to_bytes()
}

/// The TL inline message id of a Bot API `inline_message_id`.
fn inline_message_id(id: &str) -> Result<base::InputBotInlineMessageID> {
    Ok(id.parse::<InlineMessageId>()?.0)
}

/// A TL `Updates` that tells of nothing, for the methods whose Bot API counterpart answers `true`.
fn no_updates() -> Vec<u8> {
    types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 }.to_bytes()
}

/// The Bot API's `ChatAdministratorRights` of TL admin rights.
fn admin_rights_json(rights: &base::ChatAdminRights) -> Value {
    let base::ChatAdminRights::ChatAdminRights(r) = rights;
    json!({
        "is_anonymous": r.anonymous,
        "can_manage_chat": r.other,
        "can_delete_messages": r.delete_messages,
        "can_manage_video_chats": r.manage_call,
        "can_restrict_members": r.ban_users,
        "can_promote_members": r.add_admins,
        "can_change_info": r.change_info,
        "can_invite_users": r.invite_users,
        "can_post_stories": r.post_stories,
        "can_edit_stories": r.edit_stories,
        "can_delete_stories": r.delete_stories,
        "can_post_messages": r.post_messages,
        "can_edit_messages": r.edit_messages,
        "can_pin_messages": r.pin_messages,
        "can_manage_topics": r.manage_topics,
        "can_manage_direct_messages": r.manage_direct_messages,
        "can_manage_tags": r.manage_ranks,
        "can_send_welcome_messages": r.manage_welcome_messages,
    })
}

/// TL admin rights of the Bot API's `ChatAdministratorRights`; a right it leaves out is off.
fn admin_rights_tl(rights: &Value) -> types::ChatAdminRights {
    let on = |key: &str| rights[key].as_bool().unwrap_or(false);
    types::ChatAdminRights {
        change_info: on("can_change_info"),
        post_messages: on("can_post_messages"),
        edit_messages: on("can_edit_messages"),
        delete_messages: on("can_delete_messages"),
        ban_users: on("can_restrict_members"),
        invite_users: on("can_invite_users"),
        pin_messages: on("can_pin_messages"),
        add_admins: on("can_promote_members"),
        anonymous: on("is_anonymous"),
        manage_call: on("can_manage_video_chats"),
        other: on("can_manage_chat"),
        manage_topics: on("can_manage_topics"),
        post_stories: on("can_post_stories"),
        edit_stories: on("can_edit_stories"),
        delete_stories: on("can_delete_stories"),
        manage_direct_messages: on("can_manage_direct_messages"),
        manage_ranks: on("can_manage_tags"),
        manage_linked_peers: false,
        manage_welcome_messages: on("can_send_welcome_messages"),
    }
}

#[cfg(test)]
mod tests {
    use super::{admin_rights_json, admin_rights_tl, business_call};
    use crate::raw::{Serializable, base, functions as f, types};

    #[test]
    fn a_business_call_names_its_connection_and_the_request_inside() {
        let read = f::messages::ReadHistory { peer: base::InputPeer::InputPeerSelf, max_id: 5 };
        let quiet = f::InvokeWithoutUpdates { query: read.clone() };
        let call = f::InvokeWithBusinessConnection { connection_id: "conn".into(), query: quiet };
        let bytes = call.to_bytes();
        let (connection, inner) = business_call(&bytes).unwrap();
        assert_eq!((connection.as_str(), inner), ("conn", read.to_bytes().as_slice()));
    }

    #[test]
    fn admin_rights_go_both_ways() {
        let rights = types::ChatAdminRights {
            delete_messages: true,
            manage_ranks: true,
            manage_welcome_messages: true,
            other: true,
            ..Default::default()
        };
        let json = admin_rights_json(&base::ChatAdminRights::ChatAdminRights(Box::new(rights.clone())));
        assert_eq!(json["can_manage_tags"], true);
        assert_eq!(json["can_manage_chat"], true);
        assert_eq!(json["can_post_messages"], false);
        assert_eq!(admin_rights_tl(&json), rights);
    }
}
