use crate::raw::base;

/// One step of a report started with [`Client::report_messages`](crate::Client::report_messages).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportResult {
    /// Telegram wants a reason. Report again with the `option` of one of `options`.
    #[non_exhaustive]
    ChooseOption {
        /// Prompt to show above the options.
        title: String,
        /// Reasons to choose from.
        options: Vec<ReportOption>,
    },
    /// Telegram wants a comment. Report again with this `option` and a comment.
    #[non_exhaustive]
    AddComment {
        /// `true` if the comment may be left empty.
        optional: bool,
        /// Opaque value to pass to [`ReportMessages::option`](crate::methods::ReportMessages::option).
        option: Vec<u8>,
    },
    /// Telegram accepted the report. There are no more steps.
    Reported,
}

/// A choice offered during a report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReportOption {
    /// Label to show for this reason.
    pub text: String,
    /// Opaque value to pass to [`ReportMessages::option`](crate::methods::ReportMessages::option).
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
