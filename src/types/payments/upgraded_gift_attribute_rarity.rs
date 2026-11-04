use crate::raw::base;

/// How rare a unique gift's model, symbol or backdrop is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UpgradedGiftAttributeRarity {
    /// The attribute shows up in this many of every thousand gifts.
    PerMille(i32),
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl UpgradedGiftAttributeRarity {
    pub(crate) fn parse(raw: &base::StarGiftAttributeRarity) -> UpgradedGiftAttributeRarity {
        match raw {
            base::StarGiftAttributeRarity::StarGiftAttributeRarity(r) => {
                UpgradedGiftAttributeRarity::PerMille(r.permille)
            }
            base::StarGiftAttributeRarity::StarGiftAttributeRarityUncommon => UpgradedGiftAttributeRarity::Uncommon,
            base::StarGiftAttributeRarity::StarGiftAttributeRarityRare => UpgradedGiftAttributeRarity::Rare,
            base::StarGiftAttributeRarity::StarGiftAttributeRarityEpic => UpgradedGiftAttributeRarity::Epic,
            base::StarGiftAttributeRarity::StarGiftAttributeRarityLegendary => UpgradedGiftAttributeRarity::Legendary,
        }
    }

    /// The share out of 1000, or `None` for the named tiers.
    pub fn per_mille(self) -> Option<i32> {
        match self {
            UpgradedGiftAttributeRarity::PerMille(n) => Some(n),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpgradedGiftAttributeRarity as R;
    use crate::raw::{base, types};

    #[test]
    fn every_rarity_tier_parses() {
        let parse = |r| R::parse(&r);
        let exact =
            base::StarGiftAttributeRarity::StarGiftAttributeRarity(types::StarGiftAttributeRarity { permille: 15 });
        assert_eq!(parse(exact), R::PerMille(15));
        assert_eq!(R::PerMille(15).per_mille(), Some(15));
        assert_eq!(parse(base::StarGiftAttributeRarity::StarGiftAttributeRarityUncommon), R::Uncommon);
        assert_eq!(parse(base::StarGiftAttributeRarity::StarGiftAttributeRarityRare), R::Rare);
        assert_eq!(parse(base::StarGiftAttributeRarity::StarGiftAttributeRarityEpic), R::Epic);
        assert_eq!(parse(base::StarGiftAttributeRarity::StarGiftAttributeRarityLegendary), R::Legendary);
        assert_eq!(R::Epic.per_mille(), None);
    }
}
