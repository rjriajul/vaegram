//! The `getUpdates` long poll that feeds the dispatcher of a client in Bot API mode while it runs.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::bridge::Transport;
use super::convert::{self, Peers, int};
use super::raw::types as bot;
use crate::Client;
use crate::errors::Error;
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
pub(crate) struct Polling {
    task: JoinHandle<()>,
    transport: Arc<Transport>,
}

impl Polling {
    /// What ended the poll for good, which `run` ends with; `None` while it polls, or when `stop` ended it.
    pub(crate) fn failure(&self) -> Option<Error> {
        self.transport.failed.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl Drop for Polling {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts polling for a client in Bot API mode, until `stop`; `None` for any other client. A poll Telegram refuses
/// for good leaves its error for [`Polling::failure`] and stops the client, so `run` ends once the handlers already
/// running are done, as after a `stop`.
pub(crate) fn poll(client: &Client) -> Option<Polling> {
    let transport = client.0.bot_api.clone()?;
    let stop = client.0.stop.subscribe();
    let catch_up = client.0.catch_up;
    // Weak, so the poll never keeps the client alive.
    let inner = Arc::downgrade(&client.0);
    let polled = transport.clone();
    let task = tokio::spawn(async move {
        tokio::select! {
            () = stopped(stop) => {}
            failed = take_updates(&polled, catch_up) => {
                let Some(error) = failed else { return };
                *polled.failed.lock().unwrap_or_else(PoisonError::into_inner) = Some(error);
                if let Some(inner) = inner.upgrade() {
                    inner.stop.send_replace(true);
                }
            }
        }
    });
    Some(Polling { task, transport })
}

async fn stopped(mut stop: watch::Receiver<bool>) {
    drop(stop.wait_for(|stopped| *stopped).await);
}

/// Polls until the dispatcher is gone, `None`, or Telegram refuses the bot for good, with that error. Without
/// catch-up, the first poll of a client asks for the newest update alone, which confirms every older one to
/// Telegram, and skips it too, as an MTProto bot starting without catch-up does.
async fn take_updates(transport: &Transport, catch_up: bool) -> Option<Error> {
    let fresh = transport.offset.load(Ordering::Acquire) == 0 && transport.handed.load(Ordering::Acquire) == 0;
    let mut skip = !catch_up && fresh;
    let mut failures = 0u32;
    loop {
        // A poll confirms to Telegram every update below its offset, so it first waits until the dispatcher took
        // every queued update. One still queued when `run` stops is taken by the next `run`, and confirmed then.
        if !drained(transport).await {
            return None;
        }
        transport.offset.fetch_max(transport.handed.load(Ordering::Acquire), Ordering::AcqRel);
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
            Err(error) if fatal(&error) => return Some(error),
            Err(error) => {
                backoff(&mut failures, error.flood_wait()).await;
                continue;
            }
        };
        failures = 0;
        if skip {
            skip = false;
            if let Some(last) = updates.last().and_then(update_id) {
                transport.handed.fetch_max(last.saturating_add(1), Ordering::AcqRel);
            }
            continue;
        }
        for value in updates {
            if !hand_on(transport, value).await {
                return None;
            }
        }
    }
}

/// Waits, woken by the queue itself, until the dispatcher took every queued update: all of the queue's room is
/// free again then. `false` once the dispatcher is gone.
async fn drained(transport: &Transport) -> bool {
    transport.updates.reserve_many(transport.updates.max_capacity()).await.is_ok()
}

/// Errors no retry gets past: a token that is wrong or revoked (401, 404), or a webhook or another poller taking
/// the updates (409).
fn fatal(error: &Error) -> bool {
    error.rpc().is_some_and(|e| matches!(e.code, 401 | 404 | 409))
}

/// How long to wait after `failures` failed polls in a row: as long as Telegram asked, or 1, 2, 4… seconds up to
/// [`MAX_BACKOFF`].
fn backoff_wait(failures: u32, flood_wait: Option<Duration>) -> Duration {
    flood_wait.unwrap_or_else(|| Duration::from_secs(1 << failures.min(5)).min(MAX_BACKOFF))
}

async fn backoff(failures: &mut u32, flood_wait: Option<Duration>) {
    let wait = backoff_wait(*failures, flood_wait);
    *failures = failures.saturating_add(1);
    tokio::time::sleep(wait).await;
}

fn update_id(value: &Value) -> Option<i64> {
    value.get("update_id")?.as_i64()
}

/// Queues one update for the dispatcher, as an MTProto connection would push it; `false` once the dispatcher is
/// gone. An update of a kind the bridge doesn't map, or one that doesn't parse, is confirmed with the others all
/// the same.
async fn hand_on(transport: &Transport, value: Value) -> bool {
    let Some(id) = update_id(&value) else { return true };
    let mut peers = Peers::new(transport.me());
    let parsed = serde_json::from_value::<bot::Update>(value).ok();
    let carried = parsed.as_ref().and_then(|u| {
        let message = u.message.as_deref().or(u.edited_message.as_deref());
        message.or(u.channel_post.as_deref()).or(u.edited_channel_post.as_deref())
    });
    // Kept before the update is queued, so the message the dispatcher builds finds the message it replies to.
    if let Some(message) = carried {
        transport.carry(message);
    }
    let Some(update) = parsed.and_then(|u| convert::update(u, &mut peers)) else {
        transport.handed.fetch_max(id.saturating_add(1), Ordering::AcqRel);
        return true;
    };
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
    // Nothing awaited since the send, so a poll aborted around here never queues an update it doesn't count.
    transport.handed.fetch_max(id.saturating_add(1), Ordering::AcqRel);
    true
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{backoff_wait, fatal};
    use crate::errors::{Error, RpcError};

    #[test]
    fn waits_double_up_to_a_cap_and_telegram_s_wait_wins() {
        let waits: Vec<u64> = (0..7).map(|failures| backoff_wait(failures, None).as_secs()).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 30, 30]);
        assert_eq!(backoff_wait(3, Some(Duration::from_secs(7))), Duration::from_secs(7));
    }

    #[test]
    fn only_a_refused_bot_ends_polling() {
        for (code, ends) in [(401, true), (404, true), (409, true), (400, false), (420, false), (500, false)] {
            assert_eq!(fatal(&Error::Rpc(RpcError::new(code, "X", None))), ends, "{code}");
        }
        assert!(!fatal(&Error::Timeout));
    }
}
