use crate::raw::{base, types};

/// The kinds of gifts an account accepts. A field is `true` when that kind is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedGiftTypes {
    /// Gifts with unlimited supply.
    pub unlimited_gifts: bool,
    /// Gifts with limited supply.
    pub limited_gifts: bool,
    /// Unique gifts, such as upgraded or transferred ones.
    pub unique_gifts: bool,
    /// Gifted Telegram Premium subscriptions.
    pub premium_subscription: bool,
    /// Gifts sent by channels.
    pub gifts_from_channels: bool,
}

impl AcceptedGiftTypes {
    pub(crate) fn parse(settings: &base::DisallowedGiftsSettings) -> AcceptedGiftTypes {
        let base::DisallowedGiftsSettings::DisallowedGiftsSettings(d) = settings;
        AcceptedGiftTypes {
            unlimited_gifts: !d.disallow_unlimited_stargifts,
            limited_gifts: !d.disallow_limited_stargifts,
            unique_gifts: !d.disallow_unique_stargifts,
            premium_subscription: !d.disallow_premium_gifts,
            gifts_from_channels: !d.disallow_stargifts_from_channels,
        }
    }

    pub(crate) fn write(self) -> base::DisallowedGiftsSettings {
        base::DisallowedGiftsSettings::DisallowedGiftsSettings(types::DisallowedGiftsSettings {
            disallow_unlimited_stargifts: !self.unlimited_gifts,
            disallow_limited_stargifts: !self.limited_gifts,
            disallow_unique_stargifts: !self.unique_gifts,
            disallow_premium_gifts: !self.premium_subscription,
            disallow_stargifts_from_channels: !self.gifts_from_channels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptedGiftTypes;
    use crate::raw::base;

    #[test]
    fn disallows_what_is_not_accepted() {
        let accepted = AcceptedGiftTypes {
            unlimited_gifts: true,
            limited_gifts: false,
            unique_gifts: true,
            premium_subscription: false,
            gifts_from_channels: true,
        };
        let base::DisallowedGiftsSettings::DisallowedGiftsSettings(d) = accepted.write();
        assert_eq!(
            (
                d.disallow_unlimited_stargifts,
                d.disallow_limited_stargifts,
                d.disallow_unique_stargifts,
                d.disallow_premium_gifts,
                d.disallow_stargifts_from_channels
            ),
            (false, true, false, true, false)
        );
    }
}
