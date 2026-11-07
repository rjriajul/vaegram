use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::methods::{
    CopyMediaGroup, EditEphemeralMessage, EditMessageChecklist, EditMessageLiveLocation, EditMessageMedia, Listen,
    SendEphemeralMessage, StopMessageLiveLocation, SummarizeMessage,
};
use crate::types::{
    CallbackQuery, EphemeralMessage, InlineKeyboardMarkup, InputChecklist, InputMedia, InvoiceRef, Message, PeerRef,
    Text,
};
use crate::utils::channel_id_of;

impl Message {
    /// Replaces the media of this message. Goes through the business connection if the message arrived on one.
    pub fn edit_media<'a>(&'a self, media: InputMedia<'a>) -> EditMessageMedia<'a> {
        self.on_behalf(self.client().edit_message_media(self.chat.id, self.id, media))
    }

    /// Moves the live location in this message. Goes through the business connection if the message arrived on one.
    pub fn edit_live_location(&self, latitude: f64, longitude: f64) -> EditMessageLiveLocation<'_> {
        self.on_behalf(self.client().edit_message_live_location(self.chat.id, self.id, latitude, longitude))
    }

    /// Ends the live location in this message. Goes through the business connection if the message arrived on one.
    pub fn stop_live_location(&self) -> StopMessageLiveLocation<'_> {
        self.on_behalf(self.client().stop_message_live_location(self.chat.id, self.id))
    }

    /// Replaces the whole checklist; `InputChecklist::from(checklist)` keeps the formatting of an existing one.
    pub fn edit_checklist(&self, checklist: InputChecklist) -> EditMessageChecklist<'_> {
        self.on_behalf(self.client().edit_message_checklist(self.chat.id, self.id, checklist))
    }

    /// Votes for the options at the given positions (several only in multiple-choice polls).
    pub async fn vote(&self, options: &[usize]) -> Result<()> {
        self.client().vote_poll(self.chat.id, self.id, options).await
    }

    /// Withdraws the account's vote from the poll in this message.
    pub async fn retract_vote(&self) -> Result<()> {
        self.client().retract_vote(self.chat.id, self.id).await
    }

    /// Counts one view of this message, as opening it in an app does.
    pub async fn view(&self) -> Result<()> {
        self.client().view_messages(self.chat.id, &[self.id]).await
    }

    /// Pays the Stars invoice in this message; nothing is paid if its price is above `star_count`. A timed-out payment
    /// may have gone through: check the balance before retrying.
    pub async fn pay(&self, star_count: i64) -> Result<()> {
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let invoice = InvoiceRef::Message { chat_id: PeerRef::from(self.chat.id), message_id: self.id };
        let invoice = invoice.write(self.client()).await?;
        self.client().pay_in_stars_up_to(invoice, star_count).await
    }

    /// Accepts the gift purchase offer in this message, which sells the gift.
    pub async fn accept_gift_purchase_offer(&self) -> Result<()> {
        self.client().process_gift_purchase_offer(self.id, true).await
    }

    /// Rejects the gift purchase offer in this message.
    pub async fn reject_gift_purchase_offer(&self) -> Result<()> {
        self.client().process_gift_purchase_offer(self.id, false).await
    }

    /// Asks Telegram for a summary of this message's text. Awaiting it returns the summary as `FormattedText`.
    pub fn summarize(&self) -> SummarizeMessage<'_> {
        self.client().summarize_message(self.chat.id, self.id)
    }

    /// Waits for the next press on this message's buttons, with the rules of [`Client::wait_for_callback_query`].
    ///
    /// [`Client::wait_for_callback_query`]: crate::Client::wait_for_callback_query
    pub fn wait_for_click(&self) -> Listen<'_, CallbackQuery> {
        self.client().wait_for_callback_query(self.chat.id).message_id(self.id)
    }

    /// Copies the album this message belongs to into `chat_id`.
    pub fn copy_media_group(&self, chat_id: impl Into<PeerRef>) -> CopyMediaGroup<'_> {
        self.client().copy_media_group(chat_id, self.chat.id, self.id)
    }

    /// Every message of the album this message belongs to.
    pub async fn get_media_group(&self) -> Result<Vec<Message>> {
        self.client().get_media_group(self.chat.id, self.id).await
    }

    /// Replies with an ephemeral message that only the sender of this message sees.
    ///
    /// Awaiting fails with `Error::InvalidArgument`, sending nothing, if the message has no sender user.
    pub fn reply_ephemeral(&self, text: impl Into<Text>) -> SendEphemeralMessage<'_> {
        let receiver = self.from_user.as_ref().map(|u| PeerRef::from(u.id));
        SendEphemeralMessage::new(self.client(), self.chat.id.into(), receiver, text.into()).reply_to(self.id)
    }

    /// The `t.me` link to this message. Only supergroups and channels have message links.
    pub fn link(&self) -> Option<String> {
        if !matches!(self.chat.kind, ChatType::Supergroup | ChatType::Channel) {
            return None;
        }
        let chat = match &self.chat.username {
            Some(username) => username.clone(),
            None => format!("c/{}", channel_id_of(self.chat.id)),
        };
        let thread = match self.message_thread_id {
            Some(thread) if self.is_topic_message => format!("{thread}/"),
            _ => String::new(),
        };
        Some(format!("https://t.me/{chat}/{thread}{}", self.id))
    }

    /// The caption, or the text when there's no caption.
    pub fn content(&self) -> &str {
        // Telegram keeps both in the same field; `text` and `caption` only split it by the media.
        &self.raw.message
    }
}

