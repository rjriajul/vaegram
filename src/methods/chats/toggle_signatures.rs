use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::toggle_signatures`].
#[must_use = "a request does nothing until it is awaited"]
pub struct ToggleSignatures<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    enabled: bool,
    profiles: bool,
}

impl Client {
    /// Adds the author's name to channel posts, or removes it.
    ///
    /// The name doesn't link to the author's profile unless [`profiles`](ToggleSignatures::profiles) is set.
    /// Awaiting the request returns `()`.
    pub fn toggle_signatures(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> ToggleSignatures<'_> {
        ToggleSignatures { client: self, chat_id: chat_id.into(), enabled, profiles: false }
    }
}

impl ToggleSignatures<'_> {
    /// Also links each signature to the author's profile.
    pub fn profiles(mut self) -> Self {
        self.profiles = true;
        self
    }

    async fn send(self) -> Result<()> {
        let channel = self.client.resolve_channel(&self.chat_id).await?;
        let request = functions::channels::ToggleSignatures {
            signatures_enabled: self.enabled,
            profiles_enabled: self.profiles,
            channel,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(ToggleSignatures, ());
