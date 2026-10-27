use super::facts::{Facts, PeerKinds};
use crate::Client;
use crate::client::Me;
use crate::types::{CallbackQuery, ChatJoinRequest, GuestMessage, InlineQuery, Message};

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

subject!(Message: HasChat, HasSender, HasText, HasMedia; |s, me| {
    let sender_bot = s.from_user.as_ref().is_some_and(|u| u.is_bot);
    Facts::plain(&s.raw, PeerKinds { chat: Some(s.chat.kind), sender_bot }, me)
});

subject!(CallbackQuery: HasChat, HasSender, HasData; |s, me| {
    let kinds = PeerKinds { chat: Some(s.chat.kind), sender_bot: s.from_user.is_bot };
    Facts::query(s.chat.id, s.from_user.id, s.data.as_bytes(), kinds, me)
});

subject!(InlineQuery: HasSender, HasText; |s, me| Facts::inline(s.from_user.id, &s.query, me));

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
