use std::sync::Arc;

use super::Dispatcher;
use super::entities::Entities;
use super::tracker::{Key, channel_of, counter_of};
use crate::enums::SkipReason;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::utils::get_channel_id;

// getChannelDifference caps users at 100 updates per call and bots at 100000.
const CHANNEL_DIFFERENCE_LIMIT: i32 = 100;
const BOT_CHANNEL_DIFFERENCE_LIMIT: i32 = 100_000;

fn new_message(message: base::Message) -> base::Update {
    base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
}

fn new_channel_message(message: base::Message) -> base::Update {
    base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage { message, pts: 0, pts_count: 0 }))
}

fn message_date(message: &base::Message) -> Option<i32> {
    match message {
        base::Message::Message(m) => Some(m.date),
        base::Message::MessageService(m) => Some(m.date),
        base::Message::MessageEmpty(_) => None,
    }
}

fn edit_date(message: &base::Message) -> Option<i32> {
    match message {
        base::Message::Message(m) => Some(m.edit_date.unwrap_or(m.date)),
        other => message_date(other),
    }
}

/// Whether `update`, released before `run` has read what Telegram held, may be one of those: Telegram holds what
/// moves a counter, and it isn't dated `since` or later. Others, such as callback queries and updates with a pts or
/// qts of 0, which [`counter_of`] takes for unnumbered, are never held. Messages a channel difference brings are
/// numbered by it, not by their own pts, so `channel_difference` filters them itself.
pub(super) fn held(update: &base::Update, since: i32) -> bool {
    counter_of(update, None).is_some() && !since_run(Some(since), update_date(update))
}

/// Whether what is dated `date` may reach the handlers while `run` drops what came before `held_before`.
fn since_run(held_before: Option<i32>, date: Option<i32>) -> bool {
    held_before.is_none_or(|since| date.is_some_and(|date| date >= since))
}

/// When Telegram got `update`, for the updates that say. A business connection and a boost give when they were
/// made instead, so a change to an old one is taken for a held update and dropped, as an undated one is.
fn update_date(update: &base::Update) -> Option<i32> {
    use base::Update as U;
    match update {
        U::UpdateNewMessage(u) => message_date(&u.message),
        U::UpdateNewChannelMessage(u) => message_date(&u.message),
        U::UpdateBotNewBusinessMessage(u) => message_date(&u.message),
        U::UpdateEditMessage(u) => edit_date(&u.message),
        U::UpdateEditChannelMessage(u) => edit_date(&u.message),
        U::UpdateBotEditBusinessMessage(u) => edit_date(&u.message),
        U::UpdateBotBusinessConnect(u) => {
            let base::BotBusinessConnection::BotBusinessConnection(connection) = &u.connection;
            Some(connection.date)
        }
        U::UpdateBotChatBoost(u) => {
            let base::Boost::Boost(boost) = &u.boost;
            Some(boost.date)
        }
        U::UpdateChatParticipant(u) => Some(u.date),
        U::UpdateChannelParticipant(u) => Some(u.date),
        U::UpdateBotStopped(u) => Some(u.date),
        U::UpdateBotChatInviteRequester(u) => Some(u.date),
        U::UpdateBotMessageReaction(u) => Some(u.date),
        U::UpdateBotMessageReactions(u) => Some(u.date),
        _ => None,
    }
}

impl Dispatcher {
    pub(super) async fn difference(&mut self) -> Result<()> {
        loop {
            // Each page's updates go to the handlers before the next request, with the state they moved it to.
            self.dispatch_ready();
            let t = &mut self.tracker;
            let request = functions::updates::GetDifference {
                pts: t.state.pts,
                pts_limit: None,
                pts_total_limit: None,
                date: t.state.date,
                qts: t.state.qts,
                qts_limit: None,
            };
            // Straight to the session, not Client::invoke: a FLOOD_WAIT slept here would stop update intake, so it
            // goes to Tracker::recover, which retries after the wait while updates keep flowing.
            let (messages, others, users, chats, state, done) = match self.client.session().invoke(&request).await? {
                base::updates::Difference::DifferenceEmpty(d) => {
                    (t.state.date, t.state.seq) = (d.date, d.seq);
                    return Ok(());
                }
                base::updates::Difference::DifferenceTooLong(d) if d.pts != t.state.pts => {
                    let from = std::mem::replace(&mut t.state.pts, d.pts);
                    // Without catch-up, skipping what came before the run is what was asked for.
                    if self.held_before.is_none() {
                        self.skipped(Key::Pts, from, d.pts, SkipReason::TooLong);
                    }
                    continue;
                }
                base::updates::Difference::DifferenceTooLong(_) => return Ok(()),
                base::updates::Difference::Difference(d) => {
                    let d = *d;
                    (d.new_messages, d.other_updates, d.users, d.chats, d.state, true)
                }
                base::updates::Difference::DifferenceSlice(d) => {
                    let d = *d;
                    (d.new_messages, d.other_updates, d.users, d.chats, d.intermediate_state, false)
                }
            };
            let entities = Arc::new(Entities::new(&self.client, users, chats));
            let held_before = self.held_before;
            for message in messages.into_iter().filter(|m| since_run(held_before, message_date(m))) {
                t.emit(new_message(message), &entities);
                self.recovered.0 += 1;
            }
            // updateChannelTooLong carries no date, but it names a channel whose updates Telegram left out: it goes
            // to the tracker, which fetches the channel, and that fetch drops what is dated before the run.
            let keep = |u: &base::Update| {
                matches!(u, base::Update::UpdateChannelTooLong(_)) || since_run(held_before, update_date(u))
            };
            for update in others.into_iter().filter(keep) {
                self.recovered.1 += 1;
                if let (base::Update::UpdateChannelTooLong(u), Some(since)) = (&update, held_before) {
                    if !self.held_channels.iter().any(|&(id, _)| id == u.channel_id) {
                        self.held_channels.push((u.channel_id, since));
                    }
                }
                if matches!(update, base::Update::UpdateChannelTooLong(_)) || channel_of(&update).is_some() {
                    t.update(update, &entities, false);
                } else {
                    t.emit(update, &entities);
                }
            }
            let base::updates::State::State(state) = state;
            t.set_state(&state);
            if done {
                return Ok(());
            }
        }
    }

