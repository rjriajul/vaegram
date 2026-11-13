use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputTonWalletCardSticker, TonWalletCard};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::set_ton_wallet_card_stickers`].
pub struct SetTonWalletCardStickers<'a> {
    client: &'a Client,
    stickers: Vec<InputTonWalletCardSticker>,
    background_nft_address: Option<String>,
}

impl Client {
    /// Sets or updates the stickers and background on the account's TON wallet card.
    ///
    /// User accounts only. Returns the updated [`TonWalletCard`].
    pub fn set_ton_wallet_card_stickers(
        &self,
        stickers: impl IntoIterator<Item = InputTonWalletCardSticker>,
    ) -> SetTonWalletCardStickers<'_> {
        SetTonWalletCardStickers {
            client: self,
            stickers: stickers.into_iter().collect(),
            background_nft_address: None,
        }
    }
}

impl SetTonWalletCardStickers<'_> {
    /// Sets the address of the NFT used as the card's background.
    pub fn background_nft_address(mut self, address: impl Into<String>) -> Self {
        self.background_nft_address = Some(address.into());
        self
    }

    fn request(&self) -> functions::wallet::SetCardStickers {
        functions::wallet::SetCardStickers {
            stickers: self.stickers.iter().map(InputTonWalletCardSticker::write).collect(),
            bg_nft_address: self.background_nft_address.clone(),
        }
    }

    async fn send(self) -> Result<TonWalletCard> {
        let raw = self.client.invoke(&self.request()).await?;
        Ok(TonWalletCard::parse(&raw, &Entities::default()))
    }
}

into_future!(SetTonWalletCardStickers, TonWalletCard);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::wallet::SetCardStickers;
    use crate::raw::{Serializable, base, types};
    use crate::types::{AreaPosition, InputTonWalletCardSticker};

    #[tokio::test]
    async fn set_ton_wallet_card_stickers_sends_request_bytes() {
        let reply = crate::types::user_and_chats::ton_wallet_card::tests::wallet_card().to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let position = AreaPosition::new(10.0, 20.0, 30.0, 40.0).rotation_angle(45.0).corner_radius_percentage(5.0);
        let sticker = InputTonWalletCardSticker::new(position, "EQDsticker_address");
        let card =
            client.set_ton_wallet_card_stickers([sticker]).background_nft_address("EQDbg_nft_address").await.unwrap();

        assert_eq!(card.address, "wallet");

        let expected_coords = base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            rotation: 45.0,
            radius: Some(5.0),
        }));
        let expected_sticker =
            base::InputWalletCardSticker::InputWalletCardSticker(Box::new(types::InputWalletCardSticker {
                coordinates: expected_coords,
                nft_address: "EQDsticker_address".into(),
            }));
        let expected =
            SetCardStickers { stickers: vec![expected_sticker], bg_nft_address: Some("EQDbg_nft_address".into()) };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
