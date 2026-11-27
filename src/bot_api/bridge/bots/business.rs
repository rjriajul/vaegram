//! Business accounts: the connection itself, and what the bot does for the account behind it: reading and deleting
//! its messages, its name, username, bio and photo, its gift settings, and its stories.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::parse;
use super::settings::no_photo;
use crate::bot_api::bridge::{Transport, unsupported, updates_bytes};
use crate::bot_api::convert::{Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::Result;
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};

const GET_CONNECTION: u32 = <f::account::GetBotBusinessConnection as Identifiable>::CONSTRUCTOR_ID;
const READ_HISTORY: u32 = <f::messages::ReadHistory as Identifiable>::CONSTRUCTOR_ID;
const DELETE_MESSAGES: u32 = <f::messages::DeleteMessages as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_PROFILE: u32 = <f::account::UpdateProfile as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_USERNAME: u32 = <f::account::UpdateUsername as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_PHOTO: u32 = <f::photos::UpdateProfilePhoto as Identifiable>::CONSTRUCTOR_ID;
const GET_PRIVACY: u32 = <f::account::GetGlobalPrivacySettings as Identifiable>::CONSTRUCTOR_ID;
const SET_PRIVACY: u32 = <f::account::SetGlobalPrivacySettings as Identifiable>::CONSTRUCTOR_ID;
const DELETE_STORIES: u32 = <f::stories::DeleteStories as Identifiable>::CONSTRUCTOR_ID;

/// The rights a business connection gives the bot. A right the Bot API leaves out is off.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Rights {
    can_reply: bool,
    can_read_messages: bool,
    can_delete_sent_messages: bool,
    can_delete_all_messages: bool,
    can_edit_name: bool,
    can_edit_bio: bool,
    can_edit_profile_photo: bool,
    can_edit_username: bool,
    can_change_gift_settings: bool,
    can_view_gifts_and_stars: bool,
    can_convert_gifts_to_stars: bool,
    can_transfer_and_upgrade_gifts: bool,
    can_transfer_stars: bool,
    can_manage_stories: bool,
}

impl Transport {
    /// `account.getBotBusinessConnection`, the one request of this group that goes outside a connection.
    pub(super) async fn answer_connection(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        if id != GET_CONNECTION {
            return None;
        }
        Some(self.get_business_connection(body).await)
    }

    /// Answers a request made through the business connection `connection`, `None` when it isn't of this group.
    pub(super) async fn answer_business(&self, id: u32, body: &[u8], connection: &str) -> Option<Result<Vec<u8>>> {
        Some(match id {
            READ_HISTORY => self.read_business_message(body, connection).await,
            DELETE_MESSAGES => self.delete_business_messages(body, connection).await,
            UPDATE_PROFILE => self.set_business_profile(body, connection).await,
            UPDATE_USERNAME => self.set_business_username(body, connection).await,
            UPDATE_PHOTO => self.remove_business_photo(body, connection).await,
            // The Bot API can't read the account's settings: the friendly method starts from Telegram's defaults.
            GET_PRIVACY => Ok(types::GlobalPrivacySettings::default().to_bytes()),
            SET_PRIVACY => self.set_business_gift_settings(body, connection).await,
            DELETE_STORIES => self.delete_business_stories(body, connection).await,
            _ => return None,
        })
    }

    /// The connection as the `updateBotBusinessConnect` Telegram answers with. Its data center is the client's own
    /// (0 in Bot API mode), so the requests made through it come back to the bridge.
    async fn get_business_connection(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Connection {
            id: String,
            user: bot::User,
            #[serde(default)]
            date: i64,
            #[serde(default)]
            rights: Option<Rights>,
            #[serde(default)]
            is_enabled: bool,
        }
        let r = f::account::GetBotBusinessConnection::from_bytes(body)?;
        let body = json!({ "business_connection_id": r.connection_id });
        let c: Connection = parse(self.call("getBusinessConnection", body).await?)?;
        let mut connection = types::BotBusinessConnection::new(c.id, c.user.id, 0, int(c.date));
        connection.disabled = !c.is_enabled;
        connection.rights = c.rights.map(|r| {
            base::BusinessBotRights::BusinessBotRights(Box::new(types::BusinessBotRights {
                reply: r.can_reply,
                read_messages: r.can_read_messages,
                delete_sent_messages: r.can_delete_sent_messages,
                delete_received_messages: r.can_delete_all_messages,
                edit_name: r.can_edit_name,
                edit_bio: r.can_edit_bio,
                edit_profile_photo: r.can_edit_profile_photo,
                edit_username: r.can_edit_username,
                view_gifts: r.can_view_gifts_and_stars,
                sell_gifts: r.can_convert_gifts_to_stars,
                change_gift_settings: r.can_change_gift_settings,
                transfer_and_upgrade_gifts: r.can_transfer_and_upgrade_gifts,
                transfer_stars: r.can_transfer_stars,
                manage_stories: r.can_manage_stories,
            }))
        });
        let connection = base::BotBusinessConnection::BotBusinessConnection(Box::new(connection));
        let update = types::UpdateBotBusinessConnect { connection, qts: 0 };
        let mut peers = Peers::new(self.me());
        peers.user(&c.user);
        Ok(updates_bytes(vec![base::Update::UpdateBotBusinessConnect(Box::new(update))], peers))
    }

