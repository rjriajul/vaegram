use std::sync::Arc;

use super::Dispatcher;
use super::entities::Entities;
use super::tracker::{Key, channel_of};
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
/// moves a counter, and it isn't dated `since` or later. Others, such as callback queries, are never held.
pub(super) fn held(update: &base::Update, since: i32) -> bool {
    let counted = update.pts().is_some() || update.qts().is_some() || channel_of(update).is_some();
    counted && !update_date(update).is_some_and(|date| date >= since)
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
            let since_run = |date: Option<i32>| held_before.is_none_or(|since| date.is_some_and(|date| date >= since));
            for message in messages.into_iter().filter(|m| since_run(message_date(m))) {
                t.emit(new_message(message), &entities);
                self.recovered.0 += 1;
            }
            for update in others.into_iter().filter(|u| since_run(update_date(u))) {
                self.recovered.1 += 1;
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
        let Some(&local) = self.tracker.state.channels.get(&channel_id) else { return Ok(()) };
        let mut pts = local;
        let channel = self.client.resolve_channel(&get_channel_id(channel_id).into()).await?;
        let bot = self.client.0.me.lock().unwrap().is_bot;
        let limit = if bot { BOT_CHANNEL_DIFFERENCE_LIMIT } else { CHANNEL_DIFFERENCE_LIMIT };
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
                    for message in d.messages {
                        self.tracker.emit(new_channel_message(message), &entities);
                        self.recovered.0 += 1;
                    }
                    if let base::Dialog::Dialog(dialog) = &d.dialog {
                        let to = dialog.pts.unwrap_or(pts);
                        if to > pts {
                            self.skipped(Key::Channel(channel_id), pts, to, SkipReason::TooLong);
                        }
                        pts = to;
                    }
                    break;
                }
                base::updates::ChannelDifference::ChannelDifference(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.new_messages {
                        self.tracker.emit(new_channel_message(message), &entities);
                        self.recovered.0 += 1;
                    }
                    for update in d.other_updates {
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
