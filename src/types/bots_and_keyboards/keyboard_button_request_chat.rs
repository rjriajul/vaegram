use crate::raw::{base, types};
use crate::types::ChatPrivileges;

/// Asks the user to pick a group or channel. It arrives as `ServiceAction::ChatShared` carrying `request_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardButtonRequestChat {
    /// Identifies the request in the [`ChatShared`](crate::types::ChatShared) that comes back.
    pub request_id: i32,
    /// A channel rather than a group.
    pub chat_is_channel: bool,
    /// Groups only. `Some(true)` only forums.
    pub chat_is_forum: Option<bool>,
    /// `Some(true)` only chats with a username, `Some(false)` only chats without. `None` for either.
    pub chat_has_username: Option<bool>,
    /// Only chats the user owns.
    pub chat_is_created: bool,
    /// Administrator rights the user must have in the chat. `None` for no requirement.
    pub user_administrator_rights: Option<ChatPrivileges>,
    /// Administrator rights the bot must have in the chat. `None` for no requirement.
    pub bot_administrator_rights: Option<ChatPrivileges>,
    /// Groups only: the bot has to be a member.
    pub bot_is_member: bool,
    /// Also share the chat's title.
    pub request_title: bool,
    /// Also share the chat's username.
    pub request_username: bool,
    /// Also share the chat's photo.
    pub request_photo: bool,
}

impl KeyboardButtonRequestChat {
    /// Creates a request for a group, with no further filters.
    pub fn new(request_id: i32) -> Self {
        KeyboardButtonRequestChat {
            request_id,
            chat_is_channel: false,
            chat_is_forum: None,
            chat_has_username: None,
            chat_is_created: false,
            user_administrator_rights: None,
            bot_administrator_rights: None,
            bot_is_member: false,
            request_title: false,
            request_username: false,
            request_photo: false,
        }
    }

    /// Asks for a channel instead of a group.
    pub fn chat_is_channel(mut self) -> Self {
        self.chat_is_channel = true;
        self
    }

    /// Limits groups to forums or to non-forums. Ignored for channels.
    pub fn chat_is_forum(mut self, is_forum: bool) -> Self {
        self.chat_is_forum = Some(is_forum);
        self
    }

    /// Limits chats to those with or without a username.
    pub fn chat_has_username(mut self, has_username: bool) -> Self {
        self.chat_has_username = Some(has_username);
        self
    }

    /// Limits chats to those the user owns.
    pub fn chat_is_created(mut self) -> Self {
        self.chat_is_created = true;
        self
    }

    /// Requires the user to have these administrator rights in the chat.
    pub fn user_administrator_rights(mut self, rights: ChatPrivileges) -> Self {
        self.user_administrator_rights = Some(rights);
        self
    }

    /// Requires the bot to have these administrator rights in the chat.
    pub fn bot_administrator_rights(mut self, rights: ChatPrivileges) -> Self {
        self.bot_administrator_rights = Some(rights);
        self
    }

    /// Limits groups to those the bot is a member of. Ignored for channels.
    pub fn bot_is_member(mut self) -> Self {
        self.bot_is_member = true;
        self
    }

    /// Also shares the chat's title.
    pub fn request_title(mut self) -> Self {
        self.request_title = true;
        self
    }

    /// Also shares the chat's username.
    pub fn request_username(mut self) -> Self {
        self.request_username = true;
        self
    }

    /// Also shares the chat's photo.
    pub fn request_photo(mut self) -> Self {
        self.request_photo = true;
        self
    }

    pub(crate) fn write(&self) -> base::ButtonType {
        let user_admin_rights = self.user_administrator_rights.as_ref().map(ChatPrivileges::write);
        let bot_admin_rights = self.bot_administrator_rights.as_ref().map(ChatPrivileges::write);
        let peer_type = if self.chat_is_channel {
            base::RequestPeerType::RequestPeerTypeBroadcast(Box::new(types::RequestPeerTypeBroadcast {
                creator: self.chat_is_created,
                has_username: self.chat_has_username,
                user_admin_rights,
                bot_admin_rights,
            }))
        } else {
            base::RequestPeerType::RequestPeerTypeChat(Box::new(types::RequestPeerTypeChat {
                creator: self.chat_is_created,
                bot_participant: self.bot_is_member,
                has_username: self.chat_has_username,
                forum: self.chat_is_forum,
                user_admin_rights,
                bot_admin_rights,
            }))
        };
        base::ButtonType::InputButtonTypeRequestPeer(Box::new(types::InputButtonTypeRequestPeer {
            name_requested: self.request_title,
            username_requested: self.request_username,
            photo_requested: self.request_photo,
            button_id: self.request_id,
            peer_type,
            max_quantity: 1,
        }))
    }

    /// `None` unless `peer_type` asks for a group or channel.
    pub(crate) fn parse(
        request_id: i32,
        peer_type: &base::RequestPeerType,
        requested: (bool, bool, bool),
    ) -> Option<KeyboardButtonRequestChat> {
        let rights = |r: &Option<base::ChatAdminRights>| r.as_ref().map(ChatPrivileges::parse);
        let mut request = KeyboardButtonRequestChat::new(request_id);
        match peer_type {
            base::RequestPeerType::RequestPeerTypeChat(c) => {
                request.chat_is_forum = c.forum;
                request.chat_has_username = c.has_username;
                request.chat_is_created = c.creator;
                request.bot_is_member = c.bot_participant;
                request.user_administrator_rights = rights(&c.user_admin_rights);
                request.bot_administrator_rights = rights(&c.bot_admin_rights);
            }
            base::RequestPeerType::RequestPeerTypeBroadcast(c) => {
                request.chat_is_channel = true;
                request.chat_has_username = c.has_username;
                request.chat_is_created = c.creator;
                request.user_administrator_rights = rights(&c.user_admin_rights);
                request.bot_administrator_rights = rights(&c.bot_admin_rights);
            }
            _ => return None,
        }
        (request.request_title, request.request_username, request.request_photo) = requested;
        Some(request)
    }
}
