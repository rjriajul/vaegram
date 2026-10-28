use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendPoll<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    poll: types::Poll,
    correct_answers: Option<Vec<i32>>,
    explanation: Option<String>,
    explanation_parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_poll(
        &self,
        chat_id: impl Into<PeerRef>,
        question: impl Into<String>,
        options: impl IntoIterator<Item = impl Into<String>>,
    ) -> SendPoll<'_> {
        let answers = options
            .into_iter()
            .map(|text| {
                let answer = types::InputPollAnswer { text: text_with_entities(text.into()), media: None };
                base::PollAnswer::InputPollAnswer(Box::new(answer))
            })
            .collect();
        SendPoll {
            client: self,
            chat_id: chat_id.into(),
            poll: poll(0, question.into(), answers),
            correct_answers: None,
            explanation: None,
            explanation_parse_mode: None,
            options: SendOptions::default(),
        }
    }
}

impl SendPoll<'_> {
    /// Shows who voted; polls are anonymous by default.
    pub fn public_voters(mut self) -> Self {
        self.poll.public_voters = true;
        self
    }

    pub fn allows_multiple_answers(mut self) -> Self {
        self.poll.multiple_choice = true;
        self
    }

    pub fn quiz(mut self, correct_option_ids: impl IntoIterator<Item = i32>) -> Self {
        self.poll.quiz = true;
        self.correct_answers = Some(correct_option_ids.into_iter().collect());
        self
    }

    pub fn explanation(mut self, text: impl Into<String>) -> Self {
        self.explanation = Some(text.into());
        self
    }

    pub fn explanation_parse_mode(mut self, mode: ParseMode) -> Self {
        self.explanation_parse_mode = Some(mode);
        self
    }

    pub fn open_period(mut self, seconds: i32) -> Self {
        self.poll.close_period = Some(seconds);
        self
    }

    pub fn close_date(mut self, unix_time: i32) -> Self {
        self.poll.close_date = Some(unix_time);
        self
    }

    pub fn is_closed(mut self) -> Self {
        self.poll.closed = true;
        self
    }

    pub fn members_only(mut self) -> Self {
        self.poll.subscribers_only = true;
        self
    }

    pub fn country_codes(mut self, codes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.poll.countries_iso2 = Some(codes.into_iter().map(Into::into).collect());
        self
    }

    async fn send(mut self) -> Result<Message> {
        let solution = match self.explanation.take() {
            Some(text) => Some(self.client.parse_text(&text, self.explanation_parse_mode).await?),
            None => None,
        };
        let (client, chat_id, options) = (self.client, self.chat_id.clone(), std::mem::take(&mut self.options));
        client.send_media(chat_id, self.media(solution), "", None, options).await
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

fn text_with_entities(text: String) -> base::TextWithEntities {
    base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities: Vec::new() }))
}

#[cfg(test)]
mod tests {
    use super::text_with_entities;
    use crate::Client;
    use crate::raw::{Serializable, base, types};

    #[test]
    fn maps_quiz_and_voting_limits() {
        let client = Client::offline();
        let media = client
            .send_poll(1, "2 + 2?", ["3", "4"])
            .public_voters()
            .allows_multiple_answers()
            .quiz([1])
            .open_period(30)
            .members_only()
            .country_codes(["US", "DE"])
            .media(Some(("Basic math".into(), Vec::new())));
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
}
