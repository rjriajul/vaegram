use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineBotResults {
    pub query_id: i64,
    /// Pass to `.offset()` for the next page; `None` on the last one.
    pub next_offset: Option<String>,
    pub gallery: bool,
    pub results: Vec<InlineBotResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineBotResult {
    pub id: String,
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
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
