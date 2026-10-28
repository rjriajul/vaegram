use super::facts::{Facts, PeerKinds};
use crate::Client;
use crate::client::Me;
use crate::types::{
    BotSubscriptionUpdated, BusinessConnection, CallbackQuery, Chat, ChatBoostRemoved, ChatBoostSource,
    ChatBoostUpdated, ChatJoinRequest, ChatMemberUpdated, ChosenInlineResult, DeletedBusinessMessages,
    DeletedEphemeralMessages, DeletedMessages, EphemeralMessage, GuestMessage, InlineQuery, ManagedBotUpdated, Message,
    MessageGenerationStopped, MessageReactionCountUpdated, MessageReactionUpdated, PollAnswer, PollUpdated,
    PreCheckoutQuery, PurchasedPaidMedia, ShippingQuery, User, UserStatusUpdated,
};

// Public traits in a private module: nothing outside the crate can name them, so `Subject` stays sealed and
// `Inspect` uncallable while `Filter::check` can still bound on it.
pub(crate) mod private {
    use super::{Client, Facts, Me};

    pub trait Sealed {}

    pub trait Inspect {
        fn client(&self) -> &Client;
        fn facts<'a>(&'a self, me: &'a Me) -> Facts<'a>;
    }
}

/// An update kind filters can test. Sealed: the capability traits below decide which filters apply.
pub trait Subject: private::Sealed + Send + 'static {}
pub trait HasChat: Subject {}
pub trait HasSender: Subject {}
pub trait HasText: Subject {}
pub trait HasMedia: Subject {}
pub trait HasData: Subject {}

macro_rules! subject {
    ($ty:ident: $($cap:ident),*; |$s:ident, $me:ident| $facts:expr) => {
        impl private::Sealed for $ty {}

        impl private::Inspect for $ty {
            fn client(&self) -> &Client {
                $ty::client(self)
            }

            fn facts<'a>(&'a self, $me: &'a Me) -> Facts<'a> {
                let $s = self;
                $facts
            }
        }

        impl Subject for $ty {}
        $(impl $cap for $ty {})*
    };
}

fn user_query<'a>(user: &User, me: &'a Me) -> Facts<'a> {
    Facts::query(user.id, user.id, &[], PeerKinds { chat: None, sender_bot: user.is_bot }, me)
}

fn chat_query<'a>(chat: Option<&Chat>, me: &'a Me) -> Facts<'a> {
    Facts::query(chat.map_or(0, |c| c.id), 0, &[], PeerKinds { chat: chat.map(|c| c.kind), sender_bot: false }, me)
}

// Anonymous admins act on behalf of a chat, which then stands in as the sender.
fn sender_of(user: Option<&User>, chat: Option<&Chat>) -> (i64, bool) {
    match (user, chat) {
        (Some(u), _) => (u.id, u.is_bot),
        (None, Some(c)) => (c.id, false),
        (None, None) => (0, false),
    }
}

subject!(Message: HasChat, HasSender, HasText, HasMedia; |s, me| {
    let sender_bot = s.from_user.as_ref().is_some_and(|u| u.is_bot);
    Facts::plain(&s.raw, PeerKinds { chat: Some(s.chat.kind), sender_bot }, me)
});

subject!(CallbackQuery: HasChat, HasSender, HasData; |s, me| {
    let chat = s.chat();
    let kinds = PeerKinds { chat: chat.map(|c| c.kind), sender_bot: s.from_user.is_bot };
    // Inline messages have no chat; 0 is what chat-less updates carry, so chat filters reject them.
    let chat_id = chat.map_or(0, |c| c.id);
    Facts::query(chat_id, s.from_user.id, s.data.as_bytes(), kinds, me)
});

subject!(InlineQuery: HasSender, HasText; |s, me| Facts::inline(s.from_user.id, &s.query, me));

subject!(ChosenInlineResult: HasSender, HasText; |s, me| Facts::inline(s.from_user.id, &s.query, me));

