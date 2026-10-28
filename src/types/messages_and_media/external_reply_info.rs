use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, LinkPreviewOptions, Media, MessageOrigin};

#[derive(Debug, Clone)]
pub struct ExternalReplyInfo {
    pub origin: MessageOrigin,
    pub chat: Option<Chat>,
    pub message_id: Option<i32>,
    pub media: Option<Media>,
    pub link_preview_options: Option<LinkPreviewOptions>,
}

impl ExternalReplyInfo {
    pub(crate) fn parse(reply: &types::MessageReplyHeader, entities: &Entities) -> Option<ExternalReplyInfo> {
        let origin = MessageOrigin::parse(reply.reply_from.as_ref()?, entities)?;
        let is_channel = matches!(origin, MessageOrigin::Channel { .. });
        let chat = reply.reply_to_peer_id.as_ref().map(|peer| Chat::parse(peer, entities, is_channel));
        let media = reply.reply_media.as_ref();
        Some(ExternalReplyInfo {
            origin,
            message_id: chat.as_ref().and(reply.reply_to_msg_id),
            chat,
            media: media.and_then(|m| Media::parse(m, entities)),
            link_preview_options: LinkPreviewOptions::parse(media, false),
        })
    }
}
