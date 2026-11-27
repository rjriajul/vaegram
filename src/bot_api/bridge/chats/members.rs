//! Members: bans, restrictions, administrators and their titles, member tags, default permissions, leaving, and
//! reading one member or the administrators.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::super::{Transport, channel_chat_id, unsupported};
use super::{flag, no_updates, put, user_of};
use crate::Client;
use crate::bot_api::client::invalid;
use crate::bot_api::convert::{Peers, int};
use crate::bot_api::raw::types as bot;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::types::{ChatMember, ChatPermissions, ChatPrivileges};
use crate::utils::{get_peer, input_peer_id};

const EDIT_BANNED: u32 = <f::channels::EditBanned as Identifiable>::CONSTRUCTOR_ID;
const EDIT_ADMIN: u32 = <f::channels::EditAdmin as Identifiable>::CONSTRUCTOR_ID;
const EDIT_CHAT_ADMIN: u32 = <f::messages::EditChatAdmin as Identifiable>::CONSTRUCTOR_ID;
const DELETE_CHAT_USER: u32 = <f::messages::DeleteChatUser as Identifiable>::CONSTRUCTOR_ID;
const LEAVE_CHANNEL: u32 = <f::channels::LeaveChannel as Identifiable>::CONSTRUCTOR_ID;
const DEFAULT_RIGHTS: u32 = <f::messages::EditChatDefaultBannedRights as Identifiable>::CONSTRUCTOR_ID;
const MEMBER_TAG: u32 = <f::messages::EditChatParticipantRank as Identifiable>::CONSTRUCTOR_ID;
const GET_PARTICIPANT: u32 = <f::channels::GetParticipant as Identifiable>::CONSTRUCTOR_ID;
const GET_PARTICIPANTS: u32 = <f::channels::GetParticipants as Identifiable>::CONSTRUCTOR_ID;

/// The rights of an administrator, which the friendly type names as the Bot API does.
macro_rules! privileges {
    ($($field:ident),* $(,)?) => {
        fn privileges_of(value: &Value) -> ChatPrivileges {
            ChatPrivileges { $($field: flag(value, stringify!($field)),)* }
        }

        /// Every right, as an owner has them.
        fn all_privileges() -> ChatPrivileges {
            ChatPrivileges { $($field: true,)* }
        }

        fn privileges_json(privileges: &ChatPrivileges, out: &mut Map<String, Value>) {
            $(out.insert(stringify!($field).into(), privileges.$field.into());)*
        }
    };
}

privileges!(
    is_anonymous,
    can_manage_chat,
    can_delete_messages,
    can_manage_video_chats,
    can_restrict_members,
    can_promote_members,
    can_change_info,
    can_invite_users,
    can_post_stories,
    can_edit_stories,
    can_delete_stories,
    can_post_messages,
    can_edit_messages,
    can_pin_messages,
    can_manage_topics,
    can_manage_direct_messages,
    can_manage_tags,
    can_send_welcome_messages,
);

/// What a member without admin rights may do, which the friendly type names as the Bot API does.
macro_rules! permissions {
    ($($field:ident),* $(,)?) => {
        fn permissions_of(value: &Value) -> ChatPermissions {
            ChatPermissions { $($field: flag(value, stringify!($field)),)* }
        }

        /// The Bot API's `ChatPermissions`, every field set.
        fn permissions_json(permissions: &ChatPermissions) -> Value {
            let mut out = Map::new();
            $(out.insert(stringify!($field).into(), permissions.$field.into());)*
            Value::Object(out)
        }
    };
}

permissions!(
    can_send_messages,
    can_send_audios,
    can_send_documents,
    can_send_photos,
    can_send_videos,
    can_send_video_notes,
    can_send_voice_notes,
    can_send_polls,
    can_send_other_messages,
    can_add_web_page_previews,
    can_react_to_messages,
    can_edit_tag,
    can_change_info,
    can_invite_users,
    can_pin_messages,
    can_manage_topics,
);

