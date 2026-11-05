use crate::raw::{base, types};

/// Asks the user to pick users; they arrive as `ServiceAction::UsersShared` carrying `request_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardButtonRequestUsers {
    /// Identifies the request in the users shared back.
    pub request_id: i32,
    /// `Some(true)` only bots, `Some(false)` only people.
    pub user_is_bot: Option<bool>,
    /// `Some(true)` only Premium users, `Some(false)` only users without Premium.
    pub user_is_premium: Option<bool>,
    /// How many users the user may pick.
    pub max_quantity: i32,
    /// Also share the users' names.
    pub request_name: bool,
    /// Also share the users' usernames.
    pub request_username: bool,
    /// Also share the users' photos.
    pub request_photo: bool,
}

impl KeyboardButtonRequestUsers {
    /// Creates a request for one user, with no filters.
    pub fn new(request_id: i32) -> Self {
        KeyboardButtonRequestUsers {
            request_id,
            user_is_bot: None,
            user_is_premium: None,
            max_quantity: 1,
            request_name: false,
            request_username: false,
            request_photo: false,
        }
    }

    /// Limits the choice to bots or to people.
    pub fn user_is_bot(mut self, is_bot: bool) -> Self {
        self.user_is_bot = Some(is_bot);
        self
    }

    /// Limits the choice to Premium users or to users without Premium.
    pub fn user_is_premium(mut self, is_premium: bool) -> Self {
        self.user_is_premium = Some(is_premium);
        self
    }

    /// 1 to 10.
    pub fn max_quantity(mut self, n: i32) -> Self {
        self.max_quantity = n;
        self
    }

    /// Also shares the users' names.
    pub fn request_name(mut self) -> Self {
        self.request_name = true;
        self
    }

    /// Also shares the users' usernames.
    pub fn request_username(mut self) -> Self {
        self.request_username = true;
        self
    }

    /// Also shares the users' photos.
    pub fn request_photo(mut self) -> Self {
        self.request_photo = true;
        self
    }

    pub(crate) fn write(&self) -> base::ButtonType {
        let peer_type = types::RequestPeerTypeUser { bot: self.user_is_bot, premium: self.user_is_premium };
        base::ButtonType::InputButtonTypeRequestPeer(Box::new(types::InputButtonTypeRequestPeer {
            name_requested: self.request_name,
            username_requested: self.request_username,
            photo_requested: self.request_photo,
            button_id: self.request_id,
            peer_type: base::RequestPeerType::RequestPeerTypeUser(peer_type),
            max_quantity: self.max_quantity,
        }))
    }

    /// `None` unless `peer_type` asks for users.
    pub(crate) fn parse(
        request_id: i32,
        peer_type: &base::RequestPeerType,
        max_quantity: i32,
        requested: (bool, bool, bool),
    ) -> Option<KeyboardButtonRequestUsers> {
        let base::RequestPeerType::RequestPeerTypeUser(user) = peer_type else { return None };
        Some(KeyboardButtonRequestUsers {
            request_id,
            user_is_bot: user.bot,
            user_is_premium: user.premium,
            max_quantity,
            request_name: requested.0,
            request_username: requested.1,
            request_photo: requested.2,
        })
    }
}
