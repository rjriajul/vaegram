use crate::Client;
use crate::errors::Result;
use crate::raw::base;
use crate::types::{
    ButtonStyle, ForceReply, InlineAction, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton,
    KeyboardButtonPollType, KeyboardButtonRequestChat, KeyboardButtonRequestManagedBot, KeyboardButtonRequestUsers,
    LoginUrl, ReplyKeyboardMarkup, ReplyKeyboardRemove, SwitchInlineQueryChosenChat, WebAppInfo,
};

/// Buttons or keyboard actions attached to a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyMarkup {
    /// Buttons shown under the message.
    Inline(InlineKeyboardMarkup),
    /// A custom keyboard that replaces the regular one.
    Keyboard(ReplyKeyboardMarkup),
    /// Removes the custom keyboard.
    Remove(ReplyKeyboardRemove),
    /// Makes the user's client open a reply to the message.
    ForceReply(ForceReply),
}

impl ReplyMarkup {
    /// Looks up the users that profile and login buttons name, which can take requests.
    pub(crate) async fn write(&self, client: &Client) -> Result<base::ReplyMarkup> {
        Ok(match self {
            ReplyMarkup::Inline(m) => m.write(client).await?,
            ReplyMarkup::Keyboard(m) => m.write(),
            ReplyMarkup::Remove(m) => m.write(),
            ReplyMarkup::ForceReply(m) => m.write(),
        })
    }

    pub(crate) async fn write_option(
        markup: Option<&ReplyMarkup>,
        client: &Client,
    ) -> Result<Option<base::ReplyMarkup>> {
        match markup {
            Some(markup) => Ok(Some(markup.write(client).await?)),
            None => Ok(None),
        }
    }

    /// `None` when any button can't be read: an inline callback whose data isn't UTF-8, or a profile button naming the
    /// sender's own account. Every other button kind Telegram has is read.
    pub(crate) fn parse(raw: &base::ReplyMarkup) -> Option<ReplyMarkup> {
        Some(match raw {
            base::ReplyMarkup::ReplyKeyboardHide(m) => {
                ReplyMarkup::Remove(ReplyKeyboardRemove::new().selective(m.selective))
            }
            base::ReplyMarkup::ReplyKeyboardForceReply(m) => ReplyMarkup::ForceReply(ForceReply::parse(m)),
            base::ReplyMarkup::ReplyKeyboardMarkup(m) => {
                let rows = m.rows.iter().map(keyboard_row).collect::<Option<_>>()?;
                ReplyMarkup::Keyboard(ReplyKeyboardMarkup::parse(m, rows))
            }
            base::ReplyMarkup::ReplyInlineMarkup(m) => {
                let rows = m.rows.iter().map(inline_row).collect::<Option<_>>()?;
                ReplyMarkup::Inline(InlineKeyboardMarkup { rows, force_reply: m.force_reply })
            }
        })
    }
}

fn keyboard_row(raw: &base::KeyboardButtonRow) -> Option<Vec<KeyboardButton>> {
    let base::KeyboardButtonRow::KeyboardButtonRow(row) = raw;
    row.buttons.iter().map(keyboard_button).collect()
}

fn keyboard_button(raw: &base::KeyboardButton) -> Option<KeyboardButton> {
    let base::KeyboardButton::KeyboardButton(b) = raw;
    let mut button = KeyboardButton::new(b.text.clone()).style(style(b.style.as_ref()));
    match &b.r#type {
        base::ButtonType::ButtonTypeDefault => {}
        base::ButtonType::ButtonTypeRequestPhone => button.request_contact = true,
        base::ButtonType::ButtonTypeRequestGeoLocation => button.request_location = true,
        base::ButtonType::ButtonTypeRequestPeer(p) => {
            request_peer(&mut button, p.button_id, &p.peer_type, p.max_quantity, (false, false, false))?
        }
        base::ButtonType::InputButtonTypeRequestPeer(p) => {
            let requested = (p.name_requested, p.username_requested, p.photo_requested);
            request_peer(&mut button, p.button_id, &p.peer_type, p.max_quantity, requested)?
        }
        base::ButtonType::ButtonTypeRequestPoll(p) => button.request_poll = Some(KeyboardButtonPollType::parse(p)),
        base::ButtonType::ButtonTypeSimpleWebView(w) => button.web_app = Some(WebAppInfo::new(w.url.clone())),
    }
    button.icon_custom_emoji_id = icon(b.style.as_ref());
    Some(button)
}