impl EphemeralMessage {
    /// Replaces the caption of this ephemeral message.
    pub fn edit_caption(&self, caption: impl Into<Text>) -> EditEphemeralMessage<'_> {
        self.client().edit_ephemeral_message_caption(self.chat.id, self.receiver_user.id, self.id, caption)
    }

    /// Replaces the media of this ephemeral message.
    pub fn edit_media<'a>(&'a self, media: InputMedia<'a>) -> EditEphemeralMessage<'a> {
        self.client().edit_ephemeral_message_media(self.chat.id, self.receiver_user.id, self.id, media)
    }

    /// Replaces the inline keyboard of this ephemeral message. `None` removes the keyboard.
    pub fn edit_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> EditEphemeralMessage<'_> {
        let edit = self.client().edit_ephemeral_message_reply_markup(self.chat.id, self.receiver_user.id, self.id);
        match reply_markup {
            Some(markup) => edit.reply_markup(markup),
            None => edit,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::dispatcher::listeners::Offer;
    use crate::dispatcher::listeners::tests::query;
    use crate::enums::ChatType;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::messages_and_media::ephemeral_message::tests::ephemeral;
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{EphemeralMessage, FileSource, InputChecklist, InputMedia, Message, Text};
    use crate::utils::{get_channel_id, get_peer};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
    const BUSINESS: [u8; 4] = 0xdd289f8e_u32.to_le_bytes();

    type Sent = tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>;

    fn seeded(client: &Client) {
        let mut peers = client.0.peers.lock().unwrap();
        for user_id in [7, 1000] {
            let user = types::InputPeerUser { user_id, access_hash: 1 };
            peers.insert(user_id, base::InputPeer::InputPeerUser(Box::new(user)));
        }
    }

    fn recording() -> (Client, Sent) {
        let (client, sent) = Client::recording();
        seeded(&client);
        client.0.caches.know_business("c", 7, 2);
        (client, sent)
    }

    fn message(client: &Client) -> Message {
        Message::parse(client, Box::new(Message::bare(5, get_peer(7), 0)), &Entities::default())
    }

    fn photo() -> InputMedia<'static> {
        InputMedia::photo(FileSource::file_id(PHOTO).unwrap())
    }

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }))
    }

    #[tokio::test]
    async fn business_edits_go_through_the_connection() {
        let (client, mut sent) = recording();
        let mut message = message(&client);
        message.business_connection_id = Some("c".into());
        let results = [
            message.edit_media(photo()).await.map(drop),
            message.edit_live_location(1.0, 2.0).await.map(drop),
            message.stop_live_location().await.map(drop),
            message.edit_checklist(InputChecklist::new("todo").task(1, "a")).await.map(drop),
        ];
        for result in results {
            assert!(result.is_err());
            assert_eq!(sent.try_recv().expect("no request sent")[..4], BUSINESS);
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn plain_edits_skip_the_connection() {
        let (client, mut sent) = recording();
        let message = message(&client);
        assert!(message.stop_live_location().await.is_err());
        assert_ne!(sent.try_recv().expect("no request sent")[..4], BUSINESS);
    }

    #[tokio::test]
    async fn votes_send_what_the_client_methods_send() {
        let (client, mut sent) = recording();
        let message = message(&client);
        assert!(matches!(message.vote(&[]).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err(), "an empty vote must not send");
        assert!(message.vote(&[0]).await.is_err());
        let (direct, mut direct_sent) = recording();
        assert!(direct.vote_poll(7, 5, &[0]).await.is_err());
        assert_eq!(sent.try_recv().unwrap(), direct_sent.try_recv().unwrap());

        assert!(message.retract_vote().await.is_err());
        let retract = functions::messages::SendVote { peer: peer(), msg_id: 5, options: Vec::new() };
        assert_eq!(sent.try_recv().unwrap(), retract.to_bytes());
    }

    #[tokio::test]
    async fn gift_offers_resolve_by_message_id() {
        let (client, mut sent) = recording();
        let message = message(&client);
        assert!(message.accept_gift_purchase_offer().await.is_err());
        assert!(message.reject_gift_purchase_offer().await.is_err());
        for decline in [false, true] {
            let request = functions::payments::ResolveStarGiftOffer { decline, offer_msg_id: 5 };
            assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        }
    }

    fn message_invoice() -> base::InputInvoice {
        base::InputInvoice::InputInvoiceMessage(Box::new(types::InputInvoiceMessage { peer: peer(), msg_id: 5 }))
    }

    /// A star gift form with id 7 priced at `price` Stars.
    fn stars_form(price: i64) -> Vec<u8> {
        let price =
            base::LabeledPrice::LabeledPrice(Box::new(types::LabeledPrice { label: "Item".into(), amount: price }));
        let invoice = base::Invoice::Invoice(Box::new(types::Invoice {
            test: true,
            name_requested: false,
            phone_requested: false,
            email_requested: false,
            shipping_address_requested: false,
            flexible: false,
            phone_to_provider: false,
            email_to_provider: false,
            recurring: false,
            currency: "XTR".into(),
            prices: vec![price],
            max_tip_amount: None,
            suggested_tip_amounts: None,
            terms_url: None,
            subscription_period: None,
        }));
        let form = types::payments::PaymentFormStarGift { form_id: 7, invoice };
        base::payments::PaymentForm::PaymentFormStarGift(Box::new(form)).to_bytes()
    }

    #[tokio::test]
    async fn pay_reads_the_form_of_this_message_and_pays_up_to_the_cap() {
        let paid = types::payments::PaymentResult { updates: base::Updates::UpdatesTooLong };
        let paid = base::payments::PaymentResult::PaymentResult(Box::new(paid)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(stars_form(900)), Ok(paid)]);
        seeded(&client);
        message(&client).pay(900).await.unwrap();
        let form = functions::payments::GetPaymentForm { invoice: message_invoice(), theme_params: None };
        assert_eq!(sent.try_recv().unwrap(), form.to_bytes());
        let pay = functions::payments::SendStarsForm { form_id: 7, invoice: message_invoice() };
        assert_eq!(sent.try_recv().unwrap(), pay.to_bytes());
    }

    #[tokio::test]
    async fn pay_refuses_a_price_above_the_cap() {
        let (client, mut sent) = Client::scripted(vec![Ok(stars_form(1000))]);
        seeded(&client);
        assert!(matches!(message(&client).pay(900).await, Err(Error::InvalidArgument(_))));
        let form = functions::payments::GetPaymentForm { invoice: message_invoice(), theme_params: None };
        assert_eq!(sent.try_recv().unwrap(), form.to_bytes());
        assert!(sent.try_recv().is_err(), "nothing may be paid above the cap");
    }

    #[tokio::test]
    async fn pay_refuses_a_non_positive_cap_without_a_request() {
        let (client, mut sent) = recording();
        let message = message(&client);
        for cap in [0, -1] {
            assert!(matches!(message.pay(cap).await, Err(Error::InvalidArgument(_))), "{cap}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn view_counts_a_view() {
        let (client, mut sent) = recording();
        assert!(message(&client).view().await.is_err());
        let request = functions::messages::GetMessagesViews { peer: peer(), id: vec![5], increment: true };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[test]
    fn links_exist_only_for_supergroups_and_channels() {
        let client = Client::offline();
        let mut message =
            Message::parse(&client, Box::new(Message::bare(9, get_peer(get_channel_id(77)), 0)), &Entities::default());
        let mut link = |kind, username: Option<&str>, thread: Option<i32>, topic: bool| {
            let chat = std::sync::Arc::make_mut(&mut message.chat);
            (chat.kind, chat.username) = (kind, username.map(Into::into));
            (message.message_thread_id, message.is_topic_message) = (thread, topic);
            message.link()
        };
        assert_eq!(link(ChatType::Private, Some("user"), None, false), None);
        assert_eq!(link(ChatType::Bot, Some("bot"), None, false), None);
        assert_eq!(link(ChatType::Group, None, None, false), None);
        assert_eq!(link(ChatType::Supergroup, Some("user"), None, false).as_deref(), Some("https://t.me/user/9"));
        assert_eq!(link(ChatType::Channel, None, None, false).as_deref(), Some("https://t.me/c/77/9"));
        assert_eq!(link(ChatType::Supergroup, None, Some(3), true).as_deref(), Some("https://t.me/c/77/3/9"));
        assert_eq!(link(ChatType::Supergroup, Some("user"), Some(3), false).as_deref(), Some("https://t.me/user/9"));
    }

    #[test]
    fn content_is_the_caption_or_the_text() {
        let client = Client::offline();
        let mut raw = Message::bare(5, get_peer(7), 0);
        raw.message = "hello".into();
        let parse = |raw: &types::Message| Message::parse(&client, Box::new(raw.clone()), &Entities::default());
        assert_eq!(parse(&raw).content(), "hello");
        raw.media = Some(document_media("image/png", Vec::new()));
        let media = parse(&raw);
        assert_eq!((media.text(), media.caption(), media.content()), ("", Some("hello"), "hello"));
    }

    #[tokio::test]
    async fn wait_for_click_takes_only_this_messages_buttons() {
        let client = Client::offline();
        let message = message(&client);
        let mut click = message.wait_for_click().into_future();
        tokio::select! {
            biased;
            _ = &mut click => panic!("nothing was offered"),
            _ = std::future::ready(()) => {}
        }
        let (book, me) = (&client.0.handlers.listeners.callbacks, client.0.me.lock().unwrap().clone());
        assert!(matches!(book.offer(query(&client, 7, 7, 6), &me), Offer::Unclaimed(_)));
        assert!(matches!(book.offer(query(&client, 7, 8, 5), &me), Offer::Unclaimed(_)));
        assert!(matches!(book.offer(query(&client, 7, 7, 5), &me), Offer::Claimed));
        assert_eq!(click.await.unwrap().message_id(), Some(5));
    }

    #[tokio::test]
    async fn ephemeral_replies_need_a_sender() {
        let (client, mut sent) = recording();
        let mut message = message(&client);
        let mut from_channel = message.clone();
        from_channel.from_user = None;
        let refused = from_channel.reply_ephemeral("hi").await;
        assert!(matches!(refused, Err(Error::InvalidArgument("this message has no sender user"))), "{refused:?}");
        assert!(sent.try_recv().is_err());

        std::sync::Arc::make_mut(&mut message.chat).id = 1000;
        assert!(message.reply_ephemeral("hi").await.is_err());
        let request = sent.try_recv().expect("no request sent");
        assert_eq!(request[..4], 0xba8d5f35_u32.to_le_bytes());
        // reply_to is flags.5 of ephemeral.sendMessage, right after the constructor id.
        assert_eq!(request[4] & 0x20, 0x20);
    }

    #[tokio::test]
    async fn a_built_caption_keeps_its_entities() {
        let (client, mut sent) = recording();
        let _ = message(&client).edit_caption(Text::new().bold("new")).await;
        let body = sent.try_recv().expect("no request sent");
        let bold = vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 3 })];
        let has = |bytes: &[u8]| body.windows(bytes.len()).any(|w| w == bytes);
        assert!(has(&String::from("new").to_bytes()) && has(&bold.to_bytes()));
    }

    #[tokio::test]
    async fn a_client_default_leaves_caption_edits_alone() {
        let (client, mut sent) = recording();
        client.set_link_preview_options(Some(crate::types::LinkPreviewOptions::new().show_above_text(true)));
        let _ = message(&client).edit_caption("cap").await;
        let _ = message(&client).edit_text("text").await;
        // invert_media is flags.16 of messages.editMessage.
        let flags = |body: Vec<u8>| u32::from_le_bytes(body[4..8].try_into().unwrap()) >> 16 & 1;
        assert_eq!((flags(sent.try_recv().unwrap()), flags(sent.try_recv().unwrap())), (0, 1));
    }

    #[tokio::test]
    async fn ephemeral_edits_hit_edit_message() {
        let (client, mut sent) = recording();
        let message = EphemeralMessage::parse(&client, ephemeral(true, None), &Entities::default());
        let results = [
            message.edit_caption("new").await.map(drop),
            message.edit_media(photo()).await.map(drop),
            message.edit_reply_markup(None).await.map(drop),
        ];
        for result in results {
            assert!(result.is_err());
            assert_eq!(sent.try_recv().expect("no request sent")[..4], 0xcf9c725b_u32.to_le_bytes());
        }
    }
}
