//! The bot itself: logging out, its commands, name and descriptions, menu button, default administrator rights and
//! profile photo removal, the bots it manages, and its answers to widget, guest and Mini App queries.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{admin_rights_json, admin_rights_tl, done, inline_message_id, parse};
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::Result;
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};

const LOG_OUT: u32 = <f::auth::LogOut as Identifiable>::CONSTRUCTOR_ID;
const WIDGET_ANSWER: u32 = <f::messages::SetBotWidgetQueryAnswer as Identifiable>::CONSTRUCTOR_ID;
const GUEST_RESULT: u32 = <f::messages::SetBotGuestChatResult as Identifiable>::CONSTRUCTOR_ID;
const WEB_APP_RESULT: u32 = <f::messages::SendWebViewResultMessage as Identifiable>::CONSTRUCTOR_ID;
const SAVE_PREPARED: u32 = <f::messages::SavePreparedInlineMessage as Identifiable>::CONSTRUCTOR_ID;
const PREPARE_BUTTON: u32 = <f::bots::RequestWebViewButton as Identifiable>::CONSTRUCTOR_ID;
const EXPORT_TOKEN: u32 = <f::bots::ExportBotToken as Identifiable>::CONSTRUCTOR_ID;
const GET_ACCESS: u32 = <f::bots::GetAccessSettings as Identifiable>::CONSTRUCTOR_ID;
const EDIT_ACCESS: u32 = <f::bots::EditAccessSettings as Identifiable>::CONSTRUCTOR_ID;
const RESET_COMMANDS: u32 = <f::bots::ResetBotCommands as Identifiable>::CONSTRUCTOR_ID;
const GET_COMMANDS: u32 = <f::bots::GetBotCommands as Identifiable>::CONSTRUCTOR_ID;
const SET_INFO: u32 = <f::bots::SetBotInfo as Identifiable>::CONSTRUCTOR_ID;
const GET_INFO: u32 = <f::bots::GetBotInfo as Identifiable>::CONSTRUCTOR_ID;
const SET_MENU_BUTTON: u32 = <f::bots::SetBotMenuButton as Identifiable>::CONSTRUCTOR_ID;
const GET_MENU_BUTTON: u32 = <f::bots::GetBotMenuButton as Identifiable>::CONSTRUCTOR_ID;
const CHANNEL_RIGHTS: u32 = <f::bots::SetBotBroadcastDefaultAdminRights as Identifiable>::CONSTRUCTOR_ID;
const GROUP_RIGHTS: u32 = <f::bots::SetBotGroupDefaultAdminRights as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_PHOTO: u32 = <f::photos::UpdateProfilePhoto as Identifiable>::CONSTRUCTOR_ID;

