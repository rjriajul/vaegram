use crate::raw::base;

/// One step of a report started with [`Client::report_messages`](crate::Client::report_messages).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportResult {
    /// Pick one of `options` and report again with its `option`.
    ChooseOption {
        title: String,
        options: Vec<ReportOption>,
    },
    /// Report again with this `option` and a comment; the comment may be empty when `optional`.
    AddComment {
        optional: bool,
        option: Vec<u8>,
    },
    Reported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOption {
    pub text: String,
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
