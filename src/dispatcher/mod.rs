mod difference;
mod entities;
mod router;
mod tracker;

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

pub(crate) use entities::Entities;
pub(crate) use router::{Router, Routes};
use tracker::{Key, Tracker};

use crate::Client;
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
        Dispatcher { client, tracker, saved_at: Instant::now(), saved: Default::default(), router, generation }
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
            self.tracker.recover(result, key);
        }
    }

    fn dispatch(&mut self, update: base::Update, entities: &Entities) {
        let handlers = &self.client.0.handlers;
        let generation = handlers.generation();
        if generation != self.generation {
            (self.router, self.generation) = (handlers.current(), generation);
        }
        self.router.dispatch(&self.client, update, entities);
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        if let Ok(mut state) = self.client.0.state.lock() {
            *state = Some(std::mem::take(&mut self.tracker.state));
        }
    }
}
