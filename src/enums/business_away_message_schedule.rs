use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusinessAwayMessageSchedule {
    Always,
    OutsideWorkHours,
    Custom { start_date: i32, end_date: i32 },
}

impl BusinessAwayMessageSchedule {
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
