use crate::Client;
use crate::enums::ChatAction;
use crate::methods::{
    OnBehalf, Routable, SendAnimation, SendAudio, SendCachedMedia, SendChatAction, SendChecklist, SendContact,
    SendDice, SendDocument, SendGame, SendInlineBotResult, SendInvoice, SendLivePhoto, SendLocation, SendPaidMedia,
    SendPhoto, SendPoll, SendRichMessage, SendSharedWidget, SendSticker, SendVenue, SendVideo, SendVideoNote,
    SendVoice, SendWidget,
};
use crate::types::{
    FileSource, InputChecklist, InputInvoice, InputMedia, InputPollOption, InputRichMessage, Message, StickerSource,
    Text,
};

impl Message {
    /// Points a send at this message's topic, direct-messages topic and business connection.
    pub(super) fn route<B: Routable>(&self, mut send: B) -> B {
        send.options().business_connection_id.clone_from(&self.business_connection_id);
        self.route_topic(send)
    }

    /// Points a send at this message's forum topic and direct-messages topic.
    fn route_topic<B: Routable>(&self, mut send: B) -> B {
        let options = send.options();
        options.direct_messages_topic_id = self.direct_messages_topic.as_ref().map(|topic| topic.topic_id.into());
        if self.is_topic_message {
            options.message_thread_id = self.message_thread_id;
        }
        send
    }

