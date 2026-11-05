use crate::raw::base;

/// One step of a report started with [`Client::report_messages`](crate::Client::report_messages).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportResult {
    /// Pick one of `options` and report again with its `option`.
    ChooseOption {
        /// The prompt to show above the options.
        title: String,
        /// The choices to offer.
        options: Vec<ReportOption>,
    },
    /// Report again with this `option` and a comment; the comment may be empty when `optional`.
    AddComment {
        /// Whether the comment may be left empty.
        optional: bool,
        /// Opaque value to pass back when reporting again.
        option: Vec<u8>,
    },
    /// The report was accepted; there are no more steps.
    Reported,
}

/// A choice offered during a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOption {
    /// The label to show.
    pub text: String,
    /// Opaque value to pass back when reporting again.
    pub option: Vec<u8>,
}

impl ReportResult {
    pub(crate) fn parse(raw: base::ReportResult) -> ReportResult {
        match raw {
            base::ReportResult::ReportResultChooseOption(r) => ReportResult::ChooseOption {
                title: r.title,
                options: r
                    .options
                    .into_iter()
                    .map(|base::MessageReportOption::MessageReportOption(o)| ReportOption {
                        text: o.text,
                        option: o.option,
                    })
                    .collect(),
            },
            base::ReportResult::ReportResultAddComment(r) => {
                ReportResult::AddComment { optional: r.optional, option: r.option }
            }
            base::ReportResult::ReportResultReported => ReportResult::Reported,
        }
    }
}
