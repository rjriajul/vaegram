//! The `getUpdates` long poll that feeds the dispatcher of a client in Bot API mode while it runs.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::bridge::Transport;
use super::convert::{self, Peers, int};
use super::raw::types as bot;
use crate::Client;
use crate::raw::{base, types};
use crate::session::Incoming;
use crate::session::internals::now;

/// How long one `getUpdates` waits for an update, in seconds: under the HTTP client's 75-second timeout.
const POLL_TIMEOUT: i64 = 50;
/// The longest wait after failed polls in a row.
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// The kinds of update the bridge turns into TL ones; `getUpdates` leaves the others out.
const ALLOWED: [&str; 6] =
    ["message", "edited_message", "channel_post", "edited_channel_post", "callback_query", "inline_query"];

/// The poll one `run` drives; dropping it stops the poll.
pub(crate) struct Polling(JoinHandle<()>);

impl Drop for Polling {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Starts polling for a client in Bot API mode, until `stop`; `None` for any other client.
pub(crate) fn poll(client: &Client) -> Option<Polling> {
    let transport = client.0.bot_api.clone()?;
    let stop = client.0.stop.subscribe();
    let catch_up = client.0.catch_up;
    Some(Polling(tokio::spawn(async move {
        tokio::select! {
            () = stopped(stop) => {}
            () = take_updates(&transport, catch_up) => {}
        }
    })))
}

async fn stopped(mut stop: watch::Receiver<bool>) {
    drop(stop.wait_for(|stopped| *stopped).await);
}

/// Polls until the dispatcher is gone. Without catch-up, the first poll asks for the newest update alone, which
/// confirms every older one to Telegram, and skips it too, as an MTProto bot starting without catch-up does.
async fn take_updates(transport: &Transport, catch_up: bool) {
    let mut skip = !catch_up && transport.offset.load(Ordering::Acquire) == 0;
    let mut failures = 0u32;
    loop {
        let body = if skip {
            json!({ "offset": -1, "limit": 1, "timeout": 0 })
        } else {
            let offset = transport.offset.load(Ordering::Acquire);
            json!({ "offset": offset, "timeout": POLL_TIMEOUT, "allowed_updates": ALLOWED })
        };
        let updates = match transport.api.call("getUpdates", &body).await {
            Ok(Value::Array(updates)) => updates,
            Ok(_) => {
                backoff(&mut failures, None).await;
                continue;
            }
            Err(error) => {
                backoff(&mut failures, error.flood_wait()).await;
                continue;
            }
        };
        failures = 0;
        if skip {
            skip = false;
            if let Some(last) = updates.last().and_then(update_id) {
                transport.offset.fetch_max(last.saturating_add(1), Ordering::AcqRel);
            }
            continue;
        }
        for value in updates {
            if !hand_on(transport, value).await {
                return;
            }
        }
    }
}

/// Waits before the next poll: as long as Telegram asked, or 1, 2, 4… seconds up to [`MAX_BACKOFF`].
async fn backoff(failures: &mut u32, flood_wait: Option<Duration>) {
    let wait = flood_wait.unwrap_or_else(|| Duration::from_secs(1 << (*failures).min(5)).min(MAX_BACKOFF));
    *failures = failures.saturating_add(1);
    tokio::time::sleep(wait).await;
}

fn update_id(value: &Value) -> Option<i64> {
    value.get("update_id")?.as_i64()
}

/// Queues one update for the dispatcher, as an MTProto connection would push it; `false` once the dispatcher is
/// gone. An update of a kind the bridge doesn't map, or that doesn't parse, is skipped; the offset moves past it
/// either way, so it never comes again.
async fn hand_on(transport: &Transport, value: Value) -> bool {
    let Some(id) = update_id(&value) else { return true };
    let mut peers = Peers::new(transport.me());
    let update = serde_json::from_value::<bot::Update>(value).ok().and_then(|u| convert::update(u, &mut peers));
    if let Some(update) = update {
        let message = match &update {
            base::Update::UpdateNewMessage(u) => Some(&u.message),
            base::Update::UpdateNewChannelMessage(u) => Some(&u.message),
            base::Update::UpdateEditMessage(u) => Some(&u.message),
            base::Update::UpdateEditChannelMessage(u) => Some(&u.message),
            _ => None,
        };
        if let Some(base::Message::Message(message)) = message {
            transport.saw(message);
        }
        let date = int(now());
        let updates = types::Updates { updates: vec![update], users: peers.users, chats: peers.chats, date, seq: 0 };
        let incoming = Incoming { updates: base::Updates::Updates(Box::new(updates)), from_request: false };
        if transport.updates.send(incoming).await.is_err() {
            return false;
        }
    }
    transport.offset.fetch_max(id.saturating_add(1), Ordering::AcqRel);
    true
}