impl Transport {
    pub(super) async fn answer_chat_members(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            EDIT_BANNED => self.edit_banned(body).await,
            EDIT_ADMIN => self.edit_admin(body).await,
            EDIT_CHAT_ADMIN => self.edit_chat_admin(body).await,
            DELETE_CHAT_USER => self.delete_chat_user(body).await,
            LEAVE_CHANNEL => self.leave_channel(body).await,
            DEFAULT_RIGHTS => self.default_rights(body).await,
            MEMBER_TAG => self.member_tag(body).await,
            GET_PARTICIPANT => self.get_participant(body).await,
            GET_PARTICIPANTS => self.get_participants(body).await,
            _ => return None,
        })
    }

    /// `view_messages` bans, empty rights lift a ban or restriction, any other rights restrict. A chat that posts as
    /// itself can be banned and unbanned, not restricted.
    async fn edit_banned(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::EditBanned::from_bytes(body)?;
        let me = self.me();
        let chat_id = channel_chat_id(&r.channel)?;
        let target = input_peer_id(&r.participant, me).ok_or(Error::InvalidArgument("the request names no member"))?;
        let base::ChatBannedRights::ChatBannedRights(rights) = &r.banned_rights;
        let sender_chat = target < 0;
        let until_date = (rights.until_date != 0).then_some(rights.until_date);
        if rights.view_messages {
            if sender_chat {
                self.call("banChatSenderChat", json!({ "chat_id": chat_id, "sender_chat_id": target })).await?;
            } else {
                let mut body = Map::new();
                body.insert("chat_id".into(), chat_id.into());
                body.insert("user_id".into(), target.into());
                put(&mut body, "until_date", until_date);
                self.call("banChatMember", Value::Object(body)).await?;
            }
        } else if **rights == types::ChatBannedRights::new(rights.until_date) {
            if sender_chat {
                self.call("unbanChatSenderChat", json!({ "chat_id": chat_id, "sender_chat_id": target })).await?;
            } else {
                self.lift(chat_id, target).await?;
            }
        } else if sender_chat {
            return Err(unsupported("restricting a chat that posts as itself"));
        } else {
            let mut body = Map::new();
            body.insert("chat_id".into(), chat_id.into());
            body.insert("user_id".into(), target.into());
            body.insert("permissions".into(), permissions_json(&ChatPermissions::parse(&r.banned_rights)));
            body.insert("use_independent_chat_permissions".into(), true.into());
            put(&mut body, "until_date", until_date);
            self.call("restrictChatMember", Value::Object(body)).await?;
        }
        Ok(no_updates(me))
    }

    /// TL's empty rights lift a ban or a restriction and leave a member in the chat. The Bot API's
    /// `unbanChatMember` removes a member unless told `only_if_banned`, and doesn't lift a restriction, so the
    /// member's status picks the method.
    async fn lift(&self, chat_id: i64, user_id: i64) -> Result<()> {
        let member = self.call("getChatMember", json!({ "chat_id": chat_id, "user_id": user_id })).await?;
        match member["status"].as_str() {
            Some("kicked") => {
                let body = json!({ "chat_id": chat_id, "user_id": user_id, "only_if_banned": true });
                self.call("unbanChatMember", body).await?;
            }
            Some("restricted") => {
                let body = json!({
                    "chat_id": chat_id,
                    "user_id": user_id,
                    "permissions": permissions_json(&ChatPermissions::ALL),
                    "use_independent_chat_permissions": true,
                });
                self.call("restrictChatMember", body).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// `promoteChatMember` with the rights, then the title, which the Bot API sets apart. A demotion drops the
    /// title with the rights.
    async fn edit_admin(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::EditAdmin::from_bytes(body)?;
        let me = self.me();
        let chat_id = channel_chat_id(&r.channel)?;
        let user_id = user_of(&r.user_id, me)?;
        let privileges = ChatPrivileges::parse(&r.admin_rights);
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        body.insert("user_id".into(), user_id.into());
        privileges_json(&privileges, &mut body);
        self.call("promoteChatMember", Value::Object(body)).await?;
        if let Some(title) = r.rank.filter(|_| privileges != ChatPrivileges::default()) {
            self.set_chat_administrator_custom_title(chat_id, user_id, title).await?;
        }
        Ok(no_updates(me))
    }

    /// A basic group's admins have no separate rights: any right makes a full admin, none demotes.
    async fn edit_chat_admin(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditChatAdmin::from_bytes(body)?;
        let user_id = user_of(&r.user_id, self.me())?;
        let body = json!({ "chat_id": r.chat_id.wrapping_neg(), "user_id": user_id, "can_manage_chat": r.is_admin });
        self.call("promoteChatMember", body).await?;
        Ok(true.to_bytes())
    }

    /// Removing the bot itself from a basic group is leaving it; anyone else is removed with `banChatMember`, which
    /// in a basic group leaves no ban behind.
    async fn delete_chat_user(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::DeleteChatUser::from_bytes(body)?;
        let me = self.me();
        let chat_id = r.chat_id.wrapping_neg();
        let user_id = user_of(&r.user_id, me)?;
        if user_id == me {
            self.call("leaveChat", json!({ "chat_id": chat_id })).await?;
        } else {
            let mut body = Map::new();
            body.insert("chat_id".into(), chat_id.into());
            body.insert("user_id".into(), user_id.into());
            if r.revoke_history {
                body.insert("revoke_messages".into(), true.into());
            }
            self.call("banChatMember", Value::Object(body)).await?;
        }
        Ok(no_updates(me))
    }

    async fn leave_channel(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::LeaveChannel::from_bytes(body)?;
        self.call("leaveChat", json!({ "chat_id": channel_chat_id(&r.channel)? })).await?;
        Ok(no_updates(self.me()))
    }

    async fn default_rights(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditChatDefaultBannedRights::from_bytes(body)?;
        let body = json!({
            "chat_id": self.chat_id(&r.peer)?,
            "permissions": permissions_json(&ChatPermissions::parse(&r.banned_rights)),
            "use_independent_chat_permissions": true,
        });
        self.call("setChatPermissions", body).await?;
        Ok(no_updates(self.me()))
    }

    /// An empty tag removes it, in TL and in the Bot API alike.
    async fn member_tag(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditChatParticipantRank::from_bytes(body)?;
        let me = self.me();
        let user_id = input_peer_id(&r.participant, me).ok_or(Error::InvalidArgument("the request names no member"))?;
        let body = json!({ "chat_id": self.chat_id(&r.peer)?, "user_id": user_id, "tag": r.rank });
        self.call("setChatMemberTag", body).await?;
        Ok(no_updates(me))
    }

    /// `getChatMember` answers a user who isn't in the chat as `left` where TL fails with `USER_NOT_PARTICIPANT`;
    /// both read back as a member who left. A chat that posts as itself can't be looked up.
    async fn get_participant(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::GetParticipant::from_bytes(body)?;
        let chat_id = channel_chat_id(&r.channel)?;
        let user_id = match input_peer_id(&r.participant, self.me()) {
            Some(id) if id > 0 => id,
            Some(_) => return Err(unsupported("reading a chat that posts as itself as a member")),
            None => return Err(Error::InvalidArgument("the request names no member")),
        };
        let (participant, users) = self.member(chat_id, user_id).await?;
        Ok(types::channels::ChannelParticipant { participant, chats: Vec::new(), users }.to_bytes())
    }

    /// Only the administrators can be listed: the Bot API has no member list. The whole list comes in one request,
    /// and the page asked for is cut from it.
    async fn get_participants(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::GetParticipants::from_bytes(body)?;
        if !matches!(r.filter, base::ChannelParticipantsFilter::ChannelParticipantsAdmins) {
            return Err(unsupported("listing members other than the administrators"));
        }
        let (admins, users) = self.admins(channel_chat_id(&r.channel)?).await?;
        let count = i32::try_from(admins.len()).unwrap_or(i32::MAX);
        let skip = usize::try_from(r.offset).unwrap_or(0);
        let take = usize::try_from(r.limit).ok().filter(|&limit| limit > 0).unwrap_or(usize::MAX);
        let participants = admins.into_iter().skip(skip).take(take).collect();
        Ok(types::channels::ChannelParticipants { count, participants, chats: Vec::new(), users }.to_bytes())
    }

    /// One member as TL describes a channel's, with the user it names.
    async fn member(&self, chat_id: i64, user_id: i64) -> Result<(base::ChannelParticipant, Vec<base::User>)> {
        let value = self.call("getChatMember", json!({ "chat_id": chat_id, "user_id": user_id })).await?;
        let mut peers = Peers::new(self.me());
        let participant = participant(&value, &mut peers)?;
        Ok((participant, peers.users))
    }

    /// Every owner and administrator, bots included, with the users they name.
    async fn admins(&self, chat_id: i64) -> Result<(Vec<base::ChannelParticipant>, Vec<base::User>)> {
        let body = json!({ "chat_id": chat_id, "return_bots": true });
        let Value::Array(list) = self.call("getChatAdministrators", body).await? else {
            return Err(Error::Unexpected("getChatAdministrators returned no list"));
        };
        let mut peers = Peers::new(self.me());
        let admins = list.iter().map(|admin| participant(admin, &mut peers)).collect::<Result<_>>()?;
        Ok((admins, peers.users))
    }

    /// `get_chat_member` in any chat: `getChatMember` reads a basic group's member too, where TL lists them all.
    pub(crate) async fn get_chat_member(&self, client: &Client, chat_id: i64, user_id: i64) -> Result<ChatMember> {
        let (participant, users) = self.member(chat_id, user_id).await?;
        Ok(ChatMember::parse_channel(&participant, &Entities::new(client, users, Vec::new())))
    }

    /// `get_chat_administrators` in any chat, bots included.
    pub(crate) async fn get_chat_administrators(&self, client: &Client, chat_id: i64) -> Result<Vec<ChatMember>> {
        let (admins, users) = self.admins(chat_id).await?;
        let entities = Entities::new(client, users, Vec::new());
        Ok(admins.iter().map(|admin| ChatMember::parse_channel(admin, &entities)).collect())
    }

    /// `get_chat_members_count`: TL reads it from the full chat, which the Bot API's `getChat` doesn't carry.
    pub(crate) async fn get_chat_member_count(&self, chat_id: i64) -> Result<i32> {
        let count = self.call("getChatMemberCount", json!({ "chat_id": chat_id })).await?;
        count.as_i64().map(int).ok_or(Error::Unexpected("getChatMemberCount returned no number"))
    }

    /// `set_chat_administrator_custom_title`: the Bot API sets the title alone and keeps the rights.
    pub(crate) async fn set_chat_administrator_custom_title(
        &self,
        chat_id: i64,
        user_id: i64,
        custom_title: String,
    ) -> Result<()> {
        let body = json!({ "chat_id": chat_id, "user_id": user_id, "custom_title": custom_title });
        self.call("setChatAdministratorCustomTitle", body).await?;
        Ok(())
    }
}

/// A Bot API `ChatMember` as the TL participant of a channel, its user added to `peers`. The Bot API gives no
/// join date, inviter, promoter or restrictor: dates are 0, the inviter is unset, and the promoter and restrictor
/// are 0, which the friendly member reads as `None`. A member's tag is its TL rank.
fn participant(value: &Value, peers: &mut Peers) -> Result<base::ChannelParticipant> {
    use base::ChannelParticipant as P;
    let user = bot::User::deserialize(&value["user"]).map_err(invalid)?;
    peers.user(&user);
    let id = user.id;
    let until_date = value["until_date"].as_i64().map(int).filter(|&date| date != 0);
    let rank = |key: &str| value[key].as_str().map(str::to_owned);
    let banned = |rights: types::ChatBannedRights, left: bool| {
        let rights = base::ChatBannedRights::ChatBannedRights(Box::new(rights));
        let mut banned = types::ChannelParticipantBanned::new(get_peer(id), 0, 0, rights);
        banned.left = left;
        banned.rank = rank("tag");
        P::ChannelParticipantBanned(Box::new(banned))
    };
    Ok(match value["status"].as_str().unwrap_or_default() {
        "creator" => {
            let privileges = ChatPrivileges { is_anonymous: flag(value, "is_anonymous"), ..all_privileges() };
            let mut creator = types::ChannelParticipantCreator::new(id, privileges.write());
            creator.rank = rank("custom_title");
            P::ChannelParticipantCreator(Box::new(creator))
        }
        "administrator" => {
            // `self` and `inviter_id` share a flag, so both stay unset: a set `self` would be read back 8 bytes
            // short.
            let mut admin = types::ChannelParticipantAdmin::new(id, 0, 0, privileges_of(value).write());
            admin.can_edit = flag(value, "can_be_edited");
            admin.rank = rank("custom_title");
            P::ChannelParticipantAdmin(Box::new(admin))
        }
        "restricted" => banned(permissions_of(value).write(until_date.unwrap_or(0)), !flag(value, "is_member")),
        "kicked" => {
            let mut rights = types::ChatBannedRights::new(until_date.unwrap_or(0));
            rights.view_messages = true;
            banned(rights, true)
        }
        "left" => P::ChannelParticipantLeft(Box::new(types::ChannelParticipantLeft { peer: get_peer(id) })),
        // A member, or a status newer than this mapping, which reads best as a plain member.
        _ => {
            let mut member = types::ChannelParticipant::new(id, 0);
            member.subscription_until_date = until_date;
            member.rank = rank("tag");
            P::ChannelParticipant(Box::new(member))
        }
    })
}
