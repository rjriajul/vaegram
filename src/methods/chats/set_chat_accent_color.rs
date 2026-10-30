use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SetChatAccentColor<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    for_profile: bool,
    accent_color_id: Option<i32>,
    background_custom_emoji_id: Option<i64>,
}

impl Client {
    pub fn set_chat_accent_color(
        &self,
        chat_id: impl Into<PeerRef>,
        accent_color_id: Option<i32>,
    ) -> SetChatAccentColor<'_> {
        SetChatAccentColor {
            client: self,
            chat_id: chat_id.into(),
            for_profile: false,
            accent_color_id,
            background_custom_emoji_id: None,
        }
    }
}

impl SetChatAccentColor<'_> {
    pub fn background_custom_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.background_custom_emoji_id = Some(custom_emoji_id);
        self
    }

    pub(super) fn for_profile(mut self) -> Self {
        self.for_profile = true;
        self
    }

    async fn send(self) -> Result<()> {
        let (client, for_profile) = (self.client, self.for_profile);
        let (color, background_emoji_id) = (self.accent_color_id, self.background_custom_emoji_id);
        if let (_, base::InputPeer::InputPeerSelf) = client.resolve(&self.chat_id).await? {
            let reset = color.is_none() && background_emoji_id.is_none();
            let color = types::PeerColor { color, background_emoji_id };
            let color = (!reset).then(|| base::PeerColor::PeerColor(Box::new(color)));
            client.invoke(&functions::account::UpdateColor { for_profile, color }).await?;
        } else {
            let channel = client.resolve_channel(&self.chat_id).await?;
            let request = functions::channels::UpdateColor { for_profile, channel, color, background_emoji_id };
            client.invoke(&request).await?;
        }
        Ok(())
    }
}

into_future!(SetChatAccentColor, ());
