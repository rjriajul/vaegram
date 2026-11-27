//! What a Bot API message holds besides its text and files, as the TL media a friendly `Message` reads: places,
//! contacts, dice, polls, checklists and games. `convert` turns files itself and falls back on [`media`] for the rest.

use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::file_id::FileId;
use crate::raw::{base, types};
use crate::utils::get_peer;

/// The media of `m` that isn't a file, with the users and chats it names added to `peers`.
pub(crate) fn media(m: &bot::Message, peers: &mut Peers) -> Option<base::MessageMedia> {
    // A venue comes with its location set too, so it goes first.
    if let Some(v) = &m.venue {
        let (provider, venue_id, venue_type) = match (&v.foursquare_id, &v.google_place_id) {
            (Some(id), _) => ("foursquare", id.clone(), v.foursquare_type.clone()),
            (None, Some(id)) => ("gplaces", id.clone(), v.google_place_type.clone()),
            (None, None) => ("", String::new(), None),
        };
        let venue = types::MessageMediaVenue {
            geo: geo(&v.location),
            title: v.title.clone(),
            address: v.address.clone(),
            provider: provider.into(),
            venue_id,
            venue_type: venue_type.unwrap_or_default(),
        };
        return Some(base::MessageMedia::MessageMediaVenue(Box::new(venue)));
    }
    if let Some(l) = &m.location {
        return Some(location(l));
    }
    if let Some(c) = &m.contact {
        let contact = types::MessageMediaContact {
            phone_number: c.phone_number.clone(),
            first_name: c.first_name.clone(),
            last_name: c.last_name.clone().unwrap_or_default(),
            vcard: c.vcard.clone().unwrap_or_default(),
            user_id: c.user_id.unwrap_or(0),
        };
        return Some(base::MessageMedia::MessageMediaContact(Box::new(contact)));
    }
    if let Some(d) = &m.dice {
        let dice = types::MessageMediaDice { value: int(d.value), emoticon: d.emoji.clone(), game_outcome: None };
        return Some(base::MessageMedia::MessageMediaDice(Box::new(dice)));
    }
    if let Some(p) = &m.poll {
        return Some(poll(p, peers));
    }
    if let Some(c) = &m.checklist {
        return Some(checklist(c, peers));
    }
    let g = m.game.as_ref()?;
    // The Bot API names a game by neither id nor short name, and its photo would need the sizes `convert` keeps to
    // itself, so the game holds its title and description.
    let game = types::Game {
        id: 0,
        access_hash: 0,
        short_name: String::new(),
        title: g.title.clone(),
        description: g.description.clone(),
        photo: base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }),
        document: None,
    };
    let game = types::MessageMediaGame { game: base::Game::Game(Box::new(game)) };
    Some(base::MessageMedia::MessageMediaGame(Box::new(game)))
}

fn geo(l: &bot::Location) -> base::GeoPoint {
    base::GeoPoint::GeoPoint(Box::new(types::GeoPoint {
        long: l.longitude,
        lat: l.latitude,
        access_hash: 0,
        accuracy_radius: l.horizontal_accuracy.map(|meters| meters as i32),
    }))
}

/// A static location, or a live one when it has a live period.
fn location(l: &bot::Location) -> base::MessageMedia {
    let Some(period) = l.live_period else {
        return base::MessageMedia::MessageMediaGeo(Box::new(types::MessageMediaGeo { geo: geo(l) }));
    };
    base::MessageMedia::MessageMediaGeoLive(Box::new(types::MessageMediaGeoLive {
        geo: geo(l),
        heading: l.heading.map(int),
        period: int(period),
        proximity_notification_radius: l.proximity_alert_radius.map(int),
    }))
}

fn text(text: &str, entities: Option<&[bot::MessageEntity]>, peers: &mut Peers) -> base::TextWithEntities {
    let entities = entities.map(|list| convert::entities(list, peers)).unwrap_or_default();
    base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
}

