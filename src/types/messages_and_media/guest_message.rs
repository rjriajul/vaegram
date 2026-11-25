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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    /// The result's title is the first non-empty line of `text`, cut to 64 characters, because Telegram refuses an
    /// article without one (`ARTICLE_TITLE_EMPTY`). Awaiting the request returns the
    /// [`SentGuestMessage`](crate::types::SentGuestMessage).
    pub fn answer_text(&self, text: impl Into<Text>) -> AnswerGuestQuery<'_> {
        let text: Text = text.into();
        self.answer_article(title_of(text.as_str()), InputMessageContent::text(text))
    }

    /// Answers the guest query with an article that sends the rich message `rich` when the user picks it.
    ///
    /// The result's title is "Answer", because Telegram refuses an article without one (`ARTICLE_TITLE_EMPTY`) and a
    /// rich message has no plain text to take it from. Awaiting the request returns the
    /// [`SentGuestMessage`](crate::types::SentGuestMessage).
    pub fn answer_rich(&self, rich: InputRichMessage) -> AnswerGuestQuery<'_> {
        self.answer_article(DEFAULT_TITLE.to_owned(), InputMessageContent::Rich(rich))
    }

    fn answer_article(&self, title: String, content: InputMessageContent) -> AnswerGuestQuery<'_> {
        self.client().answer_guest_query(self.query_id, InlineQueryResultArticle::new(title, content))
    }
}

/// The title of an article whose content has no text to take one from.
const DEFAULT_TITLE: &str = "Answer";

/// The first non-empty line of `text`, at most 64 characters. [`DEFAULT_TITLE`] if there is none.
fn title_of(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("");
    let title: String = line.chars().take(64).collect();
    if title.is_empty() { DEFAULT_TITLE.to_owned() } else { title }
}

#[cfg(test)]
mod tests {
    use super::GuestMessage;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::messages_and_media::message::tests::service;
    use crate::types::{InlineQueryResultArticle, InputRichMessage, Message};
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

    /// Where the result's `send_message` starts in a body whose result flags are all clear: after the flags, the
    /// `id` string and the `type` string, each a length byte and its bytes padded to four.
    fn send_message_at(body: &[u8]) -> usize {
        let mut at = 20;
        for _ in 0..2 {
            at += (1 + usize::from(body[at])).next_multiple_of(4);
        }
        at
    }

    #[test]
    fn reference_messages_keep_only_normal_messages() {
        let client = Client::offline();
        let normal = base::Message::Message(Box::new(Message::bare(2, get_peer(7), 0)));
        let pinned = base::Message::MessageService(Box::new(service(base::MessageAction::MessageActionPinMessage)));
        let empty = base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 3, peer_id: None }));
        let raw = update(Some(vec![normal, pinned, empty]));
        let guest = GuestMessage::parse(&client, raw, &Entities::default()).unwrap();
        assert_eq!(guest.reference_messages.len(), 1);
        assert_eq!(guest.reference_messages[0].id, 2);
        let none = GuestMessage::parse(&client, update(None), &Entities::default()).unwrap();
        assert!(none.reference_messages.is_empty(), "no reference messages is an empty vec");
    }

    #[tokio::test]
    async fn answer_text_titles_the_article_with_the_first_line() {
        let (client, mut rx) = Client::recording_raw();
        let guest = parsed_guest(&client);
        let _ = guest.answer_text("\n  first line  \nsecond").await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(flags(&body), TITLE, "only the title is set: Telegram refuses an article without one");
        assert!(body.windows(7).any(|w| w == "article".as_bytes()), "the result isn't an article");
        let title = String::from("first line").to_bytes();
        let at = send_message_at(&body);
        assert_eq!(body[at..at + title.len()], title[..], "the title follows the type string");
    }

    #[tokio::test]
    async fn answer_rich_sends_a_rich_message_with_a_title() {
        let (client, mut rx) = Client::recording_raw();
        let guest = parsed_guest(&client);
        let _ = guest.answer_rich(InputRichMessage::html("<b>hi</b>")).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(flags(&body), TITLE, "only the title is set");
        let title = String::from("Answer").to_bytes();
        let at = send_message_at(&body);
        assert_eq!(body[at..at + title.len()], title[..], "the title follows the type string");
        let at = at + title.len();
        let send_message = u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
        assert_eq!(send_message, <types::InputBotInlineMessageRichMessage as Identifiable>::CONSTRUCTOR_ID);
    }

    #[test]
    fn a_title_is_never_empty() {
        assert_eq!(super::title_of(""), "Answer");
        assert_eq!(super::title_of(" \n\t\n"), "Answer");
        assert_eq!(super::title_of("a\nb"), "a");
        assert_eq!(super::title_of(&"x".repeat(100)).chars().count(), 64);
    }

    #[tokio::test]
    async fn a_titled_article_sets_the_title_flag() {
        let (client, mut rx) = Client::recording_raw();
        let guest = parsed_guest(&client);
        let _ = guest.answer(InlineQueryResultArticle::new("t", "hi")).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(flags(&body) & TITLE, TITLE, "a titled article sets the flag");
    }
}
