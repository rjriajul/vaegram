use crate::errors::{Error, Result};
use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessOpeningHours {
    pub time_zone_name: String,
    pub opening_hours: Vec<(i32, i32)>,
}

impl BusinessOpeningHours {
    pub fn new(time_zone_name: impl Into<String>, opening_hours: impl IntoIterator<Item = (i32, i32)>) -> Self {
        let (time_zone_name, opening_hours) = (time_zone_name.into(), opening_hours.into_iter().collect());
        BusinessOpeningHours { time_zone_name, opening_hours }
    }

    /// Minutes count from Monday 00:00; an interval may run past Sunday into the next week, up to 8*24*60.
    pub(crate) fn write(&self) -> Result<base::BusinessWorkHours> {
        const WEEK: i32 = 7 * 24 * 60;
        if self.time_zone_name.is_empty() {
            return Err(Error::InvalidArgument("opening hours need a time zone name"));
        }
        let valid = |&(start, end): &(i32, i32)| {
            (0..=WEEK).contains(&start) && end > start && end <= (start + WEEK).min(8 * 24 * 60)
        };
        if !self.opening_hours.iter().all(valid) {
            return Err(Error::InvalidArgument("opening hours must lie within the week and have a positive length"));
        }
        let open = |(start_minute, end_minute)| {
            let open = types::BusinessWeeklyOpen { start_minute, end_minute };
            base::BusinessWeeklyOpen::BusinessWeeklyOpen(open)
        };
        let weekly_open = self.opening_hours.iter().copied().map(open).collect();
        let hours = types::BusinessWorkHours { open_now: false, timezone_id: self.time_zone_name.clone(), weekly_open };
        Ok(base::BusinessWorkHours::BusinessWorkHours(Box::new(hours)))
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessOpeningHours;
    use crate::errors::Error;
    use crate::raw::base;

    #[test]
    fn writes_minutes_as_weekly_open() {
        let hours = BusinessOpeningHours::new("Europe/Berlin", [(540, 1020)]);
        let base::BusinessWorkHours::BusinessWorkHours(h) = hours.write().unwrap();
        let base::BusinessWeeklyOpen::BusinessWeeklyOpen(o) = &h.weekly_open[0];
        let got = (h.timezone_id.as_str(), h.weekly_open.len(), o.start_minute, o.end_minute);
        assert_eq!(got, ("Europe/Berlin", 1, 540, 1020));
    }

    #[test]
    fn a_sunday_night_interval_may_run_into_monday() {
        let night = (6 * 24 * 60 + 21 * 60, 7 * 24 * 60 + 4 * 60);
        assert!(BusinessOpeningHours::new("Europe/Berlin", [night]).write().is_ok());
    }

    #[test]
    fn impossible_hours_are_refused() {
        let week = 7 * 24 * 60;
        let bad = [(10, 10), (20, 10), (-1, 5), (week + 1, week + 2), (0, 8 * 24 * 60 + 1), (30, 30 + week + 1)];
        for interval in bad {
            let hours = BusinessOpeningHours::new("Europe/Berlin", [interval]);
            assert!(matches!(hours.write(), Err(Error::InvalidArgument(_))), "{interval:?}");
        }
        let nameless = BusinessOpeningHours::new("", [(1, 2)]);
        assert!(matches!(nameless.write(), Err(Error::InvalidArgument(_))));
    }
}
