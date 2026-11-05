use crate::Client;
use crate::methods::SetChatAccentColor;
use crate::types::PeerRef;

impl Client {
    /// Sets the accent color of your own profile or of a channel's profile page.
    ///
    /// `None` resets the color.
    pub fn set_chat_profile_accent_color(
        &self,
        chat_id: impl Into<PeerRef>,
        accent_color_id: Option<i32>,
    ) -> SetChatAccentColor<'_> {
        self.set_chat_accent_color(chat_id, accent_color_id).for_profile()
    }
}
