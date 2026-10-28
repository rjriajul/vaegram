use super::facts::{Facts, PeerKinds};
use crate::Client;
use crate::client::Me;
use crate::types::{
    BotSubscriptionUpdated, CallbackQuery, Chat, ChatJoinRequest, ChosenInlineResult, DeletedEphemeralMessages,
    EphemeralMessage, GuestMessage, InlineQuery, ManagedBotUpdated, Message, MessageGenerationStopped, User,
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

subject!(Message: HasChat, HasSender, HasText, HasMedia; |s, me| {
    let sender_bot = s.from_user.as_ref().is_some_and(|u| u.is_bot);
    Facts::plain(&s.raw, PeerKinds { chat: Some(s.chat.kind), sender_bot }, me)
});

subject!(CallbackQuery: HasChat, HasSender, HasData; |s, me| {
    let kinds = PeerKinds { chat: Some(s.chat.kind), sender_bot: s.from_user.is_bot };
    Facts::query(s.chat.id, s.from_user.id, s.data.as_bytes(), kinds, me)
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
