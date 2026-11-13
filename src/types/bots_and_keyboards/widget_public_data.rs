use crate::raw::base;

/// The result of polling a widget's public data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WidgetPublicData {
    /// The public data has changed.
    #[non_exhaustive]
    Modified {
        /// Hash to pass to the next poll.
        hash: i64,
        /// Current public data.
        data: String,
        /// Seconds to wait until polling again.
        period: i32,
    },
    /// The supplied hash still matches the public data.
    #[non_exhaustive]
    NotModified {
        /// Seconds to wait until polling again.
        period: i32,
    },
}

impl WidgetPublicData {
    pub(crate) fn parse(raw: base::messages::WidgetPublicData) -> Self {
        match raw {
            base::messages::WidgetPublicData::WidgetPublicData(data) => {
                Self::Modified { hash: data.hash, data: data.data, period: data.period }
            }
            base::messages::WidgetPublicData::WidgetPublicDataNotModified(data) => {
                Self::NotModified { period: data.period }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetPublicData;
    use crate::raw::{base, types};

    #[test]
    fn parses_modified_and_unchanged_public_data() {
        let modified =
            base::messages::WidgetPublicData::WidgetPublicData(Box::new(types::messages::WidgetPublicData {
                hash: 19,
                data: "data".into(),
                period: 30,
            }));
        let unchanged = base::messages::WidgetPublicData::WidgetPublicDataNotModified(
            types::messages::WidgetPublicDataNotModified { period: 60 },
        );
        assert_eq!(
            WidgetPublicData::parse(modified),
            WidgetPublicData::Modified { hash: 19, data: "data".into(), period: 30 }
        );
        assert_eq!(WidgetPublicData::parse(unchanged), WidgetPublicData::NotModified { period: 60 });
    }
}
