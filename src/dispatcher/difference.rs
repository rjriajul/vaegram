use std::sync::Arc;

use super::Dispatcher;
use super::entities::Entities;
use super::tracker::channel_of;
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

impl Dispatcher {
    pub(super) async fn difference(&mut self) -> Result<()> {
        loop {
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
                    t.state.pts = d.pts;
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
            for message in messages {
                t.emit(new_message(message), &entities);
            }
            for update in others {
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

    pub(super) async fn channel_difference(&mut self, channel_id: i64) -> Result<()> {
        let Some(&local) = self.tracker.state.channels.get(&channel_id) else { return Ok(()) };
        let mut pts = local;
        let channel = self.client.resolve_channel(&get_channel_id(channel_id).into()).await?;
        let bot = self.client.0.me.lock().unwrap().is_bot;
        let limit = if bot { BOT_CHANNEL_DIFFERENCE_LIMIT } else { CHANNEL_DIFFERENCE_LIMIT };
        loop {
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
                    }
                    if let base::Dialog::Dialog(dialog) = &d.dialog {
                        pts = dialog.pts.unwrap_or(pts);
                    }
                    break;
                }
                base::updates::ChannelDifference::ChannelDifference(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.new_messages {
                        self.tracker.emit(new_channel_message(message), &entities);
                    }
                    for update in d.other_updates {
                        self.tracker.emit(update, &entities);
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
