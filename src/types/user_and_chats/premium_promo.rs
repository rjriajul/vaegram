use crate::raw::{base, types};

/// What Telegram currently offers for Premium.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PremiumPromo {
    /// Status line describing the offer, as Telegram words it.
    pub status_text: String,
    /// Subscription periods on offer.
    pub options: Vec<PremiumSubscriptionOption>,
}

/// One Premium subscription period and its price.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PremiumSubscriptionOption {
    /// Length of the subscription in months.
    pub months: i32,
    /// Three-letter ISO 4217 currency code of `amount`.
    pub currency: String,
    /// Price in the smallest unit of `currency`.
    pub amount: i64,
    /// Link to the bot that sells this option.
    pub bot_url: String,
    /// `true` for the subscription you have now.
    pub is_current: bool,
    /// This option can be bought as an upgrade of your current subscription.
    pub can_purchase_upgrade: bool,
}

impl PremiumPromo {
    pub(crate) fn parse(raw: types::help::PremiumPromo) -> PremiumPromo {
        let options = raw
            .period_options
            .into_iter()
            .map(|base::PremiumSubscriptionOption::PremiumSubscriptionOption(o)| PremiumSubscriptionOption {
                months: o.months,
                currency: o.currency,
                amount: o.amount,
                bot_url: o.bot_url,
                is_current: o.current,
                can_purchase_upgrade: o.can_purchase_upgrade,
            })
            .collect();
        PremiumPromo { status_text: raw.status_text, options }
    }
}

#[cfg(test)]
mod tests {
    use super::PremiumPromo;
    use crate::raw::{base, types};

    #[test]
    fn keeps_the_options_and_survives_none() {
        let option = types::PremiumSubscriptionOption {
            current: true,
            can_purchase_upgrade: false,
            transaction: None,
            months: 12,
            currency: "USD".into(),
            amount: 2999,
            bot_url: "https://t.me/PremiumBot".into(),
            store_product: None,
        };
        let promo = |period_options| types::help::PremiumPromo {
            status_text: "Premium".into(),
            status_entities: Vec::new(),
            video_sections: Vec::new(),
            videos: Vec::new(),
            period_options,
            users: Vec::new(),
        };
        let options = vec![base::PremiumSubscriptionOption::PremiumSubscriptionOption(Box::new(option))];
        let parsed = PremiumPromo::parse(promo(options));
        assert_eq!((parsed.options.len(), parsed.options[0].months, parsed.options[0].is_current), (1, 12, true));
        assert!(PremiumPromo::parse(promo(Vec::new())).options.is_empty());
    }
}