/// A poll with its results. An option's data is its persistent id, and a quiz's right answers are marked when the
/// Bot API shows them.
pub(super) fn poll(p: &bot::Poll, peers: &mut Peers) -> base::MessageMedia {
    let correct = p.correct_option_ids.as_deref().unwrap_or_default();
    let mut answers = Vec::with_capacity(p.options.len());
    let mut voters = Vec::with_capacity(p.options.len());
    for (index, o) in (0_i64..).zip(&p.options) {
        let option = o.persistent_id.clone().into_bytes();
        voters.push(base::PollAnswerVoters::PollAnswerVoters(Box::new(types::PollAnswerVoters {
            chosen: false,
            correct: correct.contains(&index),
            option: option.clone(),
            // `voters` and `recent_voters` share a flag, so both are set.
            voters: Some(int(o.voter_count)),
            recent_voters: Some(Vec::new()),
        })));
        let answer = types::PollAnswer {
            text: text(&o.text, o.text_entities.as_deref(), peers),
            option,
            media: None,
            added_by: None,
            date: None,
        };
        answers.push(base::PollAnswer::PollAnswer(Box::new(answer)));
    }
    // `solution` and `solution_entities` share a flag, so both are set or neither.
    let solution_entities = p
        .explanation
        .as_ref()
        .map(|_| p.explanation_entities.as_deref().map(|list| convert::entities(list, peers)).unwrap_or_default());
    let results = types::PollResults {
        min: false,
        has_unread_votes: false,
        can_view_stats: false,
        results: Some(voters),
        total_voters: Some(int(p.total_voter_count)),
        recent_voters: None,
        solution: p.explanation.clone(),
        solution_entities,
        solution_media: None,
    };
    let poll = types::Poll {
        id: p.id.parse().unwrap_or(0),
        closed: p.is_closed,
        public_voters: !p.is_anonymous,
        multiple_choice: p.allows_multiple_answers,
        quiz: p.r#type == "quiz",
        open_answers: false,
        revoting_disabled: !p.allows_revoting,
        shuffle_answers: false,
        hide_results_until_close: false,
        creator: false,
        subscribers_only: p.members_only,
        question: text(&p.question, p.question_entities.as_deref(), peers),
        answers,
        close_period: p.open_period.map(int),
        close_date: p.close_date.map(int),
        countries_iso2: p.country_codes.clone(),
        hash: 0,
    };
    base::MessageMedia::MessageMediaPoll(Box::new(types::MessageMediaPoll {
        poll: base::Poll::Poll(Box::new(poll)),
        results: base::PollResults::PollResults(Box::new(results)),
        attached_media: None,
    }))
}

/// A checklist, with who completed each done task and when.
fn checklist(c: &bot::Checklist, peers: &mut Peers) -> base::MessageMedia {
    let mut list = Vec::with_capacity(c.tasks.len());
    let mut completions = Vec::new();
    for task in &c.tasks {
        let id = int(task.id);
        let completed_by = match (&task.completed_by_user, &task.completed_by_chat) {
            (Some(user), _) => {
                peers.user(user);
                Some(user.id)
            }
            (None, Some(chat)) => {
                peers.chat(chat);
                Some(chat.id)
            }
            (None, None) => None,
        };
        if let Some(by) = completed_by {
            let date = int(task.completion_date.unwrap_or(0));
            let done = types::TodoCompletion { id, completed_by: get_peer(by), date };
            completions.push(base::TodoCompletion::TodoCompletion(Box::new(done)));
        }
        let title = text(&task.text, task.text_entities.as_deref(), peers);
        list.push(base::TodoItem::TodoItem(Box::new(types::TodoItem { id, title })));
    }
    let todo = types::TodoList {
        others_can_append: c.others_can_add_tasks.unwrap_or(false),
        others_can_complete: c.others_can_mark_tasks_as_done.unwrap_or(false),
        title: text(&c.title, c.title_entities.as_deref(), peers),
        list,
    };
    base::MessageMedia::MessageMediaToDo(Box::new(types::MessageMediaToDo {
        todo: base::TodoList::TodoList(Box::new(todo)),
        completions: (!completions.is_empty()).then_some(completions),
    }))
}