subject!(ChatJoinRequest: HasChat, HasSender; |s, me| {
    Facts::query(
        s.chat.id,
        s.from_user.id,
        &[],
        PeerKinds { chat: Some(s.chat.kind), sender_bot: s.from_user.is_bot },
        me,
    )
});

subject!(GuestMessage: HasChat, HasSender, HasText, HasMedia; |s, me| private::Inspect::facts(&s.message, me));

subject!(BotSubscriptionUpdated: HasSender; |s, me| user_query(&s.user, me));

subject!(MessageGenerationStopped: HasChat, HasSender; |s, me| {
    // Drafts only go to private chats, so the chat is also the user who pressed stop.
    let kinds = PeerKinds { chat: Some(s.chat.kind), sender_bot: false };
    Facts::query(s.chat.id, s.chat.id, &[], kinds, me)
});

subject!(ManagedBotUpdated: HasSender; |s, me| {
    // The update has no chat; its sender is the user who created the bot.
    Facts::query(0, s.user.id, &[], PeerKinds { chat: None, sender_bot: s.user.is_bot }, me)
});

subject!(EphemeralMessage: HasChat, HasSender, HasText; |s, me| {
    let sender_bot = s.from_user.as_ref().is_some_and(|u| u.is_bot);
    Facts::ephemeral(&s.raw, s.chat.id, PeerKinds { chat: Some(s.chat.kind), sender_bot }, me)
});

subject!(DeletedEphemeralMessages: HasChat; |s, me| chat_query(Some(&s.chat), me));

subject!(MessageReactionUpdated: HasChat, HasSender; |s, me| {
    let (sender_id, sender_bot) = sender_of(s.user.as_ref(), s.actor_chat.as_ref());
    Facts::query(s.chat.id, sender_id, &[], PeerKinds { chat: Some(s.chat.kind), sender_bot }, me)
});

subject!(MessageReactionCountUpdated: HasChat; |s, me| chat_query(Some(&s.chat), me));

subject!(BusinessConnection: HasSender; |s, me| user_query(&s.user, me));

subject!(DeletedBusinessMessages: HasChat; |s, me| chat_query(Some(&s.chat), me));

subject!(ShippingQuery: HasSender; |s, me| user_query(&s.from_user, me));

subject!(PreCheckoutQuery: HasSender; |s, me| user_query(&s.from_user, me));

subject!(PurchasedPaidMedia: HasSender; |s, me| user_query(&s.from_user, me));

// The booster stands in as the sender; an unclaimed giveaway prize has none.
fn boost_facts<'a>(chat: &Chat, source: &ChatBoostSource, me: &'a Me) -> Facts<'a> {
    let (sender_id, sender_bot) = source.user().map_or((0, false), |u| (u.id, u.is_bot));
    Facts::query(chat.id, sender_id, &[], PeerKinds { chat: Some(chat.kind), sender_bot }, me)
}

subject!(ChatBoostUpdated: HasChat, HasSender; |s, me| boost_facts(&s.chat, &s.boost.source, me));

subject!(ChatBoostRemoved: HasChat, HasSender; |s, me| boost_facts(&s.chat, &s.source, me));

subject!(ChatMemberUpdated: HasChat, HasSender; |s, me| {
    let kinds = PeerKinds { chat: Some(s.chat.kind), sender_bot: s.from_user.is_bot };
    Facts::query(s.chat.id, s.from_user.id, &[], kinds, me)
});

subject!(PollUpdated: HasChat; |s, me| chat_query(s.chat.as_ref(), me));

subject!(PollAnswer: HasSender; |s, me| {
    let (sender_id, sender_bot) = sender_of(s.user.as_ref(), s.voter_chat.as_ref());
    Facts::query(0, sender_id, &[], PeerKinds { chat: None, sender_bot }, me)
});

subject!(DeletedMessages: HasChat; |s, me| chat_query(s.chat.as_ref(), me));

subject!(UserStatusUpdated: HasSender; |s, me| {
    Facts::query(0, s.user.id, &[], PeerKinds { chat: None, sender_bot: s.user.is_bot }, me)
});
