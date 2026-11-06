use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::set_managed_bot_access_settings`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetManagedBotAccessSettings<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    restricted_to: Option<Vec<PeerRef>>,
}

impl Client {
    /// Lets everyone use a bot you manage.
    ///
    /// [`restrict_to`](SetManagedBotAccessSettings::restrict_to) limits it to chosen users instead.
    pub fn set_managed_bot_access_settings(&self, bot_id: impl Into<PeerRef>) -> SetManagedBotAccessSettings<'_> {
        SetManagedBotAccessSettings { client: self, bot_id: bot_id.into(), restricted_to: None }
    }
}

impl SetManagedBotAccessSettings<'_> {
    /// Restricts access to the bot: besides its owner, only these users may use it.
    pub fn restrict_to(mut self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Self {
        self.restricted_to = Some(user_ids.into_iter().map(Into::into).collect());
        self
    }

    async fn send(self) -> Result<()> {
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let add_users = match self.restricted_to {
            Some(users) => Some(self.client.resolve_users(users).await?),
            None => None,
        };
        let request = functions::bots::EditAccessSettings { restricted: add_users.is_some(), bot, add_users };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetManagedBotAccessSettings, ());
