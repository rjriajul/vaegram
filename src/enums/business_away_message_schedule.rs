use crate::raw::{base, types};

/// When a business account sends its away message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusinessAwayMessageSchedule {
    /// At all times.
    Always,
    /// Outside the account's configured work hours.
    OutsideWorkHours,
    /// Only between two dates.
    Custom {
        /// Unix time when sending starts.
        start_date: i32,
        /// Unix time when sending ends.
        end_date: i32,
    },
}

impl BusinessAwayMessageSchedule {
    pub(crate) fn parse(raw: &base::BusinessAwayMessageSchedule) -> BusinessAwayMessageSchedule {
        use base::BusinessAwayMessageSchedule as S;
        match raw {
            S::BusinessAwayMessageScheduleAlways => Self::Always,
            S::BusinessAwayMessageScheduleOutsideWorkHours => Self::OutsideWorkHours,
            S::BusinessAwayMessageScheduleCustom(c) => Self::Custom { start_date: c.start_date, end_date: c.end_date },
        }
    }

    pub(crate) fn write(self) -> base::BusinessAwayMessageSchedule {
        use base::BusinessAwayMessageSchedule as S;
        match self {
            Self::Always => S::BusinessAwayMessageScheduleAlways,
            Self::OutsideWorkHours => S::BusinessAwayMessageScheduleOutsideWorkHours,
            Self::Custom { start_date, end_date } => {
                let custom = types::BusinessAwayMessageScheduleCustom { start_date, end_date };
                S::BusinessAwayMessageScheduleCustom(custom)
            }
        }
    }
}
