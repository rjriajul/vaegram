use super::facts::{Facts, PeerKinds};
use crate::Client;
use crate::client::Me;
use crate::types::{CallbackQuery, InlineQuery, Message};

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

impl private::Sealed for Message {}

impl private::Inspect for Message {
    fn client(&self) -> &Client {
        Message::client(self)
    }

    fn facts<'a>(&'a self, me: &'a Me) -> Facts<'a> {
        let sender_bot = self.from_user.as_ref().is_some_and(|u| u.is_bot);
        Facts::plain(&self.raw, PeerKinds { chat: Some(self.chat.kind), sender_bot }, me)
    }
}

impl Subject for Message {}
impl HasChat for Message {}
impl HasSender for Message {}
impl HasText for Message {}
impl HasMedia for Message {}

impl private::Sealed for CallbackQuery {}

impl private::Inspect for CallbackQuery {
    fn client(&self) -> &Client {
        CallbackQuery::client(self)
    }

    fn facts<'a>(&'a self, me: &'a Me) -> Facts<'a> {
        let kinds = PeerKinds { chat: Some(self.chat.kind), sender_bot: self.from_user.is_bot };
        Facts::query(self.chat.id, self.from_user.id, self.data.as_bytes(), kinds, me)
    }
}

impl Subject for CallbackQuery {}
impl HasChat for CallbackQuery {}
impl HasSender for CallbackQuery {}
impl HasData for CallbackQuery {}

impl private::Sealed for InlineQuery {}

impl private::Inspect for InlineQuery {
    fn client(&self) -> &Client {
        InlineQuery::client(self)
    }

    fn facts<'a>(&'a self, me: &'a Me) -> Facts<'a> {
        Facts::inline(self.from_user.id, &self.query, me)
    }
}

impl Subject for InlineQuery {}
impl HasSender for InlineQuery {}
impl HasText for InlineQuery {}