    /// updatePtsChanged: the common box's state comes again from updates.getState, straight to the session like
    /// getDifference.
    pub(super) async fn refetch_state(&mut self) -> Result<()> {
        let base::updates::State::State(state) = self.client.session().invoke(&functions::updates::GetState {}).await?;
        self.tracker.restate(&state);
        Ok(())
    }

    pub(super) async fn channel_difference(&mut self, channel_id: i64) -> Result<()> {
        // A channel the first difference of a run without catch-up named: what is dated before the run is dropped
        // as the window did, undated updates too, and what is skipped isn't reported, as `difference` doesn't.
        let since = self.held_channels.iter().find(|&&(id, _)| id == channel_id).map(|&(_, since)| since);
        let Some(&local) = self.tracker.state.channels.get(&channel_id) else {
            // An updateChannelTooLong without a pts for a channel the client keeps none for: nowhere to start from.
            if since.is_none() {
                self.skipped(Key::Channel(channel_id), 0, 0, SkipReason::NoPts);
            }
            return Ok(());
        };
        let mut pts = local;
        let channel = self.client.resolve_channel(&get_channel_id(channel_id).into()).await?;
        let bot = self.client.0.me.lock().unwrap().is_bot;
        let limit = if bot { BOT_CHANNEL_DIFFERENCE_LIMIT } else { CHANNEL_DIFFERENCE_LIMIT };
        let keep = move |m: &base::Message| since_run(since, message_date(m));
        let keep_update = move |u: &base::Update| since_run(since, update_date(u));
        loop {
            self.dispatch_ready();
            let request = functions::updates::GetChannelDifference {
                force: false,
                channel: channel.clone(),
                filter: base::ChannelMessagesFilter::ChannelMessagesFilterEmpty,
                pts,
                limit,
            };
            match self.client.session().invoke(&request).await? {
                base::updates::ChannelDifference::ChannelDifferenceEmpty(d) => {
                    pts = d.pts;
                    break;
                }
                base::updates::ChannelDifference::ChannelDifferenceTooLong(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.messages.into_iter().filter(keep) {
                        self.tracker.emit(new_channel_message(message), &entities);
                        self.recovered.0 += 1;
                    }
                    let to = match &d.dialog {
                        base::Dialog::Dialog(dialog) => dialog.pts,
                        _ => None,
                    };
                    let Some(to) = to else {
                        // No pts came with the messages, e.g. a dialogFolder where the channel's dialog was due.
                        // Kept, the old pts would fetch these messages again at every gap; forgotten, as on
                        // PERSISTENT_TIMESTAMP_INVALID, the channel is started again by the next update pushed for it.
                        // The updates held for the channel go too: the answer's latest messages stand for them, and
                        // settling them would put the channel back at the pts they reach. An update for it inside a
                        // batch held for a seq gap is keyed by seq and stays.
                        let was_held = self.tracker.pending.len();
                        self.tracker.pending.retain(|(_, held)| held.key != Key::Channel(channel_id));
                        // What is dropped came during the run, so it is reported even for a channel whose report about
                        // what came before the run is suppressed. The range stays empty: the channel is forgotten, so
                        // the next update delivered for it builds on whatever pts it brings, not on the highest held.
                        if since.is_none() || self.tracker.pending.len() < was_held {
                            self.skipped(Key::Channel(channel_id), pts, pts, SkipReason::TooLong);
                        }
                        self.tracker.state.channels.remove(&channel_id);
                        return Ok(());
                    };
                    if to > pts && since.is_none() {
                        self.skipped(Key::Channel(channel_id), pts, to, SkipReason::TooLong);
                    }
                    pts = to;
                    break;
                }
                base::updates::ChannelDifference::ChannelDifference(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.new_messages.into_iter().filter(keep) {
                        self.tracker.emit(new_channel_message(message), &entities);
                        self.recovered.0 += 1;
                    }
                    for update in d.other_updates.into_iter().filter(keep_update) {
                        self.tracker.emit(update, &entities);
                        self.recovered.1 += 1;
                    }
                    pts = d.pts;
                    // Stored per page: a failure on a later page retries from here, not from page 1 again.
                    self.tracker.state.channels.insert(channel_id, pts);
                    if d.r#final {
                        break;
                    }
                }
            }
        }
        self.tracker.state.channels.insert(channel_id, pts);
        Ok(())
    }
}
