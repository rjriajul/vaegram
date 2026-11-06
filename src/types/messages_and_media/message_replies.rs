use crate::enums::ChatAction;
use crate::methods::{
    OnBehalf, Routable, SendAnimation, SendAudio, SendCachedMedia, SendChatAction, SendChecklist, SendContact,
    SendDice, SendDocument, SendGame, SendInlineBotResult, SendInvoice, SendLivePhoto, SendLocation, SendPaidMedia,
    SendPhoto, SendPoll, SendRichMessage, SendSticker, SendVenue, SendVideo, SendVideoNote, SendVoice,
};
use crate::types::{
    FileSource, InputChecklist, InputInvoice, InputMedia, InputPollOption, InputRichMessage, Message, Text,
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
    pub fn reply_animation<'a>(&'a self, animation: impl Into<FileSource<'a>>) -> SendAnimation<'a> {
        self.answer_animation(animation).reply_to(self.id)
    }

    /// Sends an animation to this chat, in the same topic and through the same business connection.
    pub fn answer_animation<'a>(&'a self, animation: impl Into<FileSource<'a>>) -> SendAnimation<'a> {
        self.route(self.client().send_animation(self.chat.id, animation))
    }

    /// Sends an audio file as a reply to this message.
    pub fn reply_audio<'a>(&'a self, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        self.answer_audio(audio).reply_to(self.id)
    }

    /// Sends an audio file to this chat, in the same topic and through the same business connection.
    pub fn answer_audio<'a>(&'a self, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        self.route(self.client().send_audio(self.chat.id, audio))
    }

    /// Sends a file by its file id as a reply to this message.
    pub fn reply_cached_media(&self, file_id: impl Into<String>) -> SendCachedMedia<'_> {
        self.answer_cached_media(file_id).reply_to(self.id)
    }

    /// Sends a file by its file id to this chat, in the same topic and through the same business connection.
    pub fn answer_cached_media(&self, file_id: impl Into<String>) -> SendCachedMedia<'_> {
        self.route(self.client().send_cached_media(self.chat.id, file_id))
    }

    /// Sends a checklist as a reply to this message.
    pub fn reply_checklist(&self, checklist: InputChecklist) -> SendChecklist<'_> {
        self.answer_checklist(checklist).reply_to(self.id)
    }

    /// Sends a checklist to this chat, in the same topic and through the same business connection.
    pub fn answer_checklist(&self, checklist: InputChecklist) -> SendChecklist<'_> {
        self.route(self.client().send_checklist(self.chat.id, checklist))
    }

    /// Sends a contact as a reply to this message.
    pub fn reply_contact(&self, phone_number: impl Into<String>, first_name: impl Into<String>) -> SendContact<'_> {
        self.answer_contact(phone_number, first_name).reply_to(self.id)
    }

    /// Sends a contact to this chat, in the same topic and through the same business connection.
    pub fn answer_contact(&self, phone_number: impl Into<String>, first_name: impl Into<String>) -> SendContact<'_> {
        self.route(self.client().send_contact(self.chat.id, phone_number, first_name))
    }

    /// Sends a die as a reply to this message.
    pub fn reply_dice(&self) -> SendDice<'_> {
        self.answer_dice().reply_to(self.id)
    }

    /// Sends a die to this chat, in the same topic and through the same business connection.
    pub fn answer_dice(&self) -> SendDice<'_> {
        self.route(self.client().send_dice(self.chat.id))
    }

    /// Sends a document as a reply to this message.
    pub fn reply_document<'a>(&'a self, document: impl Into<FileSource<'a>>) -> SendDocument<'a> {
        self.answer_document(document).reply_to(self.id)
    }

    /// Sends a document to this chat, in the same topic and through the same business connection.
    pub fn answer_document<'a>(&'a self, document: impl Into<FileSource<'a>>) -> SendDocument<'a> {
        self.route(self.client().send_document(self.chat.id, document))
    }

    /// Sends a game as a reply to this message.
    pub fn reply_game(&self, game_short_name: impl Into<String>) -> SendGame<'_> {
        self.answer_game(game_short_name).reply_to(self.id)
    }

    /// Sends a game to this chat, in the same topic and through the same business connection.
    pub fn answer_game(&self, game_short_name: impl Into<String>) -> SendGame<'_> {
        self.route(self.client().send_game(self.chat.id, game_short_name))
    }

    /// Sends an invoice as a reply to this message.
    pub fn reply_invoice(&self, invoice: InputInvoice) -> SendInvoice<'_> {
        self.answer_invoice(invoice).reply_to(self.id)
    }

    /// Sends an invoice to this chat, in the same topic and through the same business connection.
    pub fn answer_invoice(&self, invoice: InputInvoice) -> SendInvoice<'_> {
        self.route(self.client().send_invoice(self.chat.id, invoice))
    }

    /// Sends a live photo as a reply to this message.
    pub fn reply_live_photo<'a>(
        &'a self,
        photo: impl Into<FileSource<'a>>,
        live_photo: impl Into<FileSource<'a>>,
    ) -> SendLivePhoto<'a> {
        self.answer_live_photo(photo, live_photo).reply_to(self.id)
    }

    /// Sends a live photo to this chat, in the same topic and through the same business connection.
    pub fn answer_live_photo<'a>(
        &'a self,
        photo: impl Into<FileSource<'a>>,
        live_photo: impl Into<FileSource<'a>>,
    ) -> SendLivePhoto<'a> {
        self.route(self.client().send_live_photo(self.chat.id, photo, live_photo))
    }

    /// Sends a location as a reply to this message.
    pub fn reply_location(&self, latitude: f64, longitude: f64) -> SendLocation<'_> {
        self.answer_location(latitude, longitude).reply_to(self.id)
    }

    /// Sends a location to this chat, in the same topic and through the same business connection.
    pub fn answer_location(&self, latitude: f64, longitude: f64) -> SendLocation<'_> {
        self.route(self.client().send_location(self.chat.id, latitude, longitude))
    }

    /// Sends paid media as a reply to this message.
    pub fn reply_paid_media<'a>(
        &'a self,
        star_count: i64,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendPaidMedia<'a> {
        self.answer_paid_media(star_count, media).reply_to(self.id)
    }

    /// Sends paid media to this chat, in the same topic and through the same business connection.
    pub fn answer_paid_media<'a>(
        &'a self,
        star_count: i64,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendPaidMedia<'a> {
        self.route(self.client().send_paid_media(self.chat.id, star_count, media))
    }

    /// Sends a photo as a reply to this message.
    pub fn reply_photo<'a>(&'a self, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        self.answer_photo(photo).reply_to(self.id)
    }

    /// Sends a photo to this chat, in the same topic and through the same business connection.
    pub fn answer_photo<'a>(&'a self, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        self.route(self.client().send_photo(self.chat.id, photo))
    }

    /// Sends a poll as a reply to this message.
    pub fn reply_poll<'a>(
        &'a self,
        question: impl Into<Text>,
        options: impl IntoIterator<Item = impl Into<InputPollOption<'a>>>,
    ) -> SendPoll<'a> {
        self.answer_poll(question, options).reply_to(self.id)
    }

    /// Sends a poll to this chat, in the same topic and through the same business connection.
    pub fn answer_poll<'a>(
        &'a self,
        question: impl Into<Text>,
        options: impl IntoIterator<Item = impl Into<InputPollOption<'a>>>,
    ) -> SendPoll<'a> {
        self.route(self.client().send_poll(self.chat.id, question, options))
    }

    /// Sends a rich message as a reply to this message.
    pub fn reply_rich(&self, rich_message: InputRichMessage) -> SendRichMessage<'_> {
        self.answer_rich(rich_message).reply_to(self.id)
    }

    /// Sends a rich message to this chat, in the same topic and through the same business connection.
    pub fn answer_rich(&self, rich_message: InputRichMessage) -> SendRichMessage<'_> {
        self.route(self.client().send_rich_message(self.chat.id, rich_message))
    }

    /// Sends a sticker as a reply to this message.
    pub fn reply_sticker<'a>(&'a self, sticker: impl Into<FileSource<'a>>) -> SendSticker<'a> {
        self.answer_sticker(sticker).reply_to(self.id)
    }

    /// Sends a sticker to this chat, in the same topic and through the same business connection.
    pub fn answer_sticker<'a>(&'a self, sticker: impl Into<FileSource<'a>>) -> SendSticker<'a> {
        self.route(self.client().send_sticker(self.chat.id, sticker))
    }

    /// Sends a venue as a reply to this message.
    pub fn reply_venue(
        &self,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'_> {
        self.answer_venue(latitude, longitude, title, address).reply_to(self.id)
    }

    /// Sends a venue to this chat, in the same topic and through the same business connection.
    pub fn answer_venue(
        &self,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'_> {
        self.route(self.client().send_venue(self.chat.id, latitude, longitude, title, address))
    }

    /// Sends a video as a reply to this message.
    pub fn reply_video<'a>(&'a self, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        self.answer_video(video).reply_to(self.id)
    }

    /// Sends a video to this chat, in the same topic and through the same business connection.
    pub fn answer_video<'a>(&'a self, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        self.route(self.client().send_video(self.chat.id, video))
    }

    /// Sends a video note as a reply to this message.
    pub fn reply_video_note<'a>(&'a self, video_note: impl Into<FileSource<'a>>) -> SendVideoNote<'a> {
        self.answer_video_note(video_note).reply_to(self.id)
    }

    /// Sends a video note to this chat, in the same topic and through the same business connection.
    pub fn answer_video_note<'a>(&'a self, video_note: impl Into<FileSource<'a>>) -> SendVideoNote<'a> {
        self.route(self.client().send_video_note(self.chat.id, video_note))
    }

    /// Sends a voice message as a reply to this message.
    pub fn reply_voice<'a>(&'a self, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        self.answer_voice(voice).reply_to(self.id)
    }

    /// Sends a voice message to this chat, in the same topic and through the same business connection.
    pub fn answer_voice<'a>(&'a self, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        self.route(self.client().send_voice(self.chat.id, voice))
    }

    /// Shows `action` in this chat, through the same business connection.
    pub fn send_chat_action(&self, action: ChatAction) -> SendChatAction<'_> {
        let mut send = self.client().send_chat_action(self.chat.id, action);
        send.business_connection().clone_from(&self.business_connection_id);
        send
    }

    /// Sends an inline bot result as a reply to this message.
    pub fn reply_inline_bot_result(&self, query_id: i64, result_id: impl Into<String>) -> SendInlineBotResult<'_> {
        self.answer_inline_bot_result(query_id, result_id).reply_to(self.id)
    }

    /// Sends an inline bot result to this chat, in the same forum topic and direct-messages topic.
    /// messages.sendInlineBotResult takes no business connection, so it isn't kept.
    pub fn answer_inline_bot_result(&self, query_id: i64, result_id: impl Into<String>) -> SendInlineBotResult<'_> {
        self.route_topic(self.client().send_inline_bot_result(self.chat.id, query_id, result_id))
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::ChatAction;
    use crate::file_id::{FileId, FileType, ThumbnailSource};
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
        let (_client, mut rx, m) = seeded();
        let rx = &mut rx;
        let message = functions::messages::SendMessage::CONSTRUCTOR_ID;
        let media = functions::messages::SendMedia::CONSTRUCTOR_ID;
        let invoice = || InputInvoice::new("t", "d", "p", "XTR", [LabeledPrice::new("l", 1)]);
        let checklist = || InputChecklist::new("t").task(1, "a");
        let file_id = FileId::new(FileType::Document, 1, 42, 7, &[]).encode();
        let cases = [
            (
                "animation",
                sent(rx, m.answer_animation(document())).await,
                sent(rx, m.reply_animation(document())).await,
                media,
            ),
            ("audio", sent(rx, m.answer_audio(document())).await, sent(rx, m.reply_audio(document())).await, media),
            (
                "cached_media",
                sent(rx, m.answer_cached_media(&*file_id)).await,
                sent(rx, m.reply_cached_media(&*file_id)).await,
                media,
            ),
            (
                "checklist",
                sent(rx, m.answer_checklist(checklist())).await,
                sent(rx, m.reply_checklist(checklist())).await,
                media,
            ),
            ("contact", sent(rx, m.answer_contact("+1", "a")).await, sent(rx, m.reply_contact("+1", "a")).await, media),
            ("dice", sent(rx, m.answer_dice()).await, sent(rx, m.reply_dice()).await, media),
            (
                "document",
                sent(rx, m.answer_document(document())).await,
                sent(rx, m.reply_document(document())).await,
                media,
            ),
            ("game", sent(rx, m.answer_game("g")).await, sent(rx, m.reply_game("g")).await, media),
            ("invoice", sent(rx, m.answer_invoice(invoice())).await, sent(rx, m.reply_invoice(invoice())).await, media),
            (
                "live_photo",
                sent(rx, m.answer_live_photo(photo(), video())).await,
                sent(rx, m.reply_live_photo(photo(), video())).await,
                media,
            ),
            (
                "location",
                sent(rx, m.answer_location(1.0, 2.0)).await,
                sent(rx, m.reply_location(1.0, 2.0)).await,
                media,
            ),
            (
                "paid_media",
                sent(rx, m.answer_paid_media(1, [InputMedia::video(video())])).await,
                sent(rx, m.reply_paid_media(1, [InputMedia::video(video())])).await,
                media,
            ),
            ("photo", sent(rx, m.answer_photo(photo())).await, sent(rx, m.reply_photo(photo())).await, media),
            (
                "poll",
                sent(rx, m.answer_poll("q", ["a", "b"])).await,
                sent(rx, m.reply_poll("q", ["a", "b"])).await,
                media,
            ),
            (
                "rich",
                sent(rx, m.answer_rich(InputRichMessage::html("hi"))).await,
                sent(rx, m.reply_rich(InputRichMessage::html("hi"))).await,
                message,
            ),
            (
                "sticker",
                sent(rx, m.answer_sticker(document())).await,
                sent(rx, m.reply_sticker(document())).await,
                media,
            ),
            (
                "venue",
                sent(rx, m.answer_venue(1.0, 2.0, "t", "a")).await,
                sent(rx, m.reply_venue(1.0, 2.0, "t", "a")).await,
                media,
            ),
            ("video", sent(rx, m.answer_video(video())).await, sent(rx, m.reply_video(video())).await, media),
            (
                "video_note",
                sent(rx, m.answer_video_note(video())).await,
                sent(rx, m.reply_video_note(video())).await,
                media,
            ),
            ("voice", sent(rx, m.answer_voice(document())).await, sent(rx, m.reply_voice(document())).await, media),
        ];
        for (kind, answer, reply, id) in cases {
            assert_eq!((answer, reply), ((id, false), (id, true)), "{kind}");
        }
    }

    #[tokio::test]
    async fn route_sets_thread_direct_topic_and_connection() {
        fn routed(m: &Message) -> (Option<i32>, Option<PeerRef>, Option<String>) {
            let mut send = m.answer_photo(photo());
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
        m.direct_messages_topic = Some(DirectMessagesTopic { topic_id: 7, user: None });
        assert_eq!(routed(&m), (Some(3), Some(PeerRef::Id(7)), Some("c".into())));
    }

    #[tokio::test]
    async fn send_chat_action_goes_through_the_business_connection() {
        let (_client, mut rx, mut m) = seeded();
        let (id, _) = sent(&mut rx, m.send_chat_action(ChatAction::Typing)).await;
        assert_eq!(id, functions::messages::SetTyping::CONSTRUCTOR_ID);
        m.business_connection_id = Some("c".into());
        let (id, _) = sent(&mut rx, m.send_chat_action(ChatAction::Typing)).await;
        assert_eq!(id, BUSINESS);
    }

    #[tokio::test]
    async fn inline_bot_results_keep_the_forum_topic() {
        let (_client, mut rx, mut m) = seeded();
        let id = functions::messages::SendInlineBotResult::CONSTRUCTOR_ID;
        assert_eq!(sent(&mut rx, m.answer_inline_bot_result(1, "r")).await, (id, false));
        assert_eq!(sent(&mut rx, m.reply_inline_bot_result(1, "r")).await, (id, true));
        (m.message_thread_id, m.is_topic_message) = (Some(1), true);
        assert_eq!(sent(&mut rx, m.answer_inline_bot_result(1, "r")).await, (id, false));
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
        let _ = m.answer_inline_bot_result(1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let reply = topic_reply(3, None);
        assert!(body.windows(reply.len()).any(|w| w == reply), "the topic is not the reply target");
        // A reply inside the topic names the topic as well, like every other reply.
        let _ = m.reply_inline_bot_result(1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let reply = topic_reply(5, Some(3));
        assert!(body.windows(reply.len()).any(|w| w == reply), "the reply lost its topic");
        // Outside a forum topic, a direct-messages topic goes out as the monoforum reply target.
        (m.message_thread_id, m.is_topic_message) = (None, false);
        m.direct_messages_topic = Some(DirectMessagesTopic { topic_id: 7, user: None });
        let _ = m.answer_inline_bot_result(1, "r").await;
        let body = rx.try_recv().expect("no request sent");
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        let monoforum = types::InputReplyToMonoForum { monoforum_peer_id: user };
        let reply = crate::raw::Serializable::to_bytes(&base::InputReplyTo::InputReplyToMonoForum(Box::new(monoforum)));
        assert!(body.windows(reply.len()).any(|w| w == reply), "the direct-messages topic is not the reply target");
    }
}
