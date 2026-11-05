use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::{BotVerification, Chat, Photo, User};
use crate::utils::get_peer;

/// What an invite link shows about a chat before it is joined.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatPreview {
    /// The chat title.
    pub title: String,
    /// Whether the invite is for a group, supergroup or channel.
    pub kind: ChatType,
    /// The chat description. `None` when it has none.
    pub about: Option<String>,
    /// Empty for invites to chats the account already joined or may peek into: `chat` carries the photo then.
    pub photo: Option<Photo>,
    /// Number of members in the chat.
    pub members_count: i32,
    /// Some of the members, when Telegram shows them. Empty for already-joined and peek invites; see `chat`.
    pub members: Vec<User>,
    /// The chat has a public username.
    pub is_public: bool,
    /// Joining needs an admin's approval.
    pub join_by_request: bool,
    /// The chat carries Telegram's verified badge.
    pub is_verified: bool,
    /// Telegram flagged the chat as a scam.
    pub is_scam: bool,
    /// Telegram flagged the chat as an impersonation.
    pub is_fake: bool,
    /// The Stars a subscription to this channel costs per period.
    pub subscription_star_count: Option<i64>,
    /// The length in seconds of a subscription period.
    pub subscription_period: Option<i32>,
    /// The verification a bot gave the chat. `None` when it has none.
    pub bot_verification: Option<BotVerification>,
    /// The chat itself, when the account is already a member or may peek into it.
    pub chat: Option<Chat>,
}

impl ChatPreview {
    pub(crate) fn parse(client: &crate::Client, invite: base::ChatInvite) -> Result<ChatPreview> {
        let chat = match invite {
            base::ChatInvite::ChatInvite(c) => return Ok(ChatPreview::from_invite(client, *c)),
            base::ChatInvite::ChatInviteAlready(c) => c.chat,
            base::ChatInvite::ChatInvitePeek(c) => c.chat,
        };
        let entities = Entities::new(client, Vec::new(), vec![chat]);
        let id =
            entities.chats.keys().next().copied().ok_or(Error::Unexpected("messages.checkChatInvite sent no chat"))?;
        let chat = Chat::parse(&get_peer(id), &entities, false);
        Ok(ChatPreview {
            title: chat.title.clone().unwrap_or_default(),
            kind: chat.kind,
            about: chat.description.clone(),
            photo: None,
            members_count: chat.members_count.unwrap_or(0),
            members: Vec::new(),
            is_public: chat.username.is_some(),
            join_by_request: chat.join_by_request,
            is_verified: chat.is_verified,
            is_scam: chat.is_scam,
            is_fake: chat.is_fake,
            subscription_star_count: None,
            subscription_period: None,
            bot_verification: None,
            chat: Some(chat),
        })
    }

    fn from_invite(client: &crate::Client, c: crate::raw::types::ChatInvite) -> ChatPreview {
        let kind = match (c.channel, c.broadcast) {
            (false, _) => ChatType::Group,
            (true, true) => ChatType::Channel,
            (true, false) => ChatType::Supergroup,
        };
        let members = User::parse_all(client, c.participants.unwrap_or_default());
        let pricing = c.subscription_pricing.map(|base::StarsSubscriptionPricing::StarsSubscriptionPricing(p)| p);
        ChatPreview {
            title: c.title,
            kind,
            about: c.about,
            photo: Photo::parse(&c.photo),
            members_count: c.participants_count,
            members,
            is_public: c.public,
            join_by_request: c.request_needed,
            is_verified: c.verified,
            is_scam: c.scam,
            is_fake: c.fake,
            subscription_star_count: pricing.as_ref().map(|p| p.amount),
            subscription_period: pricing.as_ref().map(|p| p.period),
            bot_verification: c
                .bot_verification
                .map(|v| BotVerification::parse(v, &Entities::new(client, Vec::new(), Vec::new()))),
            chat: None,
        }
    }
}
