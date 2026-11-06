use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::set_managed_bot_access_settings`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetManagedBotAccessSettings<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    access: Option<Access>,
}

/// Who may use the bot.
enum Access {
    Everyone,
    /// The owner and these users; empty for the owner alone.
    Only(Vec<PeerRef>),
}

impl Client {
    /// Sets who can use a bot you manage.
    ///
    /// Pick exactly one of [`allow_everyone`](SetManagedBotAccessSettings::allow_everyone),
    /// [`restrict_to`](SetManagedBotAccessSettings::restrict_to) and
    /// [`restrict_to_owner`](SetManagedBotAccessSettings::restrict_to_owner); the last one called wins. Awaiting the
    /// request with none of them fails with `Error::InvalidArgument` before anything is sent.
    pub fn set_managed_bot_access_settings(&self, bot_id: impl Into<PeerRef>) -> SetManagedBotAccessSettings<'_> {
        SetManagedBotAccessSettings { client: self, bot_id: bot_id.into(), access: None }
    }
}

impl SetManagedBotAccessSettings<'_> {
    /// Lets everyone use the bot.
    pub fn allow_everyone(mut self) -> Self {
        self.access = Some(Access::Everyone);
        self
    }

    /// Restricts the bot to its owner and these users, at most 10 besides the owner.
    ///
    /// The list replaces the current one: users left out lose access.
    pub fn restrict_to(mut self, users: impl IntoIterator<Item = impl Into<PeerRef>>) -> Self {
        self.access = Some(Access::Only(users.into_iter().map(Into::into).collect()));
        self
    }

    /// Restricts the bot to its owner alone, removing every other user from its access list.
    pub fn restrict_to_owner(mut self) -> Self {
        self.access = Some(Access::Only(Vec::new()));
        self
    }

    async fn send(self) -> Result<()> {
        let Some(access) = self.access else {
            return Err(Error::InvalidArgument(
                "set_managed_bot_access_settings needs allow_everyone, restrict_to or restrict_to_owner",
            ));
        };
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let add_users = match access {
            Access::Only(users) => Some(self.client.resolve_users(users).await?),
            Access::Everyone => None,
        };
        let request = functions::bots::EditAccessSettings { restricted: add_users.is_some(), bot, add_users };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetManagedBotAccessSettings, ());