impl Transport {
    pub(super) async fn answer_settings(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            LOG_OUT => self.log_out().await,
            WIDGET_ANSWER => self.answer_widget_query(body).await,
            GUEST_RESULT => self.answer_guest_query(body).await,
            WEB_APP_RESULT => self.answer_web_app_query(body).await,
            SAVE_PREPARED => self.save_prepared_inline_message(body).await,
            PREPARE_BUTTON => self.save_prepared_keyboard_button(body).await,
            EXPORT_TOKEN => self.export_bot_token(body).await,
            GET_ACCESS => self.get_access_settings(body).await,
            EDIT_ACCESS => self.edit_access_settings(body).await,
            RESET_COMMANDS => self.delete_commands(body).await,
            GET_COMMANDS => self.get_commands(body).await,
            SET_INFO => self.set_bot_info(body).await,
            GET_INFO => self.get_bot_info(body).await,
            SET_MENU_BUTTON => self.set_menu_button(body).await,
            GET_MENU_BUTTON => self.get_menu_button(body).await,
            CHANNEL_RIGHTS => match f::bots::SetBotBroadcastDefaultAdminRights::from_bytes(body) {
                Ok(r) => self.set_default_rights(&r.admin_rights, true).await,
                Err(e) => Err(e.into()),
            },
            GROUP_RIGHTS => match f::bots::SetBotGroupDefaultAdminRights::from_bytes(body) {
                Ok(r) => self.set_default_rights(&r.admin_rights, false).await,
                Err(e) => Err(e.into()),
            },
            UPDATE_PHOTO => self.remove_profile_photo(body).await,
            _ => return None,
        })
    }

    /// The Bot API server refuses the bot for 10 minutes after this; the client forgets its account itself.
    async fn log_out(&self) -> Result<Vec<u8>> {
        self.call("logOut", json!({})).await?;
        Ok(types::auth::LoggedOut { future_auth_token: None }.to_bytes())
    }

    async fn answer_widget_query(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::SetBotWidgetQueryAnswer::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("widget_query_id".into(), r.query_id.to_string().into());
        if let Some(data) = r.result {
            out.insert("data".into(), data.into());
        }
        self.call("answerWidgetQuery", Value::Object(out)).await?;
        Ok(done())
    }

    async fn answer_guest_query(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Sent {
            inline_message_id: String,
        }
        let r = f::messages::SetBotGuestChatResult::from_bytes(body)?;
        let result = convert::inline_result_json(&r.result, self.me())?;
        let body = json!({ "guest_query_id": r.query_id.to_string(), "result": result });
        let sent: Sent = parse(self.call("answerGuestQuery", body).await?)?;
        Ok(inline_message_id(&sent.inline_message_id)?.to_bytes())
    }

    async fn answer_web_app_query(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Sent {
            inline_message_id: Option<String>,
        }
        let r = f::messages::SendWebViewResultMessage::from_bytes(body)?;
        let result = convert::inline_result_json(&r.result, self.me())?;
        let body = json!({ "web_app_query_id": r.bot_query_id, "result": result });
        let sent: Sent = parse(self.call("answerWebAppQuery", body).await?)?;
        let msg_id = match sent.inline_message_id {
            Some(id) => Some(inline_message_id(&id)?),
            None => None,
        };
        Ok(types::WebViewMessageSent { msg_id }.to_bytes())
    }

    async fn save_prepared_inline_message(&self, body: &[u8]) -> Result<Vec<u8>> {
        use base::InlineQueryPeerType as P;
        #[derive(Deserialize)]
        struct Prepared {
            id: String,
            #[serde(default)]
            expiration_date: i64,
        }
        let r = f::messages::SavePreparedInlineMessage::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("user_id".into(), self.user_id(&r.user_id)?.into());
        out.insert("result".into(), convert::inline_result_json(&r.result, self.me())?);
        for kind in r.peer_types.iter().flatten() {
            let key = match kind {
                P::InlineQueryPeerTypePM | P::InlineQueryPeerTypeSameBotPM => "allow_user_chats",
                P::InlineQueryPeerTypeBotPM => "allow_bot_chats",
                P::InlineQueryPeerTypeChat | P::InlineQueryPeerTypeMegagroup => "allow_group_chats",
                P::InlineQueryPeerTypeBroadcast => "allow_channel_chats",
            };
            out.insert(key.into(), true.into());
        }
        let prepared: Prepared = parse(self.call("savePreparedInlineMessage", Value::Object(out)).await?)?;
        let answer =
            types::messages::BotPreparedInlineMessage { id: prepared.id, expire_date: int(prepared.expiration_date) };
        Ok(answer.to_bytes())
    }

    async fn save_prepared_keyboard_button(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Prepared {
            id: String,
        }
        let r = f::bots::RequestWebViewButton::from_bytes(body)?;
        let body =
            json!({ "user_id": self.user_id(&r.user_id)?, "button": keyboard_button_json(&r.button, self.me())? });
        let prepared: Prepared = parse(self.call("savePreparedKeyboardButton", body).await?)?;
        Ok(types::bots::RequestedButton { webapp_req_id: prepared.id }.to_bytes())
    }

    /// `getManagedBotToken`, or `replaceManagedBotToken` when the old token is revoked.
    async fn export_bot_token(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::ExportBotToken::from_bytes(body)?;
        let method = if r.revoke { "replaceManagedBotToken" } else { "getManagedBotToken" };
        let token: String = parse(self.call(method, json!({ "user_id": self.user_id(&r.bot)? })).await?)?;
        Ok(types::bots::ExportedBotToken { token }.to_bytes())
    }

    async fn get_access_settings(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Access {
            #[serde(default)]
            is_access_restricted: bool,
            #[serde(default)]
            added_users: Vec<bot::User>,
        }
        let r = f::bots::GetAccessSettings::from_bytes(body)?;
        let body = json!({ "user_id": self.user_id(&r.bot)? });
        let access: Access = parse(self.call("getManagedBotAccessSettings", body).await?)?;
        let mut peers = Peers::new(self.me());
        for user in &access.added_users {
            peers.user(user);
        }
        let settings =
            types::bots::AccessSettings { restricted: access.is_access_restricted, add_users: Some(peers.users) };
        Ok(settings.to_bytes())
    }

    async fn edit_access_settings(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::EditAccessSettings::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("user_id".into(), self.user_id(&r.bot)?.into());
        out.insert("is_access_restricted".into(), r.restricted.into());
        if let Some(users) = &r.add_users {
            let ids = users.iter().map(|user| self.user_id(user)).collect::<Result<Vec<i64>>>()?;
            out.insert("added_user_ids".into(), ids.into());
        }
        self.call("setManagedBotAccessSettings", Value::Object(out)).await?;
        Ok(done())
    }

    /// The scope and language a command list is picked by.
    fn command_list(&self, scope: &base::BotCommandScope, lang_code: String) -> Result<Value> {
        let mut out = Map::new();
        out.insert("scope".into(), self.scope_json(scope)?);
        if !lang_code.is_empty() {
            out.insert("language_code".into(), lang_code.into());
        }
        Ok(Value::Object(out))
    }

    async fn delete_commands(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::ResetBotCommands::from_bytes(body)?;
        self.call("deleteMyCommands", self.command_list(&r.scope, r.lang_code)?).await?;
        Ok(done())
    }

    async fn get_commands(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Command {
            command: String,
            description: String,
            #[serde(default)]
            is_ephemeral: Option<bool>,
        }
        let r = f::bots::GetBotCommands::from_bytes(body)?;
        let list: Vec<Command> = parse(self.call("getMyCommands", self.command_list(&r.scope, r.lang_code)?).await?)?;
        let commands: Vec<base::BotCommand> = list
            .into_iter()
            .map(|c| {
                let mut command = types::BotCommand::new(c.command, c.description);
                command.ephemeral = c.is_ephemeral.unwrap_or(false);
                base::BotCommand::BotCommand(Box::new(command))
            })
            .collect();
        Ok(commands.to_bytes())
    }

    /// One Bot API call for each of the name, description and short description (TL's `about`) set.
    async fn set_bot_info(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::SetBotInfo::from_bytes(body)?;
        if r.bot.is_some() {
            return Err(unsupported("the name or descriptions of another bot"));
        }
        let fields = [
            ("setMyName", "name", r.name),
            ("setMyDescription", "description", r.description),
            ("setMyShortDescription", "short_description", r.about),
        ];
        for (method, key, value) in fields {
            let Some(value) = value else { continue };
            let mut out = Map::new();
            out.insert(key.into(), value.into());
            if !r.lang_code.is_empty() {
                out.insert("language_code".into(), r.lang_code.clone().into());
            }
            self.call(method, Value::Object(out)).await?;
        }
        Ok(done())
    }

    /// The Bot API reads the name and both descriptions apart, so this makes three calls, at once.
    async fn get_bot_info(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Name {
            #[serde(default)]
            name: String,
        }
        #[derive(Deserialize)]
        struct Description {
            #[serde(default)]
            description: String,
        }
        #[derive(Deserialize)]
        struct ShortDescription {
            #[serde(default)]
            short_description: String,
        }
        let r = f::bots::GetBotInfo::from_bytes(body)?;
        if r.bot.is_some() {
            return Err(unsupported("the name or descriptions of another bot"));
        }
        let language = if r.lang_code.is_empty() { json!({}) } else { json!({ "language_code": r.lang_code }) };
        let (name, description, about) = tokio::try_join!(
            self.call("getMyName", language.clone()),
            self.call("getMyDescription", language.clone()),
            self.call("getMyShortDescription", language),
        )?;
        let info = types::bots::BotInfo {
            name: parse::<Name>(name)?.name,
            about: parse::<ShortDescription>(about)?.short_description,
            description: parse::<Description>(description)?.description,
        };
        Ok(info.to_bytes())
    }

    /// An empty user is the default button, for every chat without one of its own.
    async fn set_menu_button(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::SetBotMenuButton::from_bytes(body)?;
        let mut out = Map::new();
        if !matches!(r.user_id, base::InputUser::InputUserEmpty) {
            out.insert("chat_id".into(), self.user_id(&r.user_id)?.into());
        }
        let button = match &r.button {
            base::BotMenuButton::BotMenuButtonDefault => json!({ "type": "default" }),
            base::BotMenuButton::BotMenuButtonCommands => json!({ "type": "commands" }),
            base::BotMenuButton::BotMenuButton(b) => {
                json!({ "type": "web_app", "text": b.text, "web_app": { "url": b.url } })
            }
        };
        out.insert("menu_button".into(), button);
        self.call("setChatMenuButton", Value::Object(out)).await?;
        Ok(done())
    }

    async fn get_menu_button(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::GetBotMenuButton::from_bytes(body)?;
        let mut out = Map::new();
        if !matches!(r.user_id, base::InputUser::InputUserEmpty) {
            out.insert("chat_id".into(), self.user_id(&r.user_id)?.into());
        }
        let button = self.call("getChatMenuButton", Value::Object(out)).await?;
        let button = match (button["type"].as_str(), button["web_app"]["url"].as_str()) {
            (Some("web_app"), Some(url)) => {
                let text = button["text"].as_str().unwrap_or_default().to_owned();
                base::BotMenuButton::BotMenuButton(Box::new(types::BotMenuButton { text, url: url.to_owned() }))
            }
            (Some("commands"), _) => base::BotMenuButton::BotMenuButtonCommands,
            _ => base::BotMenuButton::BotMenuButtonDefault,
        };
        Ok(button.to_bytes())
    }

    async fn set_default_rights(&self, rights: &base::ChatAdminRights, for_channels: bool) -> Result<Vec<u8>> {
        let body = json!({ "rights": admin_rights_json(rights), "for_channels": for_channels });
        self.call("setMyDefaultAdministratorRights", body).await?;
        Ok(done())
    }

    /// `get_bot_default_privileges`: TL reads them from the bot's full user, which `getChat` doesn't carry, so the
    /// friendly method asks here. `None` when no right is set, as Telegram leaves them out then.
    pub(crate) async fn default_admin_rights(&self, for_channels: bool) -> Result<Option<base::ChatAdminRights>> {
        let rights = self.call("getMyDefaultAdministratorRights", json!({ "for_channels": for_channels })).await?;
        let rights = admin_rights_tl(&rights);
        if rights == types::ChatAdminRights::default() {
            return Ok(None);
        }
        Ok(Some(base::ChatAdminRights::ChatAdminRights(Box::new(rights))))
    }

    /// Only the bot's own current photo can go: the Bot API can't pick an earlier one or a bot it manages.
    async fn remove_profile_photo(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::photos::UpdateProfilePhoto::from_bytes(body)?;
        if r.bot.is_some() {
            return Err(unsupported("the profile photo of another bot"));
        }
        if !matches!(r.id, base::InputPhoto::InputPhotoEmpty) {
            return Err(unsupported("setting an earlier photo as the profile photo"));
        }
        if r.fallback {
            return Err(unsupported("the public profile photo of a bot"));
        }
        self.call("removeMyProfilePhoto", json!({})).await?;
        Ok(no_photo())
    }
}

