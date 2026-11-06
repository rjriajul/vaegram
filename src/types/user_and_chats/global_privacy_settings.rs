use crate::raw::{base, types};
use crate::types::AcceptedGiftTypes;

/// Account-wide privacy settings for chats with people who aren't contacts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlobalPrivacySettings {
    /// Whether chats from new non-contacts are archived and muted.
    pub archive_and_mute_new_noncontact_peers: bool,
    /// Whether unmuted archived chats stay in the archive when new messages arrive.
    pub keep_archived_unmuted: bool,
    /// Whether unmuted archived chats that a folder includes or pins stay in the archive on new messages.
    ///
    /// Telegram ignores it when `keep_archived_unmuted` is set.
    pub keep_archived_folders: bool,
    /// Whether to hide when the account has read messages.
    pub hide_read_marks: bool,
    /// Whether non-contacts need Premium to message the account.
    pub new_noncontact_peers_require_premium: bool,
    /// Whether to show the gifts button in chats with the account.
    pub display_gifts_button: bool,
    /// Stars a non-contact pays to message the account. `None` when messages are free.
    pub noncontact_peers_paid_stars: Option<i64>,
    /// The gift types the account accepts. `None` when Telegram sent no restriction.
    pub accepted_gift_types: Option<AcceptedGiftTypes>,
}

impl GlobalPrivacySettings {
    /// Every setting off, no price for messages from non-contacts and no gift restriction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether chats from new non-contacts are archived and muted.
    #[must_use]
    pub fn archive_and_mute_new_noncontact_peers(mut self, archive_and_mute_new_noncontact_peers: bool) -> Self {
        self.archive_and_mute_new_noncontact_peers = archive_and_mute_new_noncontact_peers;
        self
    }

    /// Sets whether unmuted archived chats stay in the archive when new messages arrive.
    #[must_use]
    pub fn keep_archived_unmuted(mut self, keep_archived_unmuted: bool) -> Self {
        self.keep_archived_unmuted = keep_archived_unmuted;
        self
    }

    /// Sets whether unmuted archived chats that a folder includes or pins stay in the archive on new messages.
    #[must_use]
    pub fn keep_archived_folders(mut self, keep_archived_folders: bool) -> Self {
        self.keep_archived_folders = keep_archived_folders;
        self
    }

    /// Sets whether to hide when the account has read messages.
    #[must_use]
    pub fn hide_read_marks(mut self, hide_read_marks: bool) -> Self {
        self.hide_read_marks = hide_read_marks;
        self
    }

    /// Sets whether non-contacts need Premium to message the account.
    #[must_use]
    pub fn new_noncontact_peers_require_premium(mut self, new_noncontact_peers_require_premium: bool) -> Self {
        self.new_noncontact_peers_require_premium = new_noncontact_peers_require_premium;
        self
    }

    /// Sets whether to show the gifts button in chats with the account.
    #[must_use]
    pub fn display_gifts_button(mut self, display_gifts_button: bool) -> Self {
        self.display_gifts_button = display_gifts_button;
        self
    }

    /// Charges non-contacts this many Stars per message.
    #[must_use]
    pub fn noncontact_peers_paid_stars(mut self, noncontact_peers_paid_stars: i64) -> Self {
        self.noncontact_peers_paid_stars = Some(noncontact_peers_paid_stars);
        self
    }

    /// Limits the gifts the account accepts to these kinds.
    #[must_use]
    pub fn accepted_gift_types(mut self, accepted_gift_types: AcceptedGiftTypes) -> Self {
        self.accepted_gift_types = Some(accepted_gift_types);
        self
    }

    pub(crate) fn parse(raw: base::GlobalPrivacySettings) -> GlobalPrivacySettings {
        let base::GlobalPrivacySettings::GlobalPrivacySettings(s) = raw;
        GlobalPrivacySettings {
            archive_and_mute_new_noncontact_peers: s.archive_and_mute_new_noncontact_peers,
            keep_archived_unmuted: s.keep_archived_unmuted,
            keep_archived_folders: s.keep_archived_folders,
            hide_read_marks: s.hide_read_marks,
            new_noncontact_peers_require_premium: s.new_noncontact_peers_require_premium,
            display_gifts_button: s.display_gifts_button,
            noncontact_peers_paid_stars: s.noncontact_peers_paid_stars,
            accepted_gift_types: s.disallowed_gifts.as_ref().map(AcceptedGiftTypes::parse),
        }
    }

    pub(crate) fn write(self) -> base::GlobalPrivacySettings {
        base::GlobalPrivacySettings::GlobalPrivacySettings(Box::new(types::GlobalPrivacySettings {
            archive_and_mute_new_noncontact_peers: self.archive_and_mute_new_noncontact_peers,
            keep_archived_unmuted: self.keep_archived_unmuted,
            keep_archived_folders: self.keep_archived_folders,
            hide_read_marks: self.hide_read_marks,
            new_noncontact_peers_require_premium: self.new_noncontact_peers_require_premium,
            display_gifts_button: self.display_gifts_button,
            noncontact_peers_paid_stars: self.noncontact_peers_paid_stars,
            disallowed_gifts: self.accepted_gift_types.map(AcceptedGiftTypes::write),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::GlobalPrivacySettings;
    use crate::raw::base;
    use crate::types::AcceptedGiftTypes;

    #[test]
    fn survives_a_round_trip() {
        let gifts = AcceptedGiftTypes::new().limited_gifts(false).premium_subscription(false);
        let settings = GlobalPrivacySettings {
            keep_archived_folders: true,
            hide_read_marks: true,
            noncontact_peers_paid_stars: Some(25),
            accepted_gift_types: Some(gifts),
            ..GlobalPrivacySettings::default()
        };
        assert_eq!(GlobalPrivacySettings::parse(settings.write()), settings);
    }

    #[test]
    fn builds_from_new_and_setters() {
        let settings = GlobalPrivacySettings::new()
            .hide_read_marks(true)
            .noncontact_peers_paid_stars(25)
            .accepted_gift_types(AcceptedGiftTypes::new().unique_gifts(false));
        assert!(settings.hide_read_marks && !settings.keep_archived_unmuted && !settings.display_gifts_button);
        assert_eq!(settings.noncontact_peers_paid_stars, Some(25));
        let base::GlobalPrivacySettings::GlobalPrivacySettings(raw) = settings.write();
        let Some(base::DisallowedGiftsSettings::DisallowedGiftsSettings(gifts)) = raw.disallowed_gifts else {
            panic!("no gift settings written");
        };
        assert!(gifts.disallow_unique_stargifts && !gifts.disallow_limited_stargifts);
    }
}
