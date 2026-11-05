use crate::raw::{base, types};

/// Opens a chat picker and puts the bot's username and `query` into the input field of the chosen chat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SwitchInlineQueryChosenChat {
    /// Text put into the input field after the bot's username.
    pub query: String,
    /// Lets the user pick private chats with users.
    pub allow_user_chats: bool,
    /// Lets the user pick private chats with bots.
    pub allow_bot_chats: bool,
    /// Lets the user pick groups and supergroups.
    pub allow_group_chats: bool,
    /// Lets the user pick channels.
    pub allow_channel_chats: bool,
}

impl SwitchInlineQueryChosenChat {
    /// Creates a picker that allows no chat type yet. Turn types on with the `allow_*` methods.
    pub fn new(query: impl Into<String>) -> Self {
        SwitchInlineQueryChosenChat { query: query.into(), ..Default::default() }
    }

    /// Lets the user pick private chats with users.
    pub fn allow_user_chats(mut self) -> Self {
        self.allow_user_chats = true;
        self
    }

    /// Lets the user pick private chats with bots.
    pub fn allow_bot_chats(mut self) -> Self {
        self.allow_bot_chats = true;
        self
    }

    /// Lets the user pick groups and supergroups.
    pub fn allow_group_chats(mut self) -> Self {
        self.allow_group_chats = true;
        self
    }

    /// Lets the user pick channels.
    pub fn allow_channel_chats(mut self) -> Self {
        self.allow_channel_chats = true;
        self
    }

    pub(crate) fn write(&self) -> base::InlineButtonType {
        use base::InlineQueryPeerType as P;
        let mut peer_types = Vec::new();
        if self.allow_user_chats {
            peer_types.push(P::InlineQueryPeerTypePM);
        }
        if self.allow_bot_chats {
            peer_types.push(P::InlineQueryPeerTypeBotPM);
        }
        if self.allow_group_chats {
            peer_types.extend([P::InlineQueryPeerTypeChat, P::InlineQueryPeerTypeMegagroup]);
        }
        if self.allow_channel_chats {
            peer_types.push(P::InlineQueryPeerTypeBroadcast);
        }
        base::InlineButtonType::InlineButtonTypeSwitchInline(Box::new(types::InlineButtonTypeSwitchInline {
            same_peer: false,
            query: self.query.clone(),
            peer_types: Some(peer_types),
        }))
    }

    pub(crate) fn parse(query: &str, peer_types: &[base::InlineQueryPeerType]) -> SwitchInlineQueryChosenChat {
        use base::InlineQueryPeerType as P;
        let mut chosen = SwitchInlineQueryChosenChat::new(query);
        for peer_type in peer_types {
            match peer_type {
                P::InlineQueryPeerTypePM => chosen.allow_user_chats = true,
                P::InlineQueryPeerTypeBotPM | P::InlineQueryPeerTypeSameBotPM => chosen.allow_bot_chats = true,
                P::InlineQueryPeerTypeChat | P::InlineQueryPeerTypeMegagroup => chosen.allow_group_chats = true,
                P::InlineQueryPeerTypeBroadcast => chosen.allow_channel_chats = true,
            }
        }
        chosen
    }
}
