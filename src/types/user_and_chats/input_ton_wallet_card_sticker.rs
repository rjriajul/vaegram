use crate::raw::{base, types};
use crate::types::StoryAreaPosition;

/// An NFT sticker to place on a TON wallet card.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InputTonWalletCardSticker {
    /// Where the sticker sits on the card.
    pub position: StoryAreaPosition,
    /// Address of the NFT sticker.
    pub nft_address: String,
}

impl InputTonWalletCardSticker {
    /// Creates a new wallet card sticker placed at `position`.
    pub fn new(position: StoryAreaPosition, nft_address: impl Into<String>) -> Self {
        InputTonWalletCardSticker { position, nft_address: nft_address.into() }
    }

    pub(crate) fn write(&self) -> base::InputWalletCardSticker {
        base::InputWalletCardSticker::InputWalletCardSticker(Box::new(types::InputWalletCardSticker {
            coordinates: self.position.write(),
            nft_address: self.nft_address.clone(),
        }))
    }
}
