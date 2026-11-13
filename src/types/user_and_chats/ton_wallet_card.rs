use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{StoryAreaPosition, UniqueGift};

/// A TON wallet card shown on a profile or shared with a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TonWalletCard {
    /// The wallet address.
    pub address: String,
    /// The wallet balance in nanotons.
    pub amount: i64,
    /// NFT stickers placed on the card.
    pub stickers: Vec<TonWalletCardSticker>,
    /// Address of the NFT used as the background.
    pub background_nft_address: Option<String>,
}

/// An NFT sticker placed on a TON wallet card.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TonWalletCardSticker {
    /// Where the sticker sits on the card.
    pub position: StoryAreaPosition,
    /// The NFT shown by the sticker.
    pub nft: TonNft,
}

// Bitwise coordinate equality keeps wallet cards Eq, including coordinates containing NaNs.
impl PartialEq for TonWalletCardSticker {
    fn eq(&self, other: &Self) -> bool {
        let a = &self.position;
        let b = &other.position;
        a.x_percentage.to_bits() == b.x_percentage.to_bits()
            && a.y_percentage.to_bits() == b.y_percentage.to_bits()
            && a.width_percentage.to_bits() == b.width_percentage.to_bits()
            && a.height_percentage.to_bits() == b.height_percentage.to_bits()
            && a.rotation_angle.to_bits() == b.rotation_angle.to_bits()
            && a.corner_radius_percentage.to_bits() == b.corner_radius_percentage.to_bits()
            && self.nft == other.nft
    }
}

impl Eq for TonWalletCardSticker {}

/// An NFT on the TON blockchain.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TonNft {
    /// The NFT address.
    pub address: String,
    /// Address of the collection containing the NFT.
    pub collection_address: Option<String>,
    /// Index within its collection, as Telegram sends it.
    pub index: String,
    /// Name of the NFT.
    pub name: Option<String>,
    /// Description of the NFT.
    pub description: Option<String>,
    /// Attributes of the NFT.
    pub attributes: Vec<TonNftAttribute>,
    /// The unique Telegram gift represented by the NFT, if any.
    pub gift: Option<UniqueGift>,
}

/// A named attribute of a TON NFT.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TonNftAttribute {
    /// Name of the attribute.
    pub trait_type: String,
    /// Value of the attribute.
    pub value: String,
}

impl TonWalletCard {
    pub(crate) fn parse(raw: &base::wallet::Card, entities: &Entities) -> Self {
        let base::wallet::Card::Card(card) = raw;
        TonWalletCard {
            address: card.address.clone(),
            amount: card.balance,
            stickers: card
                .stickers
                .iter()
                .flatten()
                .map(|raw| {
                    let base::wallet::CardSticker::CardSticker(sticker) = raw;
                    let base::MediaAreaCoordinates::MediaAreaCoordinates(c) = &sticker.coordinates;
                    TonWalletCardSticker {
                        position: StoryAreaPosition {
                            x_percentage: c.x,
                            y_percentage: c.y,
                            width_percentage: c.w,
                            height_percentage: c.h,
                            rotation_angle: c.rotation,
                            corner_radius_percentage: c.radius.unwrap_or_default(),
                        },
                        nft: TonNft::parse(&sticker.nft, entities),
                    }
                })
                .collect(),
            background_nft_address: card.bg_nft_item.as_ref().map(|raw| {
                let base::wallet::NftItem::NftItem(nft) = raw;
                nft.address.clone()
            }),
        }
    }
}

impl TonNft {
    fn parse(raw: &base::wallet::NftItem, entities: &Entities) -> Self {
        let base::wallet::NftItem::NftItem(nft) = raw;
        TonNft {
            address: nft.address.clone(),
            collection_address: nft.collection_address.clone(),
            index: nft.index.clone(),
            name: nft.name.clone(),
            description: nft.description.clone(),
            attributes: nft
                .attributes
                .iter()
                .flatten()
                .map(|raw| {
                    let base::wallet::NftAttribute::NftAttribute(attribute) = raw;
                    TonNftAttribute { trait_type: attribute.trait_type.clone(), value: attribute.value.clone() }
                })
                .collect(),
            gift: match &nft.gift {
                Some(base::StarGift::StarGiftUnique(gift)) => Some(UniqueGift::parse(gift, entities)),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::TonWalletCard;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    pub(crate) fn wallet_card() -> base::wallet::Card {
        let nft = base::wallet::NftItem::NftItem(Box::new(types::wallet::NftItem {
            collection_address: Some("collection".into()),
            address: "nft".into(),
            owner_address: "wallet".into(),
            index: "18446744073709551616".into(),
            name: Some("Cat".into()),
            description: Some("A cat".into()),
            image: None,
            image_small: None,
            content_url: None,
            lottie: None,
            attributes: Some(vec![base::wallet::NftAttribute::NftAttribute(Box::new(types::wallet::NftAttribute {
                trait_type: "Color".into(),
                value: "Blue".into(),
            }))]),
            extra: None,
            gift: None,
        }));
        base::wallet::Card::Card(Box::new(types::wallet::Card {
            stickers: Some(vec![base::wallet::CardSticker::CardSticker(Box::new(types::wallet::CardSticker {
                coordinates: base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
                    x: 10.0,
                    y: 20.0,
                    w: 30.0,
                    h: 40.0,
                    rotation: 45.0,
                    radius: Some(5.0),
                })),
                nft: nft.clone(),
            }))]),
            bg_nft_item: Some(nft),
            address: "wallet".into(),
            balance: 1234,
        }))
    }

    #[test]
    fn parses_wallet_card_stickers_and_nft_metadata() {
        let card = TonWalletCard::parse(&wallet_card(), &Entities::default());
        assert_eq!(
            (card.address.as_str(), card.amount, card.background_nft_address.as_deref()),
            ("wallet", 1234, Some("nft"))
        );
        assert_eq!(card.stickers.len(), 1);
        let sticker = &card.stickers[0];
        assert_eq!((sticker.position.x_percentage, sticker.position.y_percentage), (10.0, 20.0));
        assert_eq!((sticker.position.width_percentage, sticker.position.height_percentage), (30.0, 40.0));
        assert_eq!((sticker.position.rotation_angle, sticker.position.corner_radius_percentage), (45.0, 5.0));
        let nft = &sticker.nft;
        assert_eq!(
            (nft.address.as_str(), nft.collection_address.as_deref(), nft.index.as_str()),
            ("nft", Some("collection"), "18446744073709551616")
        );
        assert_eq!((nft.name.as_deref(), nft.description.as_deref()), (Some("Cat"), Some("A cat")));
        assert_eq!((nft.attributes[0].trait_type.as_str(), nft.attributes[0].value.as_str()), ("Color", "Blue"));
    }

    #[test]
    fn sticker_equality_is_reflexive_for_nan_coordinates() {
        let mut card = TonWalletCard::parse(&wallet_card(), &Entities::default());
        card.stickers[0].position.x_percentage = f64::NAN;
        assert_eq!(card, card.clone());
    }
}
