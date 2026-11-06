use crate::raw::{base, types};

/// The boost level of a chat and the progress toward the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BoostsStatus {
    /// The chat's current boost level.
    pub level: i32,
    /// Total boosts the chat has now.
    pub boosts: i32,
    /// Total boosts the current level started at.
    pub current_level_boosts: i32,
    /// Total boosts the next level starts at. `None` at the maximum level.
    pub next_level_boosts: Option<i32>,
    /// Boosts that came from gift codes and giveaways. `0` if Telegram omits the count.
    pub gift_boosts: i32,
    /// Approximate number of Premium users among the chat's subscribers. Only admins get it.
    pub premium_members: Option<i32>,
    /// Link people open to boost the chat.
    pub boost_url: String,
    /// `true` if you boost the chat yourself.
    pub has_my_boost: bool,
    /// Your boost slots assigned to this chat.
    pub my_boost_slots: Vec<i32>,
}

impl BoostsStatus {
    pub(crate) fn parse(raw: types::premium::BoostsStatus) -> BoostsStatus {
        BoostsStatus {
            level: raw.level,
            boosts: raw.boosts,
            current_level_boosts: raw.current_level_boosts,
            next_level_boosts: raw.next_level_boosts,
            gift_boosts: raw.gift_boosts.unwrap_or_default(),
            premium_members: raw.premium_audience.map(|base::StatsPercentValue::StatsPercentValue(p)| p.part as i32),
            boost_url: raw.boost_url,
            has_my_boost: raw.my_boost,
            my_boost_slots: raw.my_boost_slots.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoostsStatus;
    use crate::raw::{base, types};

    #[test]
    fn fills_missing_counters() {
        let audience = types::StatsPercentValue { part: 12.0, total: 40.0 };
        let raw = types::premium::BoostsStatus {
            my_boost: false,
            level: 1,
            current_level_boosts: 1,
            boosts: 3,
            gift_boosts: None,
            next_level_boosts: Some(7),
            premium_audience: Some(base::StatsPercentValue::StatsPercentValue(Box::new(audience))),
            boost_url: "https://t.me/boost/x".into(),
            prepaid_giveaways: None,
            my_boost_slots: None,
        };
        let status = BoostsStatus::parse(raw);
        assert_eq!((status.gift_boosts, status.premium_members, status.my_boost_slots), (0, Some(12), vec![]));
    }
}
