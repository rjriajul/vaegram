use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;
use crate::utils::{get_channel_id, get_peer};

/// A giveaway announced in a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Giveaway {
    /// The channels a user must join to take part.
    pub chats: Vec<Chat>,
    /// When the winners are picked, as a Unix timestamp.
    pub winners_selection_date: i32,
    /// How many winners are picked.
    pub winner_count: i32,
    /// `true` if only users who join after the giveaway starts can win.
    pub only_new_members: bool,
    /// `true` if everyone can see the winners once they're picked.
    pub has_public_winners: bool,
    /// Extra prize text from the organizers. `None` if there is none.
    pub prize_description: Option<String>,
    /// Two-letter codes of the countries users must be from. Empty if users from any country can take part.
    pub country_codes: Vec<String>,
    /// Stars split between the winners. `None` unless it's a Star giveaway.
    pub prize_star_count: Option<i64>,
    /// Months of Premium each winner gets. `None` unless it's a Premium giveaway.
    pub premium_subscription_month_count: Option<i32>,
}

impl Giveaway {
    pub(crate) fn parse(raw: &types::MessageMediaGiveaway, entities: &Entities) -> Giveaway {
        Giveaway {
            chats: raw.channels.iter().map(|&id| Chat::parse(&get_peer(get_channel_id(id)), entities, true)).collect(),
            winners_selection_date: raw.until_date,
            winner_count: raw.quantity,
            only_new_members: raw.only_new_subscribers,
            has_public_winners: raw.winners_are_visible,
            prize_description: raw.prize_description.clone(),
            country_codes: raw.countries_iso2.clone().unwrap_or_default(),
            prize_star_count: raw.stars,
            premium_subscription_month_count: raw.months,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Giveaway;
    use crate::dispatcher::Entities;
    use crate::raw::types;
    use crate::utils::get_channel_id;

    pub(crate) fn giveaway() -> types::MessageMediaGiveaway {
        types::MessageMediaGiveaway {
            only_new_subscribers: true,
            winners_are_visible: false,
            channels: vec![9, 10],
            countries_iso2: Some(vec!["DE".into(), "FR".into()]),
            prize_description: None,
            quantity: 3,
            months: None,
            stars: Some(500),
            until_date: 1_900_000_000,
        }
    }

    #[test]
    fn reads_channels_countries_and_stars() {
        let parsed = Giveaway::parse(&giveaway(), &Entities::default());
        let chats: Vec<_> = parsed.chats.iter().map(|c| c.id).collect();
        assert_eq!(chats, [get_channel_id(9), get_channel_id(10)]);
        assert_eq!(
            (parsed.country_codes, parsed.prize_star_count, parsed.winner_count),
            (vec!["DE".into(), "FR".into()], Some(500), 3)
        );
        assert!(parsed.only_new_members && !parsed.has_public_winners);
        assert_eq!((parsed.premium_subscription_month_count, parsed.winners_selection_date), (None, 1_900_000_000));

        let worldwide = types::MessageMediaGiveaway { countries_iso2: None, channels: Vec::new(), ..giveaway() };
        let parsed = Giveaway::parse(&worldwide, &Entities::default());
        assert!(parsed.country_codes.is_empty() && parsed.chats.is_empty());
    }

    #[test]
    fn unknown_giveaway_chats_are_channels() {
        let parsed = Giveaway::parse(&giveaway(), &Entities::default());
        assert!(parsed.chats.iter().all(|c| c.kind == crate::enums::ChatType::Channel));
    }
}
