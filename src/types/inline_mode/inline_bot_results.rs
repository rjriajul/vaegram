use crate::raw::base;

/// One page of results returned by a bot for an inline query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InlineBotResults {
    /// Id of the query these results answer.
    pub query_id: i64,
    /// Pass to `.offset()` for the next page; `None` on the last one.
    pub next_offset: Option<String>,
    /// Set when the results should be shown as a media gallery instead of a list.
    pub gallery: bool,
    /// The results on this page.
    pub results: Vec<InlineBotResult>,
}

/// A single result in [`InlineBotResults`], reduced to its listing fields.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InlineBotResult {
    /// Id of the result, as the bot set it.
    pub id: String,
    /// Result type the bot set, such as `article`, `photo` or `gif`.
    pub kind: String,
    /// Title of the result. `None` if the bot gave none.
    pub title: Option<String>,
    /// Short description of the result. `None` if the bot gave none.
    pub description: Option<String>,
    /// URL attached to the result. Always `None` for media results.
    pub url: Option<String>,
}

impl InlineBotResults {
    pub(crate) fn parse(raw: base::messages::BotResults) -> InlineBotResults {
        let base::messages::BotResults::BotResults(r) = raw;
        let results = r
            .results
            .into_iter()
            .map(|result| match result {
                base::BotInlineResult::BotInlineResult(r) => {
                    InlineBotResult { id: r.id, kind: r.r#type, title: r.title, description: r.description, url: r.url }
                }
                base::BotInlineResult::BotInlineMediaResult(r) => {
                    InlineBotResult { id: r.id, kind: r.r#type, title: r.title, description: r.description, url: None }
                }
            })
            .collect();
        InlineBotResults { query_id: r.query_id, next_offset: r.next_offset, gallery: r.gallery, results }
    }
}

#[cfg(test)]
mod tests {
    use super::{InlineBotResult, InlineBotResults};
    use crate::raw::{base, types};

    #[test]
    fn media_results_have_no_url() {
        let message = base::BotInlineMessage::BotInlineMessageMediaAuto(Box::new(types::BotInlineMessageMediaAuto {
            invert_media: false,
            message: String::new(),
            entities: None,
            reply_markup: None,
        }));
        let media = types::BotInlineMediaResult {
            id: "1".into(),
            r#type: "gif".into(),
            photo: None,
            document: None,
            title: Some("cat".into()),
            description: None,
            send_message: message,
        };
        let raw = types::messages::BotResults {
            gallery: true,
            query_id: 7,
            next_offset: Some("20".into()),
            switch_pm: None,
            switch_webview: None,
            results: vec![base::BotInlineResult::BotInlineMediaResult(Box::new(media))],
            cache_time: 0,
            users: Vec::new(),
        };
        let results = InlineBotResults::parse(base::messages::BotResults::BotResults(Box::new(raw)));
        let gif = InlineBotResult {
            id: "1".into(),
            kind: "gif".into(),
            title: Some("cat".into()),
            description: None,
            url: None,
        };
        let expected =
            InlineBotResults { query_id: 7, next_offset: Some("20".into()), gallery: true, results: vec![gif] };
        assert_eq!(results, expected);
    }
}
