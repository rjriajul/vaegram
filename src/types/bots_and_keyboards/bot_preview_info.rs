use crate::Client;
use crate::raw::base;
use crate::types::BotPreviewMedia;

#[derive(Debug, Clone)]
pub struct BotPreviewInfo {
    pub media: Vec<BotPreviewMedia>,
    pub lang_codes: Vec<String>,
}

impl BotPreviewInfo {
    pub(crate) fn parse(client: &Client, raw: base::bots::PreviewInfo) -> BotPreviewInfo {
        let base::bots::PreviewInfo::PreviewInfo(r) = raw;
        BotPreviewInfo {
            media: r.media.into_iter().map(|m| BotPreviewMedia::parse(client, m)).collect(),
            lang_codes: r.lang_codes,
        }
    }
}
