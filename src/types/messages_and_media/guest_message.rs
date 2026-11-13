use crate::Client;
use crate::dispatcher::Entities;
use crate::methods::AnswerGuestQuery;
use crate::raw::{base, types};
use crate::types::{
    Chat, InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputRichMessage, Message, Text, User,
};

/// A message in a chat the bot isn't in. [`answer`](GuestMessage::answer) and its shortcuts
/// ([`answer_text`](GuestMessage::answer_text), [`answer_rich`](GuestMessage::answer_rich)) are the only way to
/// respond there.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GuestMessage {
    /// The message that invoked the bot.
    pub message: Message,
    /// Messages the guest query refers to. Service and empty messages are left out.
    pub reference_messages: Vec<Message>,
    /// Id of the guest query, which [`answer`](GuestMessage::answer) replies to.
    pub query_id: i64,
    /// The user the message came via. `None` if it came via a chat or Telegram names no caller.
    pub caller_user: Option<User>,
    /// The chat the message came via. `None` if it came via a user or Telegram names no caller.
    pub caller_chat: Option<Chat>,
}

impl GuestMessage {
    pub(crate) fn parse(client: &Client, raw: types::UpdateBotGuestChatQuery, entities: &Entities) -> Option<Self> {
        let base::Message::Message(m) = raw.message else { return None };
        let reference_messages = raw
            .reference_messages
            .unwrap_or_default()
            .into_iter()
            .filter_map(|reference| match reference {
                base::Message::Message(m) => Some(Message::parse(client, m, entities)),
                base::Message::MessageService(_) | base::Message::MessageEmpty(_) => None,
            })
            .collect();
        let (mut caller_user, mut caller_chat) = (None, None);
        match &m.guestchat_via_from {
            Some(base::Peer::PeerUser(p)) => caller_user = Some(User::parse(p.user_id, entities)),
            Some(peer) => caller_chat = Some(Chat::parse(peer, entities, false)),
            None => {}
        }
        let message = Message::parse(client, m, entities);
        Some(GuestMessage { message, reference_messages, query_id: raw.query_id, caller_user, caller_chat })
    }

    pub(crate) fn client(&self) -> &Client {
        self.message.client()
    }

    /// Answers the guest query with an inline result.
    ///
    /// Awaiting the request returns the [`SentGuestMessage`](crate::types::SentGuestMessage).
    pub fn answer(&self, result: impl Into<InlineQueryResult>) -> AnswerGuestQuery<'_> {
        self.client().answer_guest_query(self.query_id, result)
    }

    /// Answers the guest query with an article that sends `text` when the user picks it.
    ///
    /// The result's title is left unset. Awaiting the request returns the
    /// [`SentGuestMessage`](crate::types::SentGuestMessage).
    pub fn answer_text(&self, text: impl Into<Text>) -> AnswerGuestQuery<'_> {
        self.answer_article(InputMessageContent::text(text))
    }

    /// Answers the guest query with an article that sends the rich message `rich` when the user picks it.
    ///
    /// The result's title is left unset. Awaiting the request returns the
    /// [`SentGuestMessage`](crate::types::SentGuestMessage).
    pub fn answer_rich(&self, rich: InputRichMessage) -> AnswerGuestQuery<'_> {
        self.answer_article(InputMessageContent::Rich(rich))
    }

    fn answer_article(&self, content: InputMessageContent) -> AnswerGuestQuery<'_> {
        self.client().answer_guest_query(self.query_id, InlineQueryResultArticle::untitled(content))
    }
}

#[cfg(test)]
mod tests {
    use super::GuestMessage;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{Identifiable, base, types};
    use crate::types::messages_and_media::message::tests::service;
    use crate::types::{InlineQueryResultArticle, Message};
    use crate::utils::get_peer;

    /// `InputBotInlineResult`'s `title` flag.
    const TITLE: u32 = 1 << 1;

    fn update(reference_messages: Option<Vec<base::Message>>) -> types::UpdateBotGuestChatQuery {
        types::UpdateBotGuestChatQuery {
            query_id: 1,
            message: base::Message::Message(Box::new(Message::bare(1, get_peer(7), 0))),
            reference_messages,
            qts: 0,
        }
    }

    fn parsed_guest(client: &Client) -> GuestMessage {
        GuestMessage::parse(client, update(None), &Entities::default()).expect("a normal message parses")
    }

    /// The flags of the `InputBotInlineResult` that a `SetBotGuestChatResult` body carries after the query id.
    fn flags(body: &[u8]) -> u32 {
        let result = &body[12..];
        let constructor = u32::from_le_bytes(result[..4].try_into().unwrap());
        assert_eq!(constructor, <types::InputBotInlineResult as Identifiable>::CONSTRUCTOR_ID);
        u32::from_le_bytes(result[4..8].try_into().unwrap())
    }

    #[test]
    fn reference_messages_keep_only_normal_messages() {
        let client = Client::offline();
        let normal = base::Message::Message(Box::new(Message::bare(2, get_peer(7), 0)));
        let pinned = base::Message::MessageService(Box::new(service(base::MessageAction::MessageActionPinMessage)));
        let raw = update(Some(vec![normal, pinned]));
        let guest = GuestMessage::parse(&client, raw, &Entities::default()).unwrap();
        assert_eq!(guest.reference_messages.len(), 1);
        assert_eq!(guest.reference_messages[0].id, 2);
    }

    #[tokio::test]
    async fn answer_text_leaves_the_title_unset() {
        let (client, mut rx) = Client::recording();
        let guest = parsed_guest(&client);
        let _ = guest.answer_text("hi").await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(flags(&body), 0, "nothing but the result's id and type is set");
        assert!(body.windows(7).any(|w| w == "article".as_bytes()), "the result isn't an article");
    }

    #[tokio::test]
    async fn a_titled_article_sets_the_title_flag() {
        let (client, mut rx) = Client::recording();
        let guest = parsed_guest(&client);
        let _ = guest.answer(InlineQueryResultArticle::new("t", "hi")).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(flags(&body) & TITLE, TITLE, "a titled article sets the flag");
    }
}
