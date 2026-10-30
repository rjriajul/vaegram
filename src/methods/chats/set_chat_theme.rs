use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn set_chat_theme(&self, chat_id: impl Into<PeerRef>, emoji: impl Into<String>) -> Result<()> {
        let emoticon = emoji.into();
        let theme = if emoticon.is_empty() {
            base::InputChatTheme::InputChatThemeEmpty
        } else {
            base::InputChatTheme::InputChatTheme(Box::new(types::InputChatTheme { emoticon }))
        };
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::SetChatTheme { peer, theme }).await.map(drop)
    }
}
