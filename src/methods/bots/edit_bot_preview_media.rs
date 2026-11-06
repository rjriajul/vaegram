use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, InputMedia, PeerRef};

/// Request built by [`Client::edit_bot_preview_media`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditBotPreviewMedia<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    old: &'a BotPreviewMedia,
    new: InputMedia<'a>,
    language_code: String,
}

impl Client {
    /// Replaces one of your bot's default preview media, shown in languages without previews of their own, with a
    /// new photo or video.
    ///
    /// [`language_code`](EditBotPreviewMedia::language_code) edits one language's previews instead. Returns the new
    /// [`BotPreviewMedia`]. Any other media kind fails with `Error::InvalidArgument`.
    pub fn edit_bot_preview_media<'a>(
        &'a self,
        bot_id: impl Into<PeerRef>,
        old: &'a BotPreviewMedia,
        new: InputMedia<'a>,
    ) -> EditBotPreviewMedia<'a> {
        EditBotPreviewMedia { client: self, bot_id: bot_id.into(), old, new, language_code: String::new() }
    }
}

impl EditBotPreviewMedia<'_> {
    /// Edits the previews for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<BotPreviewMedia> {
        let media = self.old.input()?;
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let new_media = self.client.upload_preview(&self.new).await?;
        let request = functions::bots::EditPreviewMedia { bot, lang_code: self.language_code, media, new_media };
        Ok(BotPreviewMedia::parse(self.client, self.client.invoke(&request).await?))
    }
}

into_future!(EditBotPreviewMedia, BotPreviewMedia);