    /// Sends an animation as a reply to this message.
    pub fn reply_animation<'a>(
        &'a self,
        client: &'a Client,
        animation: impl Into<FileSource<'a>>,
    ) -> SendAnimation<'a> {
        self.answer_animation(client, animation).reply_to(self.id)
    }

    /// Sends an animation to this chat, in the same topic and through the same business connection.
    pub fn answer_animation<'a>(
        &'a self,
        client: &'a Client,
        animation: impl Into<FileSource<'a>>,
    ) -> SendAnimation<'a> {
        self.route(client.send_animation(self.chat.id, animation))
    }

    /// Sends an audio file as a reply to this message.
    pub fn reply_audio<'a>(&'a self, client: &'a Client, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        self.answer_audio(client, audio).reply_to(self.id)
    }

    /// Sends an audio file to this chat, in the same topic and through the same business connection.
    pub fn answer_audio<'a>(&'a self, client: &'a Client, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        self.route(client.send_audio(self.chat.id, audio))
    }

    /// Sends a file by its file id as a reply to this message.
    pub fn reply_cached_media<'a>(&'a self, client: &'a Client, file_id: impl Into<String>) -> SendCachedMedia<'a> {
        self.answer_cached_media(client, file_id).reply_to(self.id)
    }

    /// Sends a file by its file id to this chat, in the same topic and through the same business connection.
    pub fn answer_cached_media<'a>(&'a self, client: &'a Client, file_id: impl Into<String>) -> SendCachedMedia<'a> {
        self.route(client.send_cached_media(self.chat.id, file_id))
    }

    /// Sends a checklist as a reply to this message.
    pub fn reply_checklist<'a>(&'a self, client: &'a Client, checklist: InputChecklist) -> SendChecklist<'a> {
        self.answer_checklist(client, checklist).reply_to(self.id)
    }

    /// Sends a checklist to this chat, in the same topic and through the same business connection.
    pub fn answer_checklist<'a>(&'a self, client: &'a Client, checklist: InputChecklist) -> SendChecklist<'a> {
        self.route(client.send_checklist(self.chat.id, checklist))
    }

    /// Sends a contact as a reply to this message.
    pub fn reply_contact<'a>(
        &'a self,
        client: &'a Client,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> SendContact<'a> {
        self.answer_contact(client, phone_number, first_name).reply_to(self.id)
    }

    /// Sends a contact to this chat, in the same topic and through the same business connection.
    pub fn answer_contact<'a>(
        &'a self,
        client: &'a Client,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> SendContact<'a> {
        self.route(client.send_contact(self.chat.id, phone_number, first_name))
    }

    /// Sends a die as a reply to this message.
    pub fn reply_dice<'a>(&'a self, client: &'a Client) -> SendDice<'a> {
        self.answer_dice(client).reply_to(self.id)
    }

    /// Sends a die to this chat, in the same topic and through the same business connection.
    pub fn answer_dice<'a>(&'a self, client: &'a Client) -> SendDice<'a> {
        self.route(client.send_dice(self.chat.id))
    }

    /// Sends a document as a reply to this message.
    pub fn reply_document<'a>(&'a self, client: &'a Client, document: impl Into<FileSource<'a>>) -> SendDocument<'a> {
        self.answer_document(client, document).reply_to(self.id)
    }

    /// Sends a document to this chat, in the same topic and through the same business connection.
    pub fn answer_document<'a>(&'a self, client: &'a Client, document: impl Into<FileSource<'a>>) -> SendDocument<'a> {
        self.route(client.send_document(self.chat.id, document))
    }

    /// Sends a game as a reply to this message.
    pub fn reply_game<'a>(&'a self, client: &'a Client, game_short_name: impl Into<String>) -> SendGame<'a> {
        self.answer_game(client, game_short_name).reply_to(self.id)
    }

    /// Sends a game to this chat, in the same topic and through the same business connection.
    pub fn answer_game<'a>(&'a self, client: &'a Client, game_short_name: impl Into<String>) -> SendGame<'a> {
        self.route(client.send_game(self.chat.id, game_short_name))
    }

    /// Sends an invoice as a reply to this message.
    pub fn reply_invoice<'a>(&'a self, client: &'a Client, invoice: InputInvoice) -> SendInvoice<'a> {
        self.answer_invoice(client, invoice).reply_to(self.id)
    }

    /// Sends an invoice to this chat, in the same topic and through the same business connection.
    pub fn answer_invoice<'a>(&'a self, client: &'a Client, invoice: InputInvoice) -> SendInvoice<'a> {
        self.route(client.send_invoice(self.chat.id, invoice))
    }

    /// Sends a live photo as a reply to this message.
    pub fn reply_live_photo<'a>(
        &'a self,
        client: &'a Client,
        photo: impl Into<FileSource<'a>>,
        live_photo: impl Into<FileSource<'a>>,
    ) -> SendLivePhoto<'a> {
        self.answer_live_photo(client, photo, live_photo).reply_to(self.id)
    }

    /// Sends a live photo to this chat, in the same topic and through the same business connection.
    pub fn answer_live_photo<'a>(
        &'a self,
        client: &'a Client,
        photo: impl Into<FileSource<'a>>,
        live_photo: impl Into<FileSource<'a>>,
    ) -> SendLivePhoto<'a> {
        self.route(client.send_live_photo(self.chat.id, photo, live_photo))
    }

    /// Sends a location as a reply to this message.
    pub fn reply_location<'a>(&'a self, client: &'a Client, latitude: f64, longitude: f64) -> SendLocation<'a> {
        self.answer_location(client, latitude, longitude).reply_to(self.id)
    }

    /// Sends a location to this chat, in the same topic and through the same business connection.
    pub fn answer_location<'a>(&'a self, client: &'a Client, latitude: f64, longitude: f64) -> SendLocation<'a> {
        self.route(client.send_location(self.chat.id, latitude, longitude))
    }

    /// Sends paid media as a reply to this message.
    pub fn reply_paid_media<'a>(
        &'a self,
        client: &'a Client,
        star_count: i64,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendPaidMedia<'a> {
        self.answer_paid_media(client, star_count, media).reply_to(self.id)
    }

    /// Sends paid media to this chat, in the same topic and through the same business connection.
    pub fn answer_paid_media<'a>(
        &'a self,
        client: &'a Client,
        star_count: i64,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendPaidMedia<'a> {
        self.route(client.send_paid_media(self.chat.id, star_count, media))
    }

    /// Sends a photo as a reply to this message.
    pub fn reply_photo<'a>(&'a self, client: &'a Client, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        self.answer_photo(client, photo).reply_to(self.id)
    }

    /// Sends a photo to this chat, in the same topic and through the same business connection.
    pub fn answer_photo<'a>(&'a self, client: &'a Client, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        self.route(client.send_photo(self.chat.id, photo))
    }

    /// Sends a poll as a reply to this message.
    pub fn reply_poll<'a>(
        &'a self,
        client: &'a Client,
        question: impl Into<Text>,
        options: impl IntoIterator<Item = impl Into<InputPollOption<'a>>>,
    ) -> SendPoll<'a> {
        self.answer_poll(client, question, options).reply_to(self.id)
    }

    /// Sends a poll to this chat, in the same topic and through the same business connection.
    pub fn answer_poll<'a>(
        &'a self,
        client: &'a Client,
        question: impl Into<Text>,
        options: impl IntoIterator<Item = impl Into<InputPollOption<'a>>>,
    ) -> SendPoll<'a> {
        self.route(client.send_poll(self.chat.id, question, options))
    }

    /// Sends a rich message as a reply to this message.
    pub fn reply_rich<'a>(&'a self, client: &'a Client, rich_message: InputRichMessage) -> SendRichMessage<'a> {
        self.answer_rich(client, rich_message).reply_to(self.id)
    }

    /// Sends a rich message to this chat, in the same topic and through the same business connection.
    pub fn answer_rich<'a>(&'a self, client: &'a Client, rich_message: InputRichMessage) -> SendRichMessage<'a> {
        self.route(client.send_rich_message(self.chat.id, rich_message))
    }

    /// Sends a sticker as a reply to this message.
    pub fn reply_sticker<'a>(&'a self, client: &'a Client, sticker: impl Into<StickerSource<'a>>) -> SendSticker<'a> {
        self.answer_sticker(client, sticker).reply_to(self.id)
    }

    /// Sends a sticker to this chat, in the same topic and through the same business connection.
    pub fn answer_sticker<'a>(&'a self, client: &'a Client, sticker: impl Into<StickerSource<'a>>) -> SendSticker<'a> {
        self.route(client.send_sticker(self.chat.id, sticker))
    }

    /// Sends a venue as a reply to this message.
    pub fn reply_venue<'a>(
        &'a self,
        client: &'a Client,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'a> {
        self.answer_venue(client, latitude, longitude, title, address).reply_to(self.id)
    }

    /// Sends a venue to this chat, in the same topic and through the same business connection.
    pub fn answer_venue<'a>(
        &'a self,
        client: &'a Client,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'a> {
        self.route(client.send_venue(self.chat.id, latitude, longitude, title, address))
    }

    /// Sends a video as a reply to this message.
    pub fn reply_video<'a>(&'a self, client: &'a Client, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        self.answer_video(client, video).reply_to(self.id)
    }

    /// Sends a video to this chat, in the same topic and through the same business connection.
    pub fn answer_video<'a>(&'a self, client: &'a Client, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        self.route(client.send_video(self.chat.id, video))
    }

    /// Sends a video note as a reply to this message.
    pub fn reply_video_note<'a>(
        &'a self,
        client: &'a Client,
        video_note: impl Into<FileSource<'a>>,
    ) -> SendVideoNote<'a> {
        self.answer_video_note(client, video_note).reply_to(self.id)
    }

    /// Sends a video note to this chat, in the same topic and through the same business connection.
    pub fn answer_video_note<'a>(
        &'a self,
        client: &'a Client,
        video_note: impl Into<FileSource<'a>>,
    ) -> SendVideoNote<'a> {
        self.route(client.send_video_note(self.chat.id, video_note))
    }

    /// Sends a voice message as a reply to this message.
    pub fn reply_voice<'a>(&'a self, client: &'a Client, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        self.answer_voice(client, voice).reply_to(self.id)
    }

    /// Sends a voice message to this chat, in the same topic and through the same business connection.
    pub fn answer_voice<'a>(&'a self, client: &'a Client, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        self.route(client.send_voice(self.chat.id, voice))
    }

    /// Sends a widget as a reply to this message.
    pub fn reply_widget<'a>(&'a self, client: &'a Client, html: impl Into<String>) -> SendWidget<'a> {
        self.answer_widget(client, html).reply_to(self.id)
    }

    /// Sends a widget to this chat in the same topic and through the same business connection.
    pub fn answer_widget<'a>(&'a self, client: &'a Client, html: impl Into<String>) -> SendWidget<'a> {
        self.route(client.send_widget(self.chat.id, html))
    }

    /// Sends a shared widget as a reply to this message.
    pub fn reply_shared_widget<'a>(&'a self, client: &'a Client, widget_id: i64) -> SendSharedWidget<'a> {
        self.answer_shared_widget(client, widget_id).reply_to(self.id)
    }

    /// Sends a shared widget to this chat in the same topic and through the same business connection.
    pub fn answer_shared_widget<'a>(&'a self, client: &'a Client, widget_id: i64) -> SendSharedWidget<'a> {
        self.route(client.send_shared_widget(self.chat.id, widget_id))
    }

    /// Shows `action` in this chat, through the same business connection.
    pub fn send_chat_action<'a>(&'a self, client: &'a Client, action: ChatAction) -> SendChatAction<'a> {
        let mut send = client.send_chat_action(self.chat.id, action);
        send.business_connection().clone_from(&self.business_connection_id);
        send
    }

    /// Sends an inline bot result as a reply to this message.
    ///
    /// On a message that came through a business connection awaiting fails with `Error::InvalidArgument`, as for
    /// [`answer_inline_bot_result`](Message::answer_inline_bot_result).
    pub fn reply_inline_bot_result<'a>(
        &'a self,
        client: &'a Client,
        query_id: i64,
        result_id: impl Into<String>,
    ) -> SendInlineBotResult<'a> {
        self.answer_inline_bot_result(client, query_id, result_id).reply_to(self.id)
    }

    /// Sends an inline bot result to this chat, in the same forum topic and direct-messages topic.
    ///
    /// messages.sendInlineBotResult can't go through a business connection, so on a message that came through one
    /// awaiting fails with `Error::InvalidArgument`.
    pub fn answer_inline_bot_result<'a>(
        &'a self,
        client: &'a Client,
        query_id: i64,
        result_id: impl Into<String>,
    ) -> SendInlineBotResult<'a> {
        let send = self.route_topic(client.send_inline_bot_result(self.chat.id, query_id, result_id));
        match self.business_connection_id {
            Some(_) => send.refuse("an inline bot result can't be sent through a business connection"),
            None => send,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::{ChatAction, FileType};
    use crate::file_id::{FileId, ThumbnailSource};
    use crate::raw::{Identifiable, base, functions, types};
    use crate::types::{
        DirectMessagesTopic, FileSource, InputChecklist, InputInvoice, InputMedia, InputRichMessage, LabeledPrice,
        Message, PeerRef,
    };
    use crate::utils::get_peer;

    const BUSINESS: u32 = 0xdd289f8e;

    fn seeded() -> (Client, UnboundedReceiver<Vec<u8>>, Message) {
        let (client, rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        client.0.caches.know_business("c", 7, 2);
        let message = Message::parse(&client, Box::new(Message::bare(5, get_peer(7), 0)), &Entities::default());
        (client, rx, message)
    }

    fn document<'a>() -> FileSource<'a> {
        FileSource::file_id(&FileId::new(FileType::Document, 1, 42, 7, &[]).encode()).unwrap()
    }

    fn video<'a>() -> FileSource<'a> {
        FileSource::file_id(&FileId::new(FileType::Video, 1, 43, 7, &[]).encode()).unwrap()
    }

    /// A stored photo: a URL can't carry a live photo's video, so the live photo pair needs one.
    fn photo<'a>() -> FileSource<'a> {
        let source = ThumbnailSource::Thumbnail { file_type: FileType::Photo, size: 'm' };
        let id = FileId { thumbnail_source: Some(source), ..FileId::new(FileType::Photo, 1, 44, 7, &[]) };
        FileSource::file_id(&id.encode()).unwrap()
    }

    /// The constructor id and the reply_to bit (flags.0, right after the id) of the first request `call` sends.
    async fn sent<F: IntoFuture<Output: std::fmt::Debug>>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> (u32, bool) {
        let result = call.await;
        let Ok(body) = rx.try_recv() else { panic!("no request sent: {result:?}") };
        assert!(rx.try_recv().is_err(), "more than one request sent");
        (u32::from_le_bytes(body[..4].try_into().unwrap()), body[4] & 1 == 1)
    }

    #[tokio::test]
    async fn every_pair_sends_its_constructor_and_only_reply_sets_reply_to() {
        let (client, mut rx, m) = seeded();
        let rx = &mut rx;
        let message = functions::messages::SendMessage::CONSTRUCTOR_ID;
        let media = functions::messages::SendMedia::CONSTRUCTOR_ID;
        let invoice = || InputInvoice::new("t", "d", "p", "XTR", [LabeledPrice::new("l", 1)]);
        let checklist = || InputChecklist::new("t").task(1, "a");
        let file_id = FileId::new(FileType::Document, 1, 42, 7, &[]).encode();
        let cases = [
            (
                "animation",
                sent(rx, m.answer_animation(&client, document())).await,
                sent(rx, m.reply_animation(&client, document())).await,
                media,
            ),
            (
                "audio",
                sent(rx, m.answer_audio(&client, document())).await,
                sent(rx, m.reply_audio(&client, document())).await,
                media,
            ),
            (
                "cached_media",
                sent(rx, m.answer_cached_media(&client, &*file_id)).await,
                sent(rx, m.reply_cached_media(&client, &*file_id)).await,
                media,
            ),
            (
                "checklist",
                sent(rx, m.answer_checklist(&client, checklist())).await,
                sent(rx, m.reply_checklist(&client, checklist())).await,
                media,
            ),
            (
                "contact",
                sent(rx, m.answer_contact(&client, "+1", "a")).await,
                sent(rx, m.reply_contact(&client, "+1", "a")).await,
                media,
            ),
            ("dice", sent(rx, m.answer_dice(&client)).await, sent(rx, m.reply_dice(&client)).await, media),
            (
                "document",
                sent(rx, m.answer_document(&client, document())).await,
                sent(rx, m.reply_document(&client, document())).await,
                media,
            ),
            ("game", sent(rx, m.answer_game(&client, "g")).await, sent(rx, m.reply_game(&client, "g")).await, media),
            (
                "invoice",
                sent(rx, m.answer_invoice(&client, invoice())).await,
                sent(rx, m.reply_invoice(&client, invoice())).await,
                media,
            ),
            (
                "live_photo",
                sent(rx, m.answer_live_photo(&client, photo(), video())).await,
                sent(rx, m.reply_live_photo(&client, photo(), video())).await,
                media,
            ),
            (
                "location",
                sent(rx, m.answer_location(&client, 1.0, 2.0)).await,
                sent(rx, m.reply_location(&client, 1.0, 2.0)).await,
                media,
            ),
            (
                "paid_media",
                sent(rx, m.answer_paid_media(&client, 1, [InputMedia::video(video())])).await,
                sent(rx, m.reply_paid_media(&client, 1, [InputMedia::video(video())])).await,
                media,
            ),
            (
                "photo",
                sent(rx, m.answer_photo(&client, photo())).await,
                sent(rx, m.reply_photo(&client, photo())).await,
                media,
            ),
            (
                "poll",
                sent(rx, m.answer_poll(&client, "q", ["a", "b"])).await,
                sent(rx, m.reply_poll(&client, "q", ["a", "b"])).await,
                media,
            ),
            (
                "rich",
                sent(rx, m.answer_rich(&client, InputRichMessage::html("hi"))).await,
                sent(rx, m.reply_rich(&client, InputRichMessage::html("hi"))).await,
                message,
            ),
            (
                "sticker",
                sent(rx, m.answer_sticker(&client, document())).await,
                sent(rx, m.reply_sticker(&client, document())).await,
                media,
            ),
            (
                "venue",
                sent(rx, m.answer_venue(&client, 1.0, 2.0, "t", "a")).await,
                sent(rx, m.reply_venue(&client, 1.0, 2.0, "t", "a")).await,
                media,
            ),
            (
                "video",
                sent(rx, m.answer_video(&client, video())).await,
                sent(rx, m.reply_video(&client, video())).await,
                media,
            ),
            (
                "video_note",
                sent(rx, m.answer_video_note(&client, video())).await,
                sent(rx, m.reply_video_note(&client, video())).await,
                media,
            ),
            (
                "voice",
                sent(rx, m.answer_voice(&client, document())).await,
                sent(rx, m.reply_voice(&client, document())).await,
                media,
            ),
            (
                "widget",
                sent(rx, m.answer_widget(&client, "html")).await,
                sent(rx, m.reply_widget(&client, "html")).await,
                media,
            ),
            (
                "shared_widget",
                sent(rx, m.answer_shared_widget(&client, 1)).await,
                sent(rx, m.reply_shared_widget(&client, 1)).await,
                media,
            ),
        ];
        for (kind, answer, reply, id) in cases {
            assert_eq!((answer, reply), ((id, false), (id, true)), "{kind}");
        }
    }

    #[tokio::test]
    async fn route_sets_thread_direct_topic_and_connection() {
        fn routed(m: &Message) -> (Option<i32>, Option<PeerRef>, Option<String>) {
            let client = Client::offline();
            let mut send = m.answer_photo(&client, photo());
            let options = crate::methods::Routable::options(&mut send);
            (
                options.message_thread_id,
                options.direct_messages_topic_id.clone(),
                options.business_connection_id.clone(),
            )
        }
        let (_client, _rx, mut m) = seeded();
        assert_eq!(routed(&m), (None, None, None));

        // A reply thread outside a forum topic isn't a topic.
        m.message_thread_id = Some(3);
        assert_eq!(routed(&m), (None, None, None));

        m.is_topic_message = true;
        m.business_connection_id = Some("c".into());
        m.direct_messages_topic = Some(Box::new(DirectMessagesTopic { topic_id: 7, user: None }));
        assert_eq!(routed(&m), (Some(3), Some(PeerRef::Id(7)), Some("c".into())));
    }

    #[tokio::test]
    async fn send_chat_action_goes_through_the_business_connection() {
        let (client, mut rx, mut m) = seeded();
        let (id, _) = sent(&mut rx, m.send_chat_action(&client, ChatAction::Typing)).await;
        assert_eq!(id, functions::messages::SetTyping::CONSTRUCTOR_ID);
        m.business_connection_id = Some("c".into());
        let (id, _) = sent(&mut rx, m.send_chat_action(&client, ChatAction::Typing)).await;
        assert_eq!(id, BUSINESS);
    }

    #[tokio::test]
    async fn inline_bot_results_keep_the_forum_topic() {
        let (client, mut rx, mut m) = seeded();
        let id = functions::messages::SendInlineBotResult::CONSTRUCTOR_ID;
        assert_eq!(sent(&mut rx, m.answer_inline_bot_result(&client, 1, "r")).await, (id, false));
        assert_eq!(sent(&mut rx, m.reply_inline_bot_result(&client, 1, "r")).await, (id, true));
        (m.message_thread_id, m.is_topic_message) = (Some(1), true);
        assert_eq!(sent(&mut rx, m.answer_inline_bot_result(&client, 1, "r")).await, (id, false));
        m.message_thread_id = Some(3);
        let topic_reply = |reply_to_msg_id, top_msg_id| {
            let reply = base::InputReplyTo::InputReplyToMessage(Box::new(types::InputReplyToMessage {
                reply_to_msg_id,
                top_msg_id,
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
                quote_rich: None,
            }));
            crate::raw::Serializable::to_bytes(&reply)
        };
        let _ = m.answer_inline_bot_result(&client, 1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let reply = topic_reply(3, None);
        assert!(body.windows(reply.len()).any(|w| w == reply), "the topic is not the reply target");
        // A reply inside the topic names the topic as well, like every other reply.
        let _ = m.reply_inline_bot_result(&client, 1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let reply = topic_reply(5, Some(3));
        assert!(body.windows(reply.len()).any(|w| w == reply), "the reply lost its topic");
        // Outside a forum topic, a direct-messages topic goes out as the monoforum reply target.
        (m.message_thread_id, m.is_topic_message) = (None, false);
        m.direct_messages_topic = Some(Box::new(DirectMessagesTopic { topic_id: 7, user: None }));
        let _ = m.answer_inline_bot_result(&client, 1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        let monoforum = types::InputReplyToMonoForum { monoforum_peer_id: user };
        let reply = crate::raw::Serializable::to_bytes(&base::InputReplyTo::InputReplyToMonoForum(Box::new(monoforum)));
        assert!(body.windows(reply.len()).any(|w| w == reply), "the direct-messages topic is not the reply target");
    }
}
