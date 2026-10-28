use crate::dispatcher::Entities;
use crate::enums::PollType;
use crate::raw::base;
use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    pub id: i64,
    pub question: String,
    pub question_entities: Option<Vec<MessageEntity>>,
    pub options: Vec<PollOption>,
    pub total_voter_count: i32,
    pub is_closed: bool,
    pub is_anonymous: bool,
    pub kind: PollType,
    pub allows_multiple_answers: bool,
    /// Index into [`Poll::options`] of the right answer of a quiz, once the client may see it.
    pub correct_option_id: Option<usize>,
    pub explanation: Option<String>,
    pub explanation_entities: Option<Vec<MessageEntity>>,
    pub open_period: Option<i32>,
    pub close_date: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOption {
    pub text: String,
    pub text_entities: Option<Vec<MessageEntity>>,
    pub voter_count: i32,
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
                let (text, data) = match answer {
                    base::PollAnswer::PollAnswer(a) => (&a.text, a.option.clone()),
                    base::PollAnswer::InputPollAnswer(a) => (&a.text, Vec::new()),
                };
                let (text, text_entities) = text_with_entities(text, entities);
                let voter_count = voters_of(&data).and_then(|v| v.voters).unwrap_or(0);
                PollOption { text, text_entities, voter_count, data }
            })
            .collect();
        let correct_option_id = options.iter().position(|o| voters_of(&o.data).is_some_and(|v| v.correct));
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
            correct_option_id,
            explanation: results.solution.clone(),
            explanation_entities,
            open_period: poll.close_period,
            close_date: poll.close_date,
        }
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
mod tests {
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

    #[test]
    fn matches_results_to_options() {
        let answer = |t: &str, option: u8| {
            base::PollAnswer::PollAnswer(Box::new(types::PollAnswer {
                text: text(t),
                option: vec![option],
                media: None,
                added_by: None,
                date: None,
            }))
        };
        let poll = base::Poll::Poll(Box::new(types::Poll {
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
        }));
        let voters = |option: u8, voters: i32, correct: bool| {
            base::PollAnswerVoters::PollAnswerVoters(Box::new(types::PollAnswerVoters {
                chosen: false,
                correct,
                option: vec![option],
                voters: Some(voters),
                recent_voters: None,
            }))
        };
        let results = base::PollResults::PollResults(Box::new(types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: Some(vec![voters(1, 5, true)]),
            total_voters: Some(5),
            recent_voters: None,
            solution: Some("Math".into()),
            solution_entities: None,
            solution_media: None,
        }));
        let poll = Poll::parse(&poll, &results, &Entities::default());
        let counts: Vec<_> = poll.options.iter().map(|o| (o.text.as_str(), o.voter_count)).collect();
        assert_eq!(counts, [("3", 0), ("4", 5)]);
        assert_eq!((poll.kind, poll.correct_option_id, poll.total_voter_count), (PollType::Quiz, Some(1), 5));
        assert_eq!((poll.is_closed, poll.is_anonymous, poll.explanation.as_deref()), (true, true, Some("Math")));
    }
}