/// Sets whichever request the peer type asks for; `None` for a peer type no button here asks for.
fn request_peer(
    button: &mut KeyboardButton,
    request_id: i32,
    peer_type: &base::RequestPeerType,
    max_quantity: i32,
    requested: (bool, bool, bool),
) -> Option<()> {
    if let base::RequestPeerType::RequestPeerTypeCreateBot(bot) = peer_type {
        button.request_managed_bot = Some(KeyboardButtonRequestManagedBot {
            request_id,
            suggested_name: bot.suggested_name.clone(),
            suggested_username: bot.suggested_username.clone(),
        });
    } else if let Some(request) = KeyboardButtonRequestUsers::parse(request_id, peer_type, max_quantity, requested) {
        button.request_users = Some(request);
    } else {
        button.request_chat = Some(KeyboardButtonRequestChat::parse(request_id, peer_type, requested)?);
    }
    Some(())
}

fn icon(raw: Option<&base::KeyboardButtonStyle>) -> Option<i64> {
    match raw {
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) => s.icon,
        None => None,
    }
}

fn inline_row(raw: &base::KeyboardInlineButtonRow) -> Option<Vec<InlineKeyboardButton>> {
    let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = raw;
    row.buttons.iter().map(inline_button).collect()
}

fn inline_button(raw: &base::KeyboardInlineButton) -> Option<InlineKeyboardButton> {
    use base::InlineButtonType as T;
    let base::KeyboardInlineButton::KeyboardInlineButton(b) = raw;
    let action = match &b.r#type {
        T::InlineButtonTypeCallback(t) if t.requires_password => {
            InlineAction::CallbackWithPassword(std::str::from_utf8(&t.data).ok()?.to_owned())
        }
        T::InlineButtonTypeCallback(t) => InlineAction::Callback(std::str::from_utf8(&t.data).ok()?.to_owned()),
        T::InlineButtonTypeUrl(t) => InlineAction::Url(t.url.clone()),
        T::InlineButtonTypeSwitchInline(t) if t.peer_types.is_none() && t.same_peer => {
            InlineAction::SwitchInlineQueryCurrentChat(t.query.clone())
        }
        T::InlineButtonTypeSwitchInline(t) => match &t.peer_types {
            None => InlineAction::SwitchInlineQuery(t.query.clone()),
            Some(types) => {
                InlineAction::SwitchInlineQueryChosenChat(SwitchInlineQueryChosenChat::parse(&t.query, types))
            }
        },
        T::InlineButtonTypeWebView(t) => InlineAction::WebApp(WebAppInfo::new(t.url.clone())),
        T::InlineButtonTypeUrlAuth(t) => InlineAction::LoginUrl(LoginUrl {
            url: t.url.clone(),
            forward_text: t.fwd_text.clone(),
            bot_username: None,
            request_write_access: false,
        }),
        T::InputInlineButtonTypeUrlAuth(t) => InlineAction::LoginUrl(LoginUrl {
            url: t.url.clone(),
            forward_text: t.fwd_text.clone(),
            bot_username: None,
            request_write_access: t.request_write_access,
        }),
        T::InlineButtonTypeUserProfile(t) => InlineAction::UserProfile(t.user_id),
        T::InputInlineButtonTypeUserProfile(t) => match &t.user_id {
            base::InputUser::InputUser(u) => InlineAction::UserProfile(u.user_id),
            _ => return None,
        },
        T::InlineButtonTypeBuy => InlineAction::Pay,
        T::InlineButtonTypeCopy(t) => InlineAction::CopyText(t.copy_text.clone()),
        T::InlineButtonTypeDisabled => InlineAction::Disabled,
        T::InlineButtonTypeGame => InlineAction::CallbackGame,
    };
    let icon_custom_emoji_id = icon(b.style.as_ref());
    Some(InlineKeyboardButton { text: b.text.clone(), action, style: style(b.style.as_ref()), icon_custom_emoji_id })
}

fn style(raw: Option<&base::KeyboardButtonStyle>) -> ButtonStyle {
    match raw {
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_primary => ButtonStyle::Primary,
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_danger => ButtonStyle::Danger,
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_success => ButtonStyle::Success,
        _ => ButtonStyle::Default,
    }
}

