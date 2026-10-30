use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoostsStatus {
    pub level: i32,
    pub boosts: i32,
    pub current_level_boosts: i32,
    pub next_level_boosts: Option<i32>,
    pub gift_boosts: i32,
    pub premium_members: Option<i32>,
    pub boost_url: String,
    pub my_boost: bool,
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
            my_boost: raw.my_boost,
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
