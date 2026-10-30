use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::UniqueGift;

#[derive(Debug, Clone)]
pub struct ResaleGift {
    pub gift: UniqueGift,
    /// `None` when the seller takes only TON.
    pub star_count: Option<i64>,
}

impl ResaleGift {
    pub(crate) fn parse(raw: &types::StarGiftUnique, entities: &Entities) -> ResaleGift {
        let star_count = if raw.resale_ton_only { None } else { star_price(raw.resell_amount.as_deref()) };
        ResaleGift { gift: UniqueGift::parse(raw, entities), star_count }
    }
}

// Sellers can list a price in Stars, in TON, or both.
fn star_price(prices: Option<&[base::StarsAmount]>) -> Option<i64> {
    prices?.iter().find_map(|p| match p {
        base::StarsAmount::StarsAmount(a) => Some(a.amount),
        base::StarsAmount::StarsTonAmount(_) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::star_price;
    use crate::raw::{base, types};

    #[test]
    fn picks_the_stars_price() {
        let ton = base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 5 });
        let stars = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 900, nanos: 0 }));
        assert_eq!(star_price(Some(&[ton.clone(), stars])), Some(900));
        assert_eq!(star_price(Some(&[ton])), None);
        assert_eq!(star_price(None), None);
    }
}