impl From<InlineKeyboardMarkup> for ReplyMarkup {
    fn from(m: InlineKeyboardMarkup) -> Self {
        ReplyMarkup::Inline(m)
    }
}

impl From<ReplyKeyboardMarkup> for ReplyMarkup {
    fn from(m: ReplyKeyboardMarkup) -> Self {
        ReplyMarkup::Keyboard(m)
    }
}

impl From<ReplyKeyboardRemove> for ReplyMarkup {
    fn from(m: ReplyKeyboardRemove) -> Self {
        ReplyMarkup::Remove(m)
    }
}

impl From<ForceReply> for ReplyMarkup {
    fn from(m: ForceReply) -> Self {
        ReplyMarkup::ForceReply(m)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ButtonStyle, ForceReply, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, ReplyKeyboardMarkup,
        ReplyKeyboardRemove, ReplyMarkup, base,
    };
    use crate::Client;
    use crate::enums::PollType;
    use crate::raw::{Deserializable, Serializable, types};
    use crate::types::{
        ChatPrivileges, KeyboardButtonPollType, KeyboardButtonRequestChat, KeyboardButtonRequestManagedBot,
        KeyboardButtonRequestUsers, LoginUrl, SwitchInlineQueryChosenChat, WebAppInfo,
    };

    fn client_knowing_user_7() -> Client {
        let client = Client::offline();
        let user = types::InputPeerUser { user_id: 7, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        client
    }

    fn rights() -> ChatPrivileges {
        ChatPrivileges::new().can_delete_messages(true).can_pin_messages(true)
    }

    #[tokio::test]
    async fn written_markup_reads_back() {
        let inline = InlineKeyboardMarkup::new([
            vec![
                InlineKeyboardButton::callback("a", "data").style(ButtonStyle::Primary),
                InlineKeyboardButton::url("b", "https://t.me"),
            ],
            vec![
                InlineKeyboardButton::switch_inline_query("c", "q"),
                InlineKeyboardButton::switch_inline_query_current_chat("d", ""),
                InlineKeyboardButton::copy_text("e", "copied").style(ButtonStyle::Danger),
            ],
        ]);
        let rows = [
            vec![KeyboardButton::new("phone").request_contact(), KeyboardButton::new("geo").request_location()],
            vec![KeyboardButton::new("plain").style(ButtonStyle::Success)],
        ];
        let keyboard = ReplyKeyboardMarkup::new(rows).one_time().placeholder("pick");
        let markups: [ReplyMarkup; 4] = [
            inline.into(),
            keyboard.into(),
            ReplyKeyboardRemove::new().selective(true).into(),
            ForceReply::new().selective(true).placeholder("why?").into(),
        ];
        let client = Client::offline();
        for markup in markups {
            let written = markup.write(&client).await.unwrap();
            let raw = base::ReplyMarkup::from_bytes(&written.to_bytes()).unwrap();
            assert_eq!(ReplyMarkup::parse(&raw), Some(markup));
        }
    }

    #[tokio::test]
    async fn every_button_kind_round_trips() {
        let reply = ReplyKeyboardMarkup::new([
            vec![
                KeyboardButton::new("users").request_users(
                    KeyboardButtonRequestUsers::new(1)
                        .user_is_bot(false)
                        .user_is_premium(true)
                        .max_quantity(3)
                        .request_name()
                        .request_photo(),
                ),
                KeyboardButton::new("group").request_chat(
                    KeyboardButtonRequestChat::new(2)
                        .chat_is_forum(true)
                        .chat_has_username(false)
                        .chat_is_created()
                        .bot_is_member()
                        .user_administrator_rights(rights())
                        .bot_administrator_rights(rights())
                        .request_title()
                        .request_username(),
                ),
                KeyboardButton::new("channel")
                    .request_chat(KeyboardButtonRequestChat::new(3).chat_is_channel().chat_has_username(true)),
            ],
            vec![
                KeyboardButton::new("poll").request_poll(KeyboardButtonPollType::default()),
                KeyboardButton::new("quiz").request_poll(KeyboardButtonPollType::new(PollType::Quiz)),
                KeyboardButton::new("regular").request_poll(KeyboardButtonPollType::new(PollType::Regular)),
            ],
            vec![
                KeyboardButton::new("app").web_app(WebAppInfo::new("https://a.b/app")).icon_custom_emoji_id(5),
                KeyboardButton::new("bot").request_managed_bot(KeyboardButtonRequestManagedBot::new(4)),
            ],
        ]);
        let inline = InlineKeyboardMarkup::new([
            vec![
                InlineKeyboardButton::web_app("app", WebAppInfo::new("https://a.b/app")),
                InlineKeyboardButton::login_url(
                    "login",
                    LoginUrl::new("https://a.b/login").forward_text("Log in").request_write_access(),
                ),
                InlineKeyboardButton::user_profile("user", 7),
            ],
            vec![
                InlineKeyboardButton::switch_inline_query_chosen_chat(
                    "chosen",
                    SwitchInlineQueryChosenChat::new("q").allow_user_chats().allow_group_chats().allow_channel_chats(),
                ),
                InlineKeyboardButton::switch_inline_query_chosen_chat("none", SwitchInlineQueryChosenChat::new("")),
                InlineKeyboardButton::pay("pay"),
                InlineKeyboardButton::callback_with_password("pw", "secret-data").icon_custom_emoji_id(8),
            ],
        ]);
        let client = client_knowing_user_7();
        for markup in [ReplyMarkup::from(reply), ReplyMarkup::from(inline)] {
            let written = markup.write(&client).await.unwrap();
            let raw = base::ReplyMarkup::from_bytes(&written.to_bytes()).unwrap();
            assert_eq!(ReplyMarkup::parse(&raw), Some(markup));
        }
    }

    #[tokio::test]
    async fn user_profile_button_resolves_the_user() {
        let markup = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::user_profile("u", 7)]]);
        let base::ReplyMarkup::ReplyInlineMarkup(raw) = markup.write(&client_knowing_user_7()).await.unwrap() else {
            panic!("expected an inline markup")
        };
        let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = &raw.rows[0];
        let base::KeyboardInlineButton::KeyboardInlineButton(button) = &row.buttons[0];
        let base::InlineButtonType::InputInlineButtonTypeUserProfile(profile) = &button.r#type else {
            panic!("expected a user profile button")
        };
        let expected = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 3 }));
        assert_eq!(profile.user_id, expected);
        // Without a client the user can't be named.
        assert!(InlineKeyboardButton::user_profile("u", 7).write_offline().is_err());
    }

    #[test]
    fn chosen_chat_writes_peer_types() {
        use base::InlineQueryPeerType as P;
        let chosen = SwitchInlineQueryChosenChat::new("q").allow_bot_chats().allow_group_chats().write();
        let base::InlineButtonType::InlineButtonTypeSwitchInline(switch) = chosen else { panic!("not a switch") };
        assert_eq!(
            (switch.same_peer, switch.peer_types),
            (
                false,
                Some(vec![P::InlineQueryPeerTypeBotPM, P::InlineQueryPeerTypeChat, P::InlineQueryPeerTypeMegagroup])
            )
        );
    }

    #[test]
    fn unknown_button_kind_still_drops_only_that_markup() {
        let button = |r#type| {
            let button = types::KeyboardInlineButton { style: None, text: "x".into(), r#type };
            base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button))
        };
        let markup = |buttons| {
            let row = types::KeyboardInlineButtonRow { buttons };
            let rows = vec![base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(row))];
            base::ReplyMarkup::ReplyInlineMarkup(Box::new(types::ReplyInlineMarkup { force_reply: false, rows }))
        };
        let callback = |data: &[u8]| {
            let callback = types::InlineButtonTypeCallback { requires_password: false, data: data.to_vec() };
            button(base::InlineButtonType::InlineButtonTypeCallback(Box::new(callback)))
        };

        assert!(ReplyMarkup::parse(&markup(vec![callback(b"ok")])).is_some());
        // The sender's own profile can't be told apart from a user id, so a button naming it isn't read.
        let own = types::InputInlineButtonTypeUserProfile { user_id: base::InputUser::InputUserSelf };
        let unknown = button(base::InlineButtonType::InputInlineButtonTypeUserProfile(Box::new(own)));
        assert_eq!(ReplyMarkup::parse(&markup(vec![callback(b"ok"), unknown])), None);
        assert_eq!(ReplyMarkup::parse(&markup(vec![callback(&[0xff])])), None);
        let buy = button(base::InlineButtonType::InlineButtonTypeBuy);
        assert!(ReplyMarkup::parse(&markup(vec![buy])).is_some());
    }
}
