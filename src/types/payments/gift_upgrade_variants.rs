use super::gift::{attribute, backdrop};
use crate::raw::base;
use crate::types::{UniqueGiftAttribute, UniqueGiftBackdrop};

/// The models, symbols and backdrops a gift can get when it's upgraded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GiftUpgradeVariants {
    pub models: Vec<UniqueGiftAttribute>,
    pub symbols: Vec<UniqueGiftAttribute>,
    pub backdrops: Vec<UniqueGiftBackdrop>,
}

impl GiftUpgradeVariants {
    pub(crate) fn parse(raw: &[base::StarGiftAttribute]) -> GiftUpgradeVariants {
        let mut variants = GiftUpgradeVariants::default();
        for raw_attribute in raw {
            match raw_attribute {
                base::StarGiftAttribute::StarGiftAttributeModel(a) => {
                    variants.models.extend(attribute(&a.name, &a.document, &a.rarity))
                }
                base::StarGiftAttribute::StarGiftAttributePattern(a) => {
                    variants.symbols.extend(attribute(&a.name, &a.document, &a.rarity))
                }
                base::StarGiftAttribute::StarGiftAttributeBackdrop(a) => variants.backdrops.push(backdrop(a)),
                base::StarGiftAttribute::StarGiftAttributeOriginalDetails(_) => {}
            }
        }
        variants
    }
}

#[cfg(test)]
mod tests {
    use super::GiftUpgradeVariants;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::media::tests::document_media;

    fn document(mime_type: &str, attributes: Vec<base::DocumentAttribute>) -> base::Document {
        let base::MessageMedia::MessageMediaDocument(media) = document_media(mime_type, attributes) else {
            panic!("expected a document")
        };
        media.document.expect("a document")
    }

    fn sticker() -> base::Document {
        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        document("image/webp", vec![sticker])
    }

    fn rare(permille: i32) -> base::StarGiftAttributeRarity {
        base::StarGiftAttributeRarity::StarGiftAttributeRarity(types::StarGiftAttributeRarity { permille })
    }

    fn attributes() -> Vec<base::StarGiftAttribute> {
        let model = types::StarGiftAttributeModel {
            crafted: false,
            name: "Pepe".into(),
            document: sticker(),
            rarity: rare(15),
        };
        let pattern = types::StarGiftAttributePattern { name: "Star".into(), document: sticker(), rarity: rare(20) };
        let backdrop = types::StarGiftAttributeBackdrop {
            name: "Night".into(),
            backdrop_id: 1,
            center_color: 1,
            edge_color: 2,
            pattern_color: 3,
            text_color: 4,
            rarity: base::StarGiftAttributeRarity::StarGiftAttributeRarityRare,
        };
        let details = types::StarGiftAttributeOriginalDetails {
            sender_id: None,
            recipient_id: crate::utils::get_peer(5),
            date: 1,
            message: None,
        };
        let not_a_sticker = types::StarGiftAttributeModel {
            crafted: false,
            name: "Photo".into(),
            document: document("image/jpeg", Vec::new()),
            rarity: rare(1),
        };
        vec![
            base::StarGiftAttribute::StarGiftAttributeModel(Box::new(model)),
            base::StarGiftAttribute::StarGiftAttributePattern(Box::new(pattern)),
            base::StarGiftAttribute::StarGiftAttributeBackdrop(Box::new(backdrop)),
            base::StarGiftAttribute::StarGiftAttributeOriginalDetails(Box::new(details)),
            base::StarGiftAttribute::StarGiftAttributeModel(Box::new(not_a_sticker)),
        ]
    }

    #[test]
    fn keeps_models_symbols_and_backdrops_only() {
        let variants = GiftUpgradeVariants::parse(&attributes());
        let names = |a: &[crate::types::UniqueGiftAttribute]| a.iter().map(|a| a.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&variants.models), ["Pepe"], "the original details and a non-sticker are dropped");
        assert_eq!(variants.models[0].rarity_per_mille, Some(15));
        assert_eq!(names(&variants.symbols), ["Star"]);
        assert_eq!(variants.backdrops.len(), 1);
        let night = &variants.backdrops[0];
        assert_eq!(
            (night.name.as_str(), night.center_color, night.symbol_color, night.rarity_per_mille),
            ("Night", 1, 3, None)
        );
    }
}
