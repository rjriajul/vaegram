use crate::dispatcher::Entities;
use crate::enums::PollType;
use crate::raw::base;
use crate::types::MessageEntity;

/// A poll or quiz.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Poll {
    /// Telegram's id of the poll.
    pub id: i64,
    /// The question, as plain text.
    pub question: String,
    /// Formatting of the question. `None` if it has none.
    pub question_entities: Option<Vec<MessageEntity>>,
    /// The answer options, in order.
    pub options: Vec<PollOption>,
    /// Number of people who voted. 0 if Telegram sent no count.
    pub total_voter_count: i32,
    /// The poll no longer takes votes.
    pub is_closed: bool,
    /// Voters are hidden.
    pub is_anonymous: bool,
    /// Whether this is a regular poll or a quiz.
    pub kind: PollType,
    /// A voter may pick more than one option.
    pub allows_multiple_answers: bool,
    /// Index into [`Poll::options`] of the right answer of a quiz, once the client may see it. The first one when
    /// the quiz has several; see [`correct_option_ids`](Poll::correct_option_ids).
    pub correct_option_id: Option<usize>,
    /// Indexes into [`Poll::options`] of every right answer of a quiz, once the client may see them. Empty otherwise.
    pub correct_option_ids: Vec<usize>,
    /// Explanation of a quiz's right answer. `None` if there is none or the account can't see it yet.
    pub explanation: Option<String>,
    /// Formatting of the explanation. `None` if it has none.
    pub explanation_entities: Option<Vec<MessageEntity>>,
    /// Seconds the poll stays open after it was sent. `None` if not set.
    pub open_period: Option<i32>,
    /// Unix time the poll closes. `None` if not set.
    pub close_date: Option<i32>,
}

/// One answer option of a [`Poll`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PollOption {
    /// The option text, as plain text.
    pub text: String,
    /// Formatting of the option text. `None` if it has none.
    pub text_entities: Option<Vec<MessageEntity>>,
    /// Number of people who picked this option. 0 if Telegram sent no count.
    pub voter_count: i32,
    /// Telegram's identifier for the option. Empty for an option built locally rather than received.
    pub data: Vec<u8>,
}

impl Poll {
    pub(crate) fn parse(poll: &base::Poll, results: &base::PollResults, entities: &Entities) -> Poll {
        let (base::Poll::Poll(poll), base::PollResults::PollResults(results)) = (poll, results);
        let voters = results.results.as_deref().unwrap_or_default();
        let voters_of = |data: &[u8]| {
            voters.iter().find_map(|v| {
                let base::PollAnswerVoters::PollAnswerVoters(v) = v;
                (v.option == data).then_some(v)
            })
        };
        let options: Vec<PollOption> = poll
            .answers
            .iter()
            .map(|answer| {
                let option = PollOption::from_answer(answer, entities);
                PollOption { voter_count: voters_of(&option.data).and_then(|v| v.voters).unwrap_or(0), ..option }
            })
            .collect();
        let correct_option_ids: Vec<usize> = options
            .iter()
            .enumerate()
            .filter(|(_, o)| voters_of(&o.data).is_some_and(|v| v.correct))
            .map(|(i, _)| i)
            .collect();
        let (question, question_entities) = text_with_entities(&poll.question, entities);
        let explanation_entities =
            results.solution_entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities));
        Poll {
            id: poll.id,
            question,
            question_entities,
            options,
            total_voter_count: results.total_voters.unwrap_or(0),
            is_closed: poll.closed,
            is_anonymous: !poll.public_voters,
            kind: if poll.quiz { PollType::Quiz } else { PollType::Regular },
            allows_multiple_answers: poll.multiple_choice,
            correct_option_id: correct_option_ids.first().copied(),
            correct_option_ids,
            explanation: results.solution.clone(),
            explanation_entities,
            open_period: poll.close_period,
            close_date: poll.close_date,
        }
    }
}

impl PollOption {
    /// An option without its votes, which come with the poll's results.
    pub(crate) fn from_answer(answer: &base::PollAnswer, entities: &Entities) -> PollOption {
        let (text, data) = match answer {
            base::PollAnswer::PollAnswer(a) => (&a.text, a.option.clone()),
            base::PollAnswer::InputPollAnswer(a) => (&a.text, Vec::new()),
        };
        let (text, text_entities) = text_with_entities(text, entities);
        PollOption { text, text_entities, voter_count: 0, data }
    }
}

pub(crate) fn text_with_entities(
    text: &base::TextWithEntities,
    entities: &Entities,
) -> (String, Option<Vec<MessageEntity>>) {
    let base::TextWithEntities::TextWithEntities(t) = text;
    (t.text.clone(), MessageEntity::parse_all(&t.entities, entities))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Poll;
    use crate::dispatcher::Entities;
    use crate::enums::PollType;
    use crate::raw::{base, types};

    fn text(text: &str) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities {
            text: text.into(),
            entities: Vec::new(),
        }))
    }

    /// A closed quiz "2 + 2?" with options "3" (data 0) and "4" (data 1).
    pub(crate) fn raw_poll() -> base::Poll {
        let answer = |t: &str, option: u8| {
            base::PollAnswer::PollAnswer(Box::new(types::PollAnswer {
                text: text(t),
                option: vec![option],
                media: None,
                added_by: None,
                date: None,
            }))
        };
        base::Poll::Poll(Box::new(types::Poll {
            id: 9,
            closed: true,
            public_voters: false,
            multiple_choice: false,
            quiz: true,
            open_answers: false,
            revoting_disabled: false,
            shuffle_answers: false,
            hide_results_until_close: false,
            creator: false,
            subscribers_only: false,
            question: text("2 + 2?"),
            answers: vec![answer("3", 0), answer("4", 1)],
            close_period: None,
            close_date: None,
            countries_iso2: None,
            hash: 0,
        }))
    }

    /// Results with `(option data, voters, correct)` per option.
    pub(crate) fn results(options: &[(u8, i32, bool)], total_voters: i32) -> base::PollResults {
        let voters = options
            .iter()
            .map(|&(option, voters, correct)| {
                base::PollAnswerVoters::PollAnswerVoters(Box::new(types::PollAnswerVoters {
                    chosen: false,
                    correct,
                    option: vec![option],
                    voters: Some(voters),
                    recent_voters: None,
                }))
            })
            .collect();
        base::PollResults::PollResults(Box::new(types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: Some(voters),
            total_voters: Some(total_voters),
            recent_voters: None,
            solution: Some("Math".into()),
            solution_entities: None,
            solution_media: None,
        }))
    }

    #[test]
    fn matches_results_to_options() {
        let poll = Poll::parse(&raw_poll(), &results(&[(1, 5, true)], 5), &Entities::default());
        let counts: Vec<_> = poll.options.iter().map(|o| (o.text.as_str(), o.voter_count)).collect();
        assert_eq!(counts, [("3", 0), ("4", 5)]);
        assert_eq!((poll.kind, poll.correct_option_id, poll.total_voter_count), (PollType::Quiz, Some(1), 5));
        assert_eq!((poll.is_closed, poll.is_anonymous, poll.explanation.as_deref()), (true, true, Some("Math")));
    }

    #[test]
    fn a_quiz_with_several_right_answers_keeps_them_all() {
        let poll = Poll::parse(&raw_poll(), &results(&[(0, 2, true), (1, 5, true)], 7), &Entities::default());
        assert_eq!((poll.correct_option_id, poll.correct_option_ids), (Some(0), vec![0, 1]));
    }
}
