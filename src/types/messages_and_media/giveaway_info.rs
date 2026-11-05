use crate::raw::base;
use crate::utils::get_channel_id;

/// The state of a giveaway, as seen by the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiveawayInfo {
    /// The giveaway hasn't finished yet.
    Ongoing {
        /// When the giveaway started.
        start_date: i32,
        /// The current user is taking part.
        is_participating: bool,
        /// The winners are being picked.
        is_preparing_results: bool,
        /// When the user joined the channel, if that was too early to qualify.
        joined_too_early_date: Option<i32>,
        /// Id of the channel whose admin status keeps the user out, if that's the reason.
        admin_disallowed_chat_id: Option<i64>,
        /// Country that keeps the user out of the giveaway, if that's the reason.
        disallowed_country: Option<String>,
    },
    /// The winners have been picked.
    Finished {
        /// When the giveaway started.
        start_date: i32,
        /// When the winners were picked.
        finish_date: i32,
        /// The current user won.
        is_winner: bool,
        /// The giveaway was cancelled and its prizes refunded.
        is_refunded: bool,
        /// Slug of the user's gift code, if they won a Premium prize.
        gift_code_slug: Option<String>,
        /// Stars each winner got, for a Star giveaway.
        prize_star_count: Option<i64>,
        /// How many winners were picked.
        winner_count: i32,
        /// How many winners have activated their gift codes.
        activated_count: Option<i32>,
    },
}

impl GiveawayInfo {
    pub(crate) fn parse(raw: base::payments::GiveawayInfo) -> GiveawayInfo {
        use base::payments::GiveawayInfo as G;
        match raw {
            G::GiveawayInfo(g) => GiveawayInfo::Ongoing {
                start_date: g.start_date,
                is_participating: g.participating,
                is_preparing_results: g.preparing_results,
                joined_too_early_date: g.joined_too_early_date,
                admin_disallowed_chat_id: g.admin_disallowed_chat_id.map(get_channel_id),
                disallowed_country: g.disallowed_country,
            },
            G::GiveawayInfoResults(g) => GiveawayInfo::Finished {
                start_date: g.start_date,
                finish_date: g.finish_date,
                is_winner: g.winner,
                is_refunded: g.refunded,
                gift_code_slug: g.gift_code_slug,
                prize_star_count: g.stars_prize,
                winner_count: g.winners_count,
                activated_count: g.activated_count,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GiveawayInfo;
    use crate::raw::{base, types};

    #[test]
    fn maps_both_giveaway_states() {
        let ongoing = types::payments::GiveawayInfo {
            participating: true,
            preparing_results: false,
            start_date: 1,
            joined_too_early_date: None,
            admin_disallowed_chat_id: Some(77),
            disallowed_country: None,
        };
        let GiveawayInfo::Ongoing { is_participating, admin_disallowed_chat_id, .. } =
            GiveawayInfo::parse(base::payments::GiveawayInfo::GiveawayInfo(Box::new(ongoing)))
        else {
            panic!("expected an ongoing giveaway")
        };
        assert_eq!((is_participating, admin_disallowed_chat_id), (true, Some(-1_000_000_000_077)));

        let finished = types::payments::GiveawayInfoResults {
            winner: true,
            refunded: false,
            start_date: 1,
            gift_code_slug: Some("abc".into()),
            stars_prize: None,
            finish_date: 2,
            winners_count: 3,
            activated_count: None,
        };
        let parsed = GiveawayInfo::parse(base::payments::GiveawayInfo::GiveawayInfoResults(Box::new(finished)));
        assert_eq!(
            parsed,
            GiveawayInfo::Finished {
                start_date: 1,
                finish_date: 2,
                is_winner: true,
                is_refunded: false,
                gift_code_slug: Some("abc".into()),
                prize_star_count: None,
                winner_count: 3,
                activated_count: None,
            }
        );
    }
}
