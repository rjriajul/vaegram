use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, LinkPreviewOptions, Media, MessageOrigin};

/// The message a reply points to when that message is in another chat.
#[derive(Debug, Clone)]
pub struct ExternalReplyInfo {
    /// Where the replied-to message came from.
    pub origin: MessageOrigin,
    /// The chat the replied-to message is in. `None` if Telegram didn't send it.
    pub chat: Option<Chat>,
    /// Id of the replied-to message in `chat`. `None` if `chat` is `None`.
    pub message_id: Option<i32>,
    /// Media of the replied-to message. `None` if it has none.
    pub media: Option<Media>,
    /// Link preview of the replied-to message. `None` if it has none.
    pub link_preview_options: Option<LinkPreviewOptions>,
}

impl ExternalReplyInfo {
    pub(crate) fn parse(
        client: &Client,
        reply: &types::MessageReplyHeader,
        entities: &Entities,
    ) -> Option<ExternalReplyInfo> {
        let origin = MessageOrigin::parse(reply.reply_from.as_ref()?, entities)?;
        let is_channel = matches!(origin, MessageOrigin::Channel { .. });
        let chat = reply.reply_to_peer_id.as_ref().map(|peer| Chat::parse(peer, entities, is_channel));
        let media = reply.reply_media.as_ref();
        Some(ExternalReplyInfo {
            origin,
            message_id: chat.as_ref().and(reply.reply_to_msg_id),
            chat,
            media: media.and_then(|m| Media::parse(client, m, entities)),
            link_preview_options: LinkPreviewOptions::parse(media, false),
        })
    }
}
