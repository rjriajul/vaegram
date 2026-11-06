use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::messages_and_media::PollMedia;
use crate::types::{InputPollMedia, InputPollOption, Message, PeerRef, Text};

/// Request built by [`Client::send_poll`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendPoll<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    question: Text,
    answers: Vec<Text>,
    // Settings only: the question and answers go in when the request is sent.
    poll: types::Poll,
    correct_answers: Option<Vec<i32>>,
    explanation: Option<Text>,
    explanation_parse_mode: Option<ParseMode>,
    options: SendOptions,
    poll_media: PollMedia<'a>,
}

impl Client {
    /// Sends a poll.
    ///
    /// A poll takes 1 to 12 options, and any other count fails with `Error::InvalidArgument`. The question and the
    /// option texts keep their formatting; one made straight from a string is parsed with the client's default parse
    /// mode. Awaiting the request returns the sent [`Message`].
    pub fn send_poll<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        question: impl Into<Text>,
        options: impl IntoIterator<Item = impl Into<InputPollOption<'a>>>,
    ) -> SendPoll<'a> {
        let mut poll_media = PollMedia::default();
        let answers = options
            .into_iter()
            .map(|option| {
                let InputPollOption { text, media } = option.into();
                poll_media.options.push(media);
                text
            })
            .collect();
        SendPoll {
            client: self,
            chat_id: chat_id.into(),
            question: question.into(),
            answers,
            poll: poll(0, String::new(), Vec::new()),
            correct_answers: None,
            explanation: None,
            explanation_parse_mode: None,
            options: SendOptions::default(),
            poll_media,
        }
    }
}

impl<'a> SendPoll<'a> {
    /// Shows who voted. Polls are anonymous by default.
    pub fn public_voters(mut self) -> Self {
        self.poll.public_voters = true;
        self
    }

    /// Lets voters pick more than one option.
    pub fn allows_multiple_answers(mut self) -> Self {
        self.poll.multiple_choice = true;
        self
    }

    /// Makes the poll a quiz with `correct_option_ids` as the correct answers.
    pub fn quiz(mut self, correct_option_ids: impl IntoIterator<Item = i32>) -> Self {
        self.poll.quiz = true;
        self.correct_answers = Some(correct_option_ids.into_iter().collect());
        self
    }

    /// Text shown after a wrong answer in a quiz.
    pub fn explanation(mut self, text: impl Into<Text>) -> Self {
        self.explanation = Some(text.into());
        self
    }

    /// Parse mode for the explanation. Without it the client's default parse mode applies.
    pub fn explanation_parse_mode(mut self, mode: ParseMode) -> Self {
        self.explanation_parse_mode = Some(mode);
        self
    }

    /// Closes the poll this many seconds after it is sent.
    pub fn open_period(mut self, seconds: i32) -> Self {
        self.poll.close_period = Some(seconds);
        self
    }

    /// Closes the poll at this Unix timestamp.
    pub fn close_date(mut self, unix_time: i32) -> Self {
        self.poll.close_date = Some(unix_time);
        self
    }

    /// Sends the poll already closed.
    pub fn is_closed(mut self) -> Self {
        self.poll.closed = true;
        self
    }

    /// Lets only subscribers vote.
    pub fn members_only(mut self) -> Self {
        self.poll.subscribers_only = true;
        self
    }

    /// Lets members add options later, through [`Client::add_poll_option`].
    pub fn allow_adding_options(mut self) -> Self {
        self.poll.open_answers = true;
        self
    }

    /// Limits the poll to users from these countries, given as two-letter ISO codes.
    pub fn country_codes(mut self, codes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.poll.countries_iso2 = Some(codes.into_iter().map(Into::into).collect());
        self
    }

    /// Attaches media to the poll description.
    pub fn description_media(mut self, media: impl Into<InputPollMedia<'a>>) -> Self {
        self.poll_media.description = Some(media.into());
        self
    }

    /// Attaches media to the quiz explanation.
    pub fn explanation_media(mut self, media: impl Into<InputPollMedia<'a>>) -> Self {
        self.poll_media.explanation = Some(media.into());
        self
    }

    async fn write_texts(&mut self) -> Result<()> {
        if !(1..=12).contains(&self.answers.len()) {
            return Err(Error::InvalidArgument("a poll takes 1 to 12 options"));
        }
        self.poll.question = write_text_with_entities(self.client, std::mem::take(&mut self.question)).await?;
        for text in std::mem::take(&mut self.answers) {
            let text = write_text_with_entities(self.client, text).await?;
            let answer = types::InputPollAnswer { text, media: None };
            self.poll.answers.push(base::PollAnswer::InputPollAnswer(Box::new(answer)));
        }
        Ok(())
    }

