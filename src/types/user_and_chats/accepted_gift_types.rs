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

/// Accepts every kind, as Telegram does for an account that set no restriction.
impl Default for AcceptedGiftTypes {
    fn default() -> Self {
        AcceptedGiftTypes {
            unlimited_gifts: true,
            limited_gifts: true,
            unique_gifts: true,
            premium_subscription: true,
            gifts_from_channels: true,
        }
    }
}

impl AcceptedGiftTypes {
    /// Accepts every kind, like `Default`; turn kinds off with the setters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether the account accepts gifts with unlimited supply.
    #[must_use]
    pub fn unlimited_gifts(mut self, unlimited_gifts: bool) -> Self {
        self.unlimited_gifts = unlimited_gifts;
        self
    }

    /// Sets whether the account accepts gifts with limited supply.
    #[must_use]
    pub fn limited_gifts(mut self, limited_gifts: bool) -> Self {
        self.limited_gifts = limited_gifts;
        self
    }

    /// Sets whether the account accepts unique gifts, such as upgraded or transferred ones.
    #[must_use]
    pub fn unique_gifts(mut self, unique_gifts: bool) -> Self {
        self.unique_gifts = unique_gifts;
        self
    }

    /// Sets whether the account accepts gifted Telegram Premium subscriptions.
    #[must_use]
    pub fn premium_subscription(mut self, premium_subscription: bool) -> Self {
        self.premium_subscription = premium_subscription;
        self
    }

    /// Sets whether the account accepts gifts sent by channels.
    #[must_use]
    pub fn gifts_from_channels(mut self, gifts_from_channels: bool) -> Self {
        self.gifts_from_channels = gifts_from_channels;
        self
    }

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

    #[test]
    fn new_accepts_every_kind_until_a_setter_turns_one_off() {
        let base::DisallowedGiftsSettings::DisallowedGiftsSettings(all) = AcceptedGiftTypes::new().write();
        assert!(
            !all.disallow_unlimited_stargifts && !all.disallow_premium_gifts && !all.disallow_stargifts_from_channels
        );
        let no_channels = AcceptedGiftTypes::new().gifts_from_channels(false);
        assert!(no_channels.unlimited_gifts && no_channels.unique_gifts && !no_channels.gifts_from_channels);
        let base::DisallowedGiftsSettings::DisallowedGiftsSettings(d) = no_channels.write();
        assert!(d.disallow_stargifts_from_channels && !d.disallow_limited_stargifts);
    }
}
