use crate::Client;
use crate::raw::base;
use crate::types::BotPreviewMedia;

/// The preview media of a bot's profile page.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BotPreviewInfo {
    /// The preview items.
    pub media: Vec<BotPreviewMedia>,
    /// Language codes the bot has previews for.
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
