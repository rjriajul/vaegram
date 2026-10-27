mod difference;
mod entities;
mod router;
mod tasks;
mod tracker;

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

pub(crate) use entities::Entities;
pub(crate) use router::{Router, Routes};
use tasks::Tasks;
use tracker::{Key, Tracker};

use crate::Client;
use crate::enums::ClientEvent;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::session::{Incoming, UpdatesRx};
use crate::storage::state::UpdateState;

const SAVE_EVERY: Duration = Duration::from_secs(10);

pub(crate) struct Dispatcher {
    client: Client,
    tracker: Tracker,
    saved_at: Instant,
    saved: (UpdateState, usize),
    router: Arc<Router>,
    generation: u64,
    tasks: Tasks,
}

impl Dispatcher {
    pub(crate) fn new(client: Client, state: &types::updates::State) -> Self {
        let mut tracker = Tracker::default();
        tracker.set_state(state);
        Dispatcher::with(client, tracker)
    }

    pub(crate) fn resume(client: Client, state: UpdateState) -> Self {
        Dispatcher::with(client, Tracker { state, due: vec![Key::Pts], ..Tracker::default() })
    }

    fn with(client: Client, mut tracker: Tracker) -> Self {
        tracker.touch(Instant::now());
        let generation = client.0.handlers.generation();
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(client.0.ordered, client.0.handler_permits.clone());
        tasks.on_error = router.on_error.clone();
        tasks.events = client.0.events.clone();
        Dispatcher { client, tracker, saved_at: Instant::now(), saved: Default::default(), router, generation, tasks }
    }

    pub(crate) async fn run(&mut self, updates: &mut UpdatesRx) {
        self.resolve().await;
        loop {
            for (update, entities) in std::mem::take(&mut self.tracker.ready) {
                self.dispatch(update, &entities);
            }
            // Saved every SAVE_EVERY when something moved, and on drop, so a crash replays at most that much.
            if self.saved_at.elapsed() >= SAVE_EVERY {
                self.saved_at = Instant::now();
                let now = (self.tracker.state.clone(), self.client.0.peers.lock().unwrap().len());
                if now != self.saved {
                    self.client.0.save_state_in_background(&now.0);
                    self.saved = now;
                }
            }
            let next = match self.tracker.deadline() {
                Some(deadline) => tokio::time::timeout_at(deadline, updates.recv()).await,
                None => Ok(updates.recv().await),
            };
            match next {
                Ok(Some(incoming)) => self.process(incoming),
                Ok(None) => return,
                Err(_) => self.tracker.expire(Instant::now()),
            }
            self.resolve().await;
        }
    }

    fn process(&mut self, incoming: Incoming) {
        let quiet = incoming.from_request;
        let t = &mut self.tracker;
        if !quiet {
            t.touch(Instant::now());
        }
        match incoming.updates {
            base::Updates::UpdatesTooLong => t.mark(Key::Pts),
            base::Updates::UpdateShortMessage(u) => t.short(u.pts, u.pts_count, quiet),
            base::Updates::UpdateShortChatMessage(u) => t.short(u.pts, u.pts_count, quiet),
            base::Updates::UpdateShortSentMessage(u) => t.short(u.pts, u.pts_count, true),
            base::Updates::UpdateShort(u) => t.update(u.update, &Arc::default(), quiet),
            base::Updates::Updates(u) => {
                let entities = Arc::new(Entities::new(&self.client, u.users, u.chats));
                t.batch(u.updates, entities, (u.date, u.seq, u.seq), quiet);
            }
            base::Updates::UpdatesCombined(u) => {
                let entities = Arc::new(Entities::new(&self.client, u.users, u.chats));
                t.batch(u.updates, entities, (u.date, u.seq_start, u.seq), quiet);
            }
        }
    }

    async fn resolve(&mut self) {
        while let Some(key) = self.tracker.due.pop() {
            let result = match key {
                Key::Channel(id) => self.channel_difference(id).await,
                _ => self.difference().await,
            };
            self.tracker.recover(&result, key);
            self.report(result);
        }
    }

    fn report(&self, result: Result<()>) {
        if let Err(error) = result {
            eprintln!("vaegram: failed to fetch missed updates: {error}");
            self.client.0.events.emit(ClientEvent::DifferenceFailed { error });
        }
    }

    fn dispatch(&mut self, update: base::Update, entities: &Entities) {
        let handlers = &self.client.0.handlers;
        let generation = handlers.generation();
        if generation != self.generation {
            (self.router, self.generation) = (handlers.current(), generation);
            self.tasks.on_error = self.router.on_error.clone();
        }
        self.router.dispatch(&self.client, update, entities, &mut self.tasks);
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        if let Ok(mut state) = self.client.0.state.lock() {
            *state = Some(std::mem::take(&mut self.tracker.state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientEvent, Dispatcher, types};
    use crate::Client;
    use crate::errors::Error;

    #[test]
    fn failed_differences_are_reported() {
        let client = Client::offline();
        let events = client.0.events.record();
        let state = types::updates::State { pts: 1, qts: 0, date: 0, seq: 0, unread_count: 0 };
        let d = Dispatcher::new(client, &state);
        d.report(Ok(()));
        d.report(Err(Error::Disconnected));
        let got: Vec<ClientEvent> = events.try_iter().collect();
        assert!(matches!(got[..], [ClientEvent::DifferenceFailed { error: Error::Disconnected }]), "{got:?}");
    }
}