/// A profile audio as the TL document its file id decodes to; `None` for an id that doesn't decode.
pub(super) fn audio_document(a: &bot::Audio) -> Option<base::Document> {
    let id = FileId::decode(&a.file_id).ok()?;
    let audio = types::DocumentAttributeAudio {
        voice: false,
        duration: int(a.duration),
        title: a.title.clone(),
        performer: a.performer.clone(),
        waveform: None,
    };
    let mut attributes = vec![base::DocumentAttribute::DocumentAttributeAudio(Box::new(audio))];
    if let Some(file_name) = &a.file_name {
        let name = types::DocumentAttributeFilename { file_name: file_name.clone() };
        attributes.push(base::DocumentAttribute::DocumentAttributeFilename(Box::new(name)));
    }
    let mime_type = a.mime_type.clone().unwrap_or_else(|| "audio/mpeg".into());
    let size = a.file_size.unwrap_or(0);
    let document =
        types::Document::new(id.media_id, id.access_hash, id.file_reference, 0, mime_type, size, id.dc_id, attributes);
    Some(base::Document::Document(Box::new(document)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn message(content: serde_json::Value) -> bot::Message {
        let mut message = json!({ "message_id": 1, "date": 2, "chat": { "id": 7, "type": "private" } });
        message.as_object_mut().unwrap().extend(content.as_object().unwrap().clone());
        serde_json::from_value(message).unwrap()
    }

    #[test]
    fn a_venue_wins_over_the_location_it_comes_with() {
        let location = json!({ "latitude": 1.5, "longitude": 2.5 });
        let m = message(json!({
            "location": location,
            "venue": { "location": location, "title": "Cafe", "address": "Main St", "google_place_id": "g1" },
        }));
        let Some(base::MessageMedia::MessageMediaVenue(venue)) = media(&m, &mut Peers::new(9)) else {
            panic!("not a venue")
        };
        assert_eq!((venue.provider.as_str(), venue.venue_id.as_str()), ("gplaces", "g1"));

        let live = message(json!({ "location": { "latitude": 1.5, "longitude": 2.5, "live_period": 60 } }));
        let Some(base::MessageMedia::MessageMediaGeoLive(geo)) = media(&live, &mut Peers::new(9)) else {
            panic!("not a live location")
        };
        assert_eq!(geo.period, 60);
    }

    #[test]
    fn a_quiz_marks_its_right_answer_and_keeps_counts() {
        let m = message(json!({ "poll": {
            "id": "55", "question": "2 + 2?", "total_voter_count": 3, "is_closed": true, "is_anonymous": false,
            "type": "quiz", "allows_multiple_answers": false, "allows_revoting": false, "members_only": false,
            "correct_option_ids": [1], "explanation": "Math",
            "options": [
                { "persistent_id": "a", "text": "3", "voter_count": 1 },
                { "persistent_id": "b", "text": "4", "voter_count": 2 },
            ],
        }}));
        let Some(base::MessageMedia::MessageMediaPoll(sent)) = media(&m, &mut Peers::new(9)) else {
            panic!("not a poll")
        };
        let base::Poll::Poll(poll) = &sent.poll;
        assert_eq!((poll.id, poll.quiz, poll.closed, poll.public_voters), (55, true, true, true));
        assert_eq!(poll.answers.len(), 2);
        let base::PollResults::PollResults(results) = &sent.results;
        let voters = results.results.as_deref().unwrap();
        let base::PollAnswerVoters::PollAnswerVoters(right) = &voters[1];
        assert_eq!((right.option.as_slice(), right.correct, right.voters), (&b"b"[..], true, Some(2)));
        let entities = results.solution_entities.as_ref().map(Vec::len);
        assert_eq!((results.solution.as_deref(), entities), (Some("Math"), Some(0)));
    }

    #[test]
    fn a_done_task_names_who_completed_it() {
        let m = message(json!({ "checklist": { "title": "Trip", "tasks": [
            { "id": 1, "text": "Tickets", "completed_by_user": { "id": 8, "is_bot": false, "first_name": "Ann" },
              "completion_date": 9 },
            { "id": 2, "text": "Bags" },
        ]}}));
        let mut peers = Peers::new(9);
        let Some(base::MessageMedia::MessageMediaToDo(checklist_media)) = media(&m, &mut peers) else {
            panic!("not a checklist")
        };
        let completions = checklist_media.completions.as_deref().unwrap();
        let base::TodoCompletion::TodoCompletion(done) = &completions[0];
        assert_eq!((completions.len(), done.id, done.completed_by.clone(), done.date), (1, 1, get_peer(8), 9));
        assert_eq!(peers.users.len(), 1);
    }
}
