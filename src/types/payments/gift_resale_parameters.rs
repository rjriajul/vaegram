use crate::raw::base;

/// The price a unique gift is listed at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GiftResaleParameters {
    /// The price in Stars; `None` when the seller takes only Toncoin.
    pub star_count: Option<i64>,
    /// The price in nanotons (1 TON = 10^9), when the seller accepts Toncoin.
    pub toncoin_nano_count: Option<i64>,
    /// `true` when the seller accepts Toncoin only, so `star_count` is `None`.
    pub is_toncoin_only: bool,
}

impl GiftResaleParameters {
    /// `None` when the gift isn't listed for sale.
    pub(crate) fn parse(prices: Option<&[base::StarsAmount]>, toncoin_only: bool) -> Option<GiftResaleParameters> {
        let prices = prices?;
        let star_count = prices.iter().find_map(|p| match p {
            base::StarsAmount::StarsAmount(a) if !toncoin_only => Some(a.amount),
            _ => None,
        });
        let toncoin_nano_count = prices.iter().find_map(|p| match p {
            base::StarsAmount::StarsTonAmount(a) => Some(a.amount),
            _ => None,
        });
        Some(GiftResaleParameters { star_count, toncoin_nano_count, is_toncoin_only: toncoin_only })
    }
}

#[cfg(test)]
mod tests {
    use super::GiftResaleParameters;
    use crate::raw::{base, types};

    #[test]
    fn resale_parameters_split_stars_and_ton() {
        let ton = base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 2_500_000_000 });
        let stars = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 900, nanos: 0 }));
        let both = GiftResaleParameters::parse(Some(&[stars.clone(), ton.clone()]), false).unwrap();
        assert_eq!(
            (both.star_count, both.toncoin_nano_count, both.is_toncoin_only),
            (Some(900), Some(2_500_000_000), false)
        );
        let only_ton = GiftResaleParameters::parse(Some(&[stars, ton]), true).unwrap();
        assert_eq!(
            (only_ton.star_count, only_ton.toncoin_nano_count, only_ton.is_toncoin_only),
            (None, Some(2_500_000_000), true)
        );
        assert_eq!(GiftResaleParameters::parse(None, false), None);
    }
}