    async fn read_business_message(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::messages::ReadHistory::from_bytes(body)?;
        let body = json!({
            "business_connection_id": connection,
            "chat_id": self.chat_id(&r.peer)?,
            "message_id": r.max_id,
        });
        self.call("readBusinessMessage", body).await?;
        Ok(affected())
    }

    async fn delete_business_messages(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::messages::DeleteMessages::from_bytes(body)?;
        self.call("deleteBusinessMessages", json!({ "business_connection_id": connection, "message_ids": r.id }))
            .await?;
        Ok(affected())
    }

    /// The name and the bio are two Bot API methods; a request may set both.
    async fn set_business_profile(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::account::UpdateProfile::from_bytes(body)?;
        match (r.first_name, r.last_name) {
            (Some(first_name), last_name) => {
                let mut out = Map::new();
                out.insert("business_connection_id".into(), connection.into());
                out.insert("first_name".into(), first_name.into());
                // An empty last name clears it, as the Bot API reads a missing one.
                if let Some(last_name) = last_name.filter(|name| !name.is_empty()) {
                    out.insert("last_name".into(), last_name.into());
                }
                self.call("setBusinessAccountName", Value::Object(out)).await?;
            }
            (None, Some(_)) => return Err(unsupported("setting a business account's last name alone")),
            (None, None) => {}
        }
        if let Some(bio) = r.about {
            self.call("setBusinessAccountBio", json!({ "business_connection_id": connection, "bio": bio })).await?;
        }
        Ok(no_user())
    }

    async fn set_business_username(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::account::UpdateUsername::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("business_connection_id".into(), connection.into());
        // No username removes it.
        if !r.username.is_empty() {
            out.insert("username".into(), r.username.into());
        }
        self.call("setBusinessAccountUsername", Value::Object(out)).await?;
        Ok(no_user())
    }

    async fn remove_business_photo(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::photos::UpdateProfilePhoto::from_bytes(body)?;
        if !matches!(r.id, base::InputPhoto::InputPhotoEmpty) {
            return Err(unsupported("setting an earlier photo as a business account's profile photo"));
        }
        let body = json!({ "business_connection_id": connection, "is_public": r.fallback });
        self.call("removeBusinessAccountProfilePhoto", body).await?;
        Ok(no_photo())
    }

    /// The gift button and the gift types are all the Bot API sets of TL's global privacy settings.
    async fn set_business_gift_settings(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::account::SetGlobalPrivacySettings::from_bytes(body)?;
        let base::GlobalPrivacySettings::GlobalPrivacySettings(settings) = &r.settings;
        let accepted = match &settings.disallowed_gifts {
            Some(base::DisallowedGiftsSettings::DisallowedGiftsSettings(d)) => json!({
                "unlimited_gifts": !d.disallow_unlimited_stargifts,
                "limited_gifts": !d.disallow_limited_stargifts,
                "unique_gifts": !d.disallow_unique_stargifts,
                "premium_subscription": !d.disallow_premium_gifts,
                "gifts_from_channels": !d.disallow_stargifts_from_channels,
            }),
            None => json!({
                "unlimited_gifts": true,
                "limited_gifts": true,
                "unique_gifts": true,
                "premium_subscription": true,
                "gifts_from_channels": true,
            }),
        };
        let body = json!({
            "business_connection_id": connection,
            "show_gift_button": settings.display_gifts_button,
            "accepted_gift_types": accepted,
        });
        self.call("setBusinessAccountGiftSettings", body).await?;
        Ok(r.settings.to_bytes())
    }

    async fn delete_business_stories(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::stories::DeleteStories::from_bytes(body)?;
        for &id in &r.id {
            self.delete_story(connection, id).await?;
        }
        Ok(r.id.to_bytes())
    }

    /// `delete_business_story`, which goes straight here: TL names the account by its user, which the Bot API
    /// doesn't need.
    pub(crate) async fn delete_story(&self, connection: &str, story_id: i32) -> Result<()> {
        self.call("deleteStory", json!({ "business_connection_id": connection, "story_id": story_id })).await?;
        Ok(())
    }
}

/// The TL answer to reading or deleting messages, with no pts: the Bot API numbers nothing.
fn affected() -> Vec<u8> {
    types::messages::AffectedMessages { pts: 0, pts_count: 0 }.to_bytes()
}

/// The TL answer to a profile change: the Bot API returns no user, and the friendly methods read none.
fn no_user() -> Vec<u8> {
    base::User::UserEmpty(types::UserEmpty { id: 0 }).to_bytes()
}
