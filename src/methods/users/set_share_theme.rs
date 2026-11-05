use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, ReactionType};

/// Sets the theme of a chat's share card. With nothing else set, it goes back to the default.
#[must_use = "a request does nothing until it is awaited"]
pub struct SetShareTheme<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    backdrop_id: Option<i32>,
    emojis: Option<Vec<ReactionType>>,
    gift: Option<String>,
}

impl Client {
    /// Changes the share card theme of a chat. Awaiting it with no option set restores the default theme.
    ///
    /// Fails with `Error::InvalidArgument` if [`gift`](SetShareTheme::gift) is combined with a backdrop or emojis.
    pub fn set_share_theme(&self, chat_id: impl Into<PeerRef>) -> SetShareTheme<'_> {
        SetShareTheme { client: self, chat_id: chat_id.into(), backdrop_id: None, emojis: None, gift: None }
    }
}

impl SetShareTheme<'_> {
    /// Uses the backdrop with this id, taken from [`get_share_theme_backdrops`](Client::get_share_theme_backdrops).
    pub fn backdrop_id(mut self, backdrop_id: i32) -> Self {
        self.backdrop_id = Some(backdrop_id);
        self
    }

    /// Shows these emojis on the share card. An empty list leaves the emojis unset.
    pub fn emojis(mut self, emojis: impl IntoIterator<Item = ReactionType>) -> Self {
        let emojis: Vec<_> = emojis.into_iter().collect();
        self.emojis = if emojis.is_empty() { None } else { Some(emojis) };
        self
    }

    /// Takes the theme from a unique gift. It can't be mixed with a backdrop or emojis.
    pub fn gift(mut self, owned_gift_id: impl Into<String>) -> Self {
        self.gift = Some(owned_gift_id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let theme = match self.gift {
            Some(_) if self.backdrop_id.is_some() || self.emojis.is_some() => {
                return Err(Error::InvalidArgument("a gift theme can't have a backdrop or emojis"));
            }
            Some(gift) => {
                let stargift = self.client.input_saved_gift(&gift).await?;
                base::ShareTheme::InputShareThemeGift(Box::new(types::InputShareThemeGift { stargift }))
            }
            None if self.backdrop_id.is_some() || self.emojis.is_some() => {
                let emojis = self.emojis.map(|e| e.iter().map(ReactionType::write).collect());
                base::ShareTheme::ShareThemeEmojiBackdrop(Box::new(types::ShareThemeEmojiBackdrop {
                    backdrop_id: self.backdrop_id,
                    emojis,
                }))
            }
            None => base::ShareTheme::ShareThemeDefault,
        };
        let peer = self.client.resolve_peer(self.chat_id).await?;
        self.client.invoke(&functions::messages::SetShareTheme { peer, theme }).await?;
        Ok(())
    }
}

into_future!(SetShareTheme, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ReactionType;

    fn client() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes()), Ok(true.to_bytes())]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        client.0.peers.lock().unwrap().insert(5, peer);
        (client, sent)
    }

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }))
    }

    #[tokio::test]
    async fn set_theme_picks_constructor() {
        let (client, mut sent) = client();
        client.set_share_theme(5_i64).await.unwrap();
        let default = functions::messages::SetShareTheme { peer: peer(), theme: base::ShareTheme::ShareThemeDefault };
        assert_eq!(sent.try_recv().unwrap(), default.to_bytes());

        client.set_share_theme(5_i64).backdrop_id(3).emojis([ReactionType::Emoji("🐱".into())]).await.unwrap();
        let fire = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "🐱".into() }));
        let backdrop = types::ShareThemeEmojiBackdrop { backdrop_id: Some(3), emojis: Some(vec![fire]) };
        let backdrop = base::ShareTheme::ShareThemeEmojiBackdrop(Box::new(backdrop));
        assert_eq!(
            sent.try_recv().unwrap(),
            functions::messages::SetShareTheme { peer: peer(), theme: backdrop }.to_bytes()
        );

        client.set_share_theme(5_i64).gift("PlushPepe-3").await.unwrap();
        let stargift = base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }));
        let gift = base::ShareTheme::InputShareThemeGift(Box::new(types::InputShareThemeGift { stargift }));
        assert_eq!(
            sent.try_recv().unwrap(),
            functions::messages::SetShareTheme { peer: peer(), theme: gift }.to_bytes()
        );
    }

    #[tokio::test]
    async fn gift_with_backdrop_is_invalid() {
        let (client, mut sent) = client();
        let result = client.set_share_theme(5_i64).gift("PlushPepe-3").backdrop_id(3).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        let result = client.set_share_theme(5_i64).gift("PlushPepe-3").emojis([ReactionType::Emoji("🐱".into())]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());

        // An empty emoji list is unset: the gift goes through, and alone it means the default theme.
        client.set_share_theme(5_i64).gift("PlushPepe-3").emojis([]).await.unwrap();
        let body = sent.try_recv().unwrap();
        let stargift = base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }));
        let gift = base::ShareTheme::InputShareThemeGift(Box::new(types::InputShareThemeGift { stargift }));
        assert_eq!(body, functions::messages::SetShareTheme { peer: peer(), theme: gift }.to_bytes());
        client.set_share_theme(5_i64).emojis([]).await.unwrap();
        let default = functions::messages::SetShareTheme { peer: peer(), theme: base::ShareTheme::ShareThemeDefault };
        assert_eq!(sent.try_recv().unwrap(), default.to_bytes());
    }
}
