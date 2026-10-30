use crate::raw::{base, types};
use crate::types::AcceptedGiftTypes;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlobalPrivacySettings {
    pub archive_and_mute_new_noncontact_peers: bool,
    pub keep_archived_unmuted: bool,
    pub keep_archived_folders: bool,
    pub hide_read_marks: bool,
    pub new_noncontact_peers_require_premium: bool,
    pub display_gifts_button: bool,
    pub noncontact_peers_paid_stars: Option<i64>,
    pub accepted_gift_types: Option<AcceptedGiftTypes>,
}

impl GlobalPrivacySettings {
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
    use crate::types::AcceptedGiftTypes;

    #[test]
    fn survives_a_round_trip() {
        let gifts = AcceptedGiftTypes {
            unlimited_gifts: true,
            limited_gifts: false,
            unique_gifts: true,
            premium_subscription: false,
            gifts_from_channels: true,
        };
        let settings = GlobalPrivacySettings {
            keep_archived_folders: true,
            hide_read_marks: true,
            noncontact_peers_paid_stars: Some(25),
            accepted_gift_types: Some(gifts),
            ..GlobalPrivacySettings::default()
        };
        assert_eq!(GlobalPrivacySettings::parse(settings.write()), settings);
    }
}
