use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::toggle_forum_topics`].
#[must_use = "a request does nothing until it is awaited"]
pub struct ToggleForumTopics<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    enabled: bool,
    tabs: bool,
    sidebar: bool,
}

impl Client {
    /// Turns topics on or off for a supergroup.
    ///
    /// Topics show as a plain list unless [`tabs`](ToggleForumTopics::tabs) or
    /// [`sidebar`](ToggleForumTopics::sidebar) is set. Awaiting the request returns `()`.
    pub fn toggle_forum_topics(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> ToggleForumTopics<'_> {
        ToggleForumTopics { client: self, chat_id: chat_id.into(), enabled, tabs: false, sidebar: false }
    }
}

impl ToggleForumTopics<'_> {
    /// Shows the topics as tabs instead of a list.
    pub fn tabs(mut self) -> Self {
        self.tabs = true;
        self
    }

    /// Shows the topics in a sidebar.
    pub fn sidebar(mut self) -> Self {
        self.sidebar = true;
        self
    }

    async fn send(self) -> Result<()> {
        let channel = self.client.resolve_channel(&self.chat_id).await?;
        let request =
            functions::channels::ToggleForum { channel, enabled: self.enabled, tabs: self.tabs, sidebar: self.sidebar };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(ToggleForumTopics, ());