    async fn send(mut self) -> Result<Message> {
        self.write_texts().await?;
        let solution = match self.explanation.take() {
            Some(text) => Some(self.client.write_text(text, self.explanation_parse_mode).await?),
            None => None,
        };
        let (client, chat_id, options) = (self.client, self.chat_id.clone(), std::mem::take(&mut self.options));
        let poll_media = std::mem::take(&mut self.poll_media);
        let mut media = self.media(solution);
        poll_media.attach(client, &chat_id, options.business_connection_id.as_deref(), &mut media).await?;
        client.send_media(chat_id, media, "", None, options).await
    }

    fn media(self, solution: Option<(String, Vec<base::MessageEntity>)>) -> base::InputMedia {
        // solution and solution_entities share flags.1, so they must be set together.
        let (solution, solution_entities) = solution.unzip();
        base::InputMedia::InputMediaPoll(Box::new(types::InputMediaPoll {
            poll: base::Poll::Poll(Box::new(self.poll)),
            correct_answers: self.correct_answers,
            attached_media: None,
            solution,
            solution_entities,
            solution_media: None,
        }))
    }
}

send_builder!(SendPoll);

pub(crate) fn poll(id: i64, question: String, answers: Vec<base::PollAnswer>) -> types::Poll {
    types::Poll {
        id,
        closed: false,
        public_voters: false,
        multiple_choice: false,
        quiz: false,
        open_answers: false,
        revoting_disabled: false,
        shuffle_answers: false,
        hide_results_until_close: false,
        creator: false,
        subscribers_only: false,
        question: text_with_entities(question),
        answers,
        close_period: None,
        close_date: None,
        countries_iso2: None,
        hash: 0,
    }
}

/// Writes `text` with its entities, parsing one made straight from a string with the client's default parse mode.
pub(super) async fn write_text_with_entities(client: &Client, text: Text) -> Result<base::TextWithEntities> {
    let (text, entities) = client.write_text(text, None).await?;
    Ok(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })))
}

pub(super) fn text_with_entities(text: String) -> base::TextWithEntities {
    base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities: Vec::new() }))
}