/// The TL answer to a profile photo change that left no photo.
pub(super) fn no_photo() -> Vec<u8> {
    let photo = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 });
    types::photos::Photo { photo, users: Vec::new() }.to_bytes()
}

/// A keyboard button as the Bot API's `KeyboardButton`. Buttons that ask for users, chats or a new bot, which
/// prepared buttons are for, are written here; the others as in a keyboard.
fn keyboard_button_json(button: &base::KeyboardButton, me: i64) -> Result<Value> {
    use base::ButtonType as T;
    use base::RequestPeerType as P;
    let base::KeyboardButton::KeyboardButton(b) = button;
    let (button_id, peer_type, max_quantity, (name, username, photo)) = match &b.r#type {
        T::ButtonTypeRequestPeer(t) => (t.button_id, &t.peer_type, t.max_quantity, (false, false, false)),
        T::InputButtonTypeRequestPeer(t) => {
            (t.button_id, &t.peer_type, t.max_quantity, (t.name_requested, t.username_requested, t.photo_requested))
        }
        _ => {
            let row = types::KeyboardButtonRow { buttons: vec![button.clone()] };
            let rows = vec![base::KeyboardButtonRow::KeyboardButtonRow(Box::new(row))];
            let markup = base::ReplyMarkup::ReplyKeyboardMarkup(Box::new(types::ReplyKeyboardMarkup::new(rows)));
            let keyboard = convert::markup_json(&markup, me)?;
            return Ok(keyboard["keyboard"][0][0].clone());
        }
    };
    let mut request = Map::new();
    request.insert("request_id".into(), button_id.into());
    let mut flag = |key: &str, on: Option<bool>| {
        if let Some(on) = on {
            request.insert(key.into(), on.into());
        }
    };
    let key = match peer_type {
        P::RequestPeerTypeUser(t) => {
            flag("user_is_bot", t.bot);
            flag("user_is_premium", t.premium);
            flag("request_name", name.then_some(true));
            flag("request_username", username.then_some(true));
            flag("request_photo", photo.then_some(true));
            request.insert("max_quantity".into(), max_quantity.into());
            "request_users"
        }
        P::RequestPeerTypeChat(t) => {
            flag("chat_is_channel", Some(false));
            flag("chat_is_forum", t.forum);
            flag("chat_has_username", t.has_username);
            flag("chat_is_created", t.creator.then_some(true));
            flag("bot_is_member", t.bot_participant.then_some(true));
            flag("request_title", name.then_some(true));
            flag("request_username", username.then_some(true));
            flag("request_photo", photo.then_some(true));
            chat_rights(&mut request, t.user_admin_rights.as_ref(), t.bot_admin_rights.as_ref());
            "request_chat"
        }
        P::RequestPeerTypeBroadcast(t) => {
            flag("chat_is_channel", Some(true));
            flag("chat_has_username", t.has_username);
            flag("chat_is_created", t.creator.then_some(true));
            flag("request_title", name.then_some(true));
            flag("request_username", username.then_some(true));
            flag("request_photo", photo.then_some(true));
            chat_rights(&mut request, t.user_admin_rights.as_ref(), t.bot_admin_rights.as_ref());
            "request_chat"
        }
        P::RequestPeerTypeCreateBot(t) => {
            if let Some(name) = &t.suggested_name {
                request.insert("suggested_name".into(), name.clone().into());
            }
            if let Some(username) = &t.suggested_username {
                request.insert("suggested_username".into(), username.clone().into());
            }
            "request_managed_bot"
        }
    };
    let mut out = Map::new();
    out.insert("text".into(), b.text.clone().into());
    out.insert(key.into(), Value::Object(request));
    Ok(Value::Object(out))
}

fn chat_rights(
    out: &mut Map<String, Value>,
    user: Option<&base::ChatAdminRights>,
    bot: Option<&base::ChatAdminRights>,
) {
    if let Some(rights) = user {
        out.insert("user_administrator_rights".into(), admin_rights_json(rights));
    }
    if let Some(rights) = bot {
        out.insert("bot_administrator_rights".into(), admin_rights_json(rights));
    }
}
