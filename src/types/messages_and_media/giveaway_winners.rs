use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, User};
use crate::utils::{get_channel_id, get_peer};

/// The public results of a finished giveaway.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GiveawayWinners {
    /// The channel that ran the giveaway.
    pub chat: Chat,
    /// The message that announced the giveaway, in `chat`.
    pub giveaway_message_id: i32,
    /// When the winners were picked, as a Unix timestamp.
    pub winners_selection_date: i32,
    /// How many winners were picked in total.
    pub winner_count: i32,
    /// The winners Telegram lists. There can be fewer than `winner_count`.
    pub winners: Vec<User>,
    /// How many other channels users had to join. `None` if there were none.
    pub additional_chat_count: Option<i32>,
    /// Stars split between the winners. `None` unless it was a Star giveaway.
    pub prize_star_count: Option<i64>,
    /// Months of Premium each winner gets. `None` unless it was a Premium giveaway.
    pub premium_subscription_month_count: Option<i32>,
    /// How many prizes nobody claimed.
    pub unclaimed_prize_count: i32,
    /// `true` if only users who joined after the giveaway started could win.
    pub only_new_members: bool,
    /// `true` if the giveaway was cancelled and its prizes refunded.
    pub was_refunded: bool,
    /// Extra prize text from the organizers. `None` if there is none.
    pub prize_description: Option<String>,
}

impl GiveawayWinners {
    pub(crate) fn parse(raw: &types::MessageMediaGiveawayResults, entities: &Entities) -> GiveawayWinners {
        GiveawayWinners {
            chat: Chat::parse(&get_peer(get_channel_id(raw.channel_id)), entities, true),
            giveaway_message_id: raw.launch_msg_id,
            winners_selection_date: raw.until_date,
            winner_count: raw.winners_count,
            winners: raw.winners.iter().map(|&id| User::parse(id, entities)).collect(),
            additional_chat_count: raw.additional_peers_count,
            prize_star_count: raw.stars,
            premium_subscription_month_count: raw.months,
            unclaimed_prize_count: raw.unclaimed_count,
            only_new_members: raw.only_new_subscribers,
            was_refunded: raw.refunded,
            prize_description: raw.prize_description.clone(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::GiveawayWinners;
    use crate::dispatcher::Entities;
    use crate::raw::types;
    use crate::utils::get_channel_id;

    pub(crate) fn results() -> types::MessageMediaGiveawayResults {
        types::MessageMediaGiveawayResults {
            only_new_subscribers: false,
            refunded: true,
            channel_id: 9,
            additional_peers_count: Some(2),
            launch_msg_id: 40,
            winners_count: 3,
            unclaimed_count: 1,
            winners: vec![7, 8],
            months: Some(6),
            stars: None,
            prize_description: Some("Mugs".into()),
            until_date: 1_900_000_000,
        }
    }

    #[test]
    fn reads_winners_and_the_refund() {
        let parsed = GiveawayWinners::parse(&results(), &Entities::default());
        assert_eq!((parsed.chat.id, parsed.giveaway_message_id, parsed.winner_count), (get_channel_id(9), 40, 3));
        assert_eq!(parsed.winners.iter().map(|u| u.id).collect::<Vec<_>>(), [7, 8]);
        assert_eq!((parsed.additional_chat_count, parsed.premium_subscription_month_count), (Some(2), Some(6)));
        assert_eq!((parsed.unclaimed_prize_count, parsed.prize_description.as_deref()), (1, Some("Mugs")));
        assert!(parsed.was_refunded && !parsed.only_new_members && parsed.prize_star_count.is_none());
    }

    #[test]
    fn an_unknown_giveaway_chat_is_a_channel() {
        let parsed = GiveawayWinners::parse(&results(), &Entities::default());
        assert_eq!(parsed.chat.kind, crate::enums::ChatType::Channel);
    }
}