#[cfg(test)]
mod tests {
    use super::text_with_entities;
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, types};
    use crate::types::{FileSource, InputMedia, InputPollMedia, InputPollOption, Text};

    #[tokio::test]
    async fn maps_quiz_and_voting_limits() {
        let client = Client::offline();
        let mut send = client
            .send_poll(1, "2 + 2?", ["3", "4"])
            .public_voters()
            .allows_multiple_answers()
            .quiz([1])
            .open_period(30)
            .members_only()
            .country_codes(["US", "DE"]);
        send.write_texts().await.unwrap();
        let media = send.media(Some(("Basic math".into(), Vec::new())));
        let base::InputMedia::InputMediaPoll(media) = media else { panic!("not a poll") };
        assert_eq!(
            (&media.correct_answers, media.solution.as_deref(), &media.solution_entities),
            (&Some(vec![1]), Some("Basic math"), &Some(Vec::new()))
        );

        let base::Poll::Poll(poll) = &media.poll;
        assert_eq!(poll.question, text_with_entities("2 + 2?".into()));
        let answer = types::InputPollAnswer { text: text_with_entities("4".into()), media: None };
        assert_eq!(poll.answers[1], base::PollAnswer::InputPollAnswer(Box::new(answer)));
        assert_eq!(
            (poll.public_voters, poll.quiz, poll.subscribers_only, poll.multiple_choice, poll.closed),
            (true, true, true, true, false)
        );
        assert_eq!(poll.close_period, Some(30));
        assert_eq!(poll.countries_iso2, Some(vec!["US".to_owned(), "DE".to_owned()]));

        let mut buf = Vec::new();
        media.poll.serialize(&mut buf);
        let flags = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        assert_eq!(flags, (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 11) | (1 << 12));
    }

    #[test]
    fn plain_poll_is_anonymous_without_solution() {
        let client = Client::offline();
        let media = client.send_poll(1, "Lunch?", ["Pizza", "Sushi"]).allows_multiple_answers().media(None);
        let base::InputMedia::InputMediaPoll(media) = media else { panic!("not a poll") };
        assert!(media.correct_answers.is_none() && media.solution.is_none() && media.solution_entities.is_none());
        let base::Poll::Poll(poll) = &media.poll;
        assert!(poll.multiple_choice && !poll.public_voters && !poll.quiz);
        assert_eq!(poll.countries_iso2, None);
    }

    #[test]
    fn allowing_new_options_opens_the_answers() {
        let client = Client::offline();
        let media = client.send_poll(1, "Lunch?", ["Pizza", "Sushi"]).allow_adding_options().media(None);
        let base::InputMedia::InputMediaPoll(media) = media else { panic!("not a poll") };
        let base::Poll::Poll(poll) = &media.poll;
        assert!(poll.open_answers && !poll.quiz && !poll.multiple_choice);
    }

    #[tokio::test]
    async fn keeps_option_media_in_option_order() {
        let client = Client::offline();
        let options = [
            InputPollOption::new("Paris").media(InputPollMedia::location(48.85, 2.35)),
            "Rome".into(),
            InputPollOption::new("Photo").media(InputMedia::photo("a.jpg")),
        ];
        let mut send = client.send_poll(1, "Where?", options);
        let has_media: Vec<_> = send.poll_media.options.iter().map(Option::is_some).collect();
        assert_eq!(has_media, [true, false, true]);
        send.write_texts().await.unwrap();
        let base::InputMedia::InputMediaPoll(media) = send.media(None) else { panic!("not a poll") };
        let base::Poll::Poll(poll) = &media.poll;
        let rome = types::InputPollAnswer { text: text_with_entities("Rome".into()), media: None };
        assert_eq!(poll.answers[1], base::PollAnswer::InputPollAnswer(Box::new(rome)));
    }

    /// Attaches a URL document as description, explanation and first option media; returns the requests sent.
    async fn attach_url_documents() -> Vec<Vec<u8>> {
        let (client, mut sent) = Client::recording();
        let document = || InputMedia::document(FileSource::url("https://example.com/a.pdf").unwrap());
        let options = [InputPollOption::new("A").media(document()), "B".into()];
        let mut send =
            client.send_poll(-5, "Which?", options).description_media(document()).explanation_media(document());
        let (poll_media, chat_id) = (std::mem::take(&mut send.poll_media), send.chat_id.clone());
        let mut media = send.media(None);
        // The recording session never answers, so every upload that went out fails.
        let result = poll_media.attach(&client, &chat_id, Some("conn"), &mut media).await;
        assert!(matches!(result, Err(Error::Disconnected)), "{result:?}");
        std::iter::from_fn(|| sent.try_recv().ok()).collect()
    }

    #[tokio::test]
    async fn poll_media_uploads_through_the_business_connection() {
        let sent = attach_url_documents().await;
        assert!(!sent.is_empty() && sent.iter().all(|body| body.windows(4).any(|w| w == b"conn")));
    }

    #[tokio::test]
    async fn poll_media_uploads_side_by_side() {
        assert_eq!(attach_url_documents().await.len(), 3);
    }

    #[tokio::test]
    async fn a_built_explanation_keeps_its_entities() {
        let (client, mut sent) = Client::recording();
        let _ = client.send_poll(-5, "2 + 2?", ["3", "4"]).quiz([1]).explanation(Text::new().bold("hi")).await;
        let body = sent.try_recv().expect("no request sent");
        let bold = vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 })];
        let mut solution = String::from("hi").to_bytes();
        solution.extend(bold.to_bytes());
        assert!(body.windows(solution.len()).any(|w| w == solution));
    }

    #[tokio::test]
    async fn the_question_and_options_keep_their_entities_on_the_wire() {
        let (client, mut sent) = Client::recording();
        let options = [InputPollOption::new(Text::new().italic("Pizza")), "Sushi".into()];
        let _ = client.send_poll(-5, Text::new().plain("Lunch ").bold("today"), options).await;
        let body = sent.try_recv().expect("no request sent");
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 6, length: 5 });
        let italic = base::MessageEntity::MessageEntityItalic(types::MessageEntityItalic { offset: 0, length: 5 });
        let has = |bytes: &[u8]| body.windows(bytes.len()).any(|w| w == bytes);
        assert!(has(&written_text("Lunch today", vec![bold]).to_bytes()), "the question lost its bold");
        let pizza = types::InputPollAnswer { text: written_text("Pizza", vec![italic]), media: None };
        let sushi = types::InputPollAnswer { text: written_text("Sushi", Vec::new()), media: None };
        let answers = vec![
            base::PollAnswer::InputPollAnswer(Box::new(pizza)),
            base::PollAnswer::InputPollAnswer(Box::new(sushi)),
        ];
        assert!(has(&answers.to_bytes()), "the options lost their formatting");
    }

    fn written_text(text: &str, entities: Vec<base::MessageEntity>) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
    }

    #[tokio::test]
    async fn empty_or_oversized_lists_are_refused_before_sending() {
        let (client, mut sent) = Client::recording();
        let none: [&str; 0] = [];
        let refused = [
            client.send_poll(-5, "Lunch?", none).await.map(drop).err(),
            client.send_poll(-5, "Lunch?", ["x"; 13]).await.map(drop).err(),
            client.copy_messages(-5, -6, &[]).await.map(drop).err(),
            client.forward_messages(-5, -6, &[]).await.map(drop).err(),
            client.delete_messages(-5, &[]).await.err(),
            client.delete_business_messages("c", &[]).await.err(),
            client.copy_messages(-5, -6, &[1; 101]).await.map(drop).err(),
            client.forward_messages(-5, -6, &[1; 101]).await.map(drop).err(),
            client.delete_messages(-5, &[1; 101]).await.err(),
            client.delete_business_messages("c", &[1; 101]).await.err(),
        ];
        for error in refused {
            assert!(matches!(error, Some(Error::InvalidArgument(_))), "{error:?}");
        }
        assert!(sent.try_recv().is_err());
    }
}
