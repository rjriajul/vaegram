use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use super::internals::data_center::Dc;
use super::{Events, Session, WeakSession, init_connection};
use crate::connection::Options;
use crate::crypto::rsa::PublicKey;
use crate::errors::{Error, Result};
use crate::raw::{RemoteCall, base, functions};

const IDLE: Duration = Duration::from_secs(15);
const PART_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TIMEOUTS: usize = 3;

// Keyed by address, so a DC's media-only address gets its own slot and auth key.
#[derive(Default)]
pub(crate) struct TransferPool(Mutex<HashMap<Dc, Arc<Slot>>>);

#[derive(Default)]
struct Slot {
    opening: tokio::sync::Mutex<()>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    auth_key: Option<[u8; 256]>,
    sessions: Vec<WeakSession>,
}

#[derive(Clone, Copy)]
pub(crate) enum Auth<'a> {
    Home(&'a Session, i32),
    Cdn(&'a PublicKey),
}

pub(crate) struct Lease {
    pub(crate) sessions: Vec<Session>,
}

impl TransferPool {
    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) fn forget(&self, dc: Dc) {
        self.0.lock().unwrap().remove(&dc);
    }

    #[cfg(test)]
    pub(crate) fn insert(&self, dc: Dc, session: &Session) {
        let state = State { auth_key: Some(*session.auth_key()), sessions: vec![session.downgrade()] };
        let slot = Slot { opening: Default::default(), state: Mutex::new(state) };
        self.0.lock().unwrap().insert(dc, Arc::new(slot));
    }

    pub(crate) async fn lease(
        &self,
        dc: Dc,
        count: usize,
        auth: Auth<'_>,
        options: &Options,
        events: &Arc<Events>,
    ) -> Result<Lease> {
        let slot = self.0.lock().unwrap().entry(dc).or_default().clone();
        let _opening = slot.opening.lock().await;
        let (mut sessions, key) = {
            let state = slot.state.lock().unwrap();
            let live: Vec<Session> = state.sessions.iter().filter_map(WeakSession::upgrade).collect();
            let home = match auth {
                Auth::Home(home, _) if home.dc_id() == dc.id => Some(*home.auth_key()),
                _ => None,
            };
            (live, home.or(state.auth_key))
        };
        let key = match key {
            Some(key) => key,
            None => {
                let cdn_key = match auth {
                    Auth::Cdn(key) => Some(key),
                    Auth::Home(..) => None,
                };
                let session = Session::connect_transfer(dc, None, cdn_key, options, events.clone()).await?;
                if let Auth::Home(home, api_id) = auth {
                    let base::auth::ExportedAuthorization::ExportedAuthorization(e) =
                        home.invoke(&functions::auth::ExportAuthorization { dc_id: dc.id }).await?;
                    let import = functions::auth::ImportAuthorization { id: e.id, bytes: e.bytes };
                    session.invoke(&init_connection(api_id, options, import)).await?;
                }
                let key = *session.auth_key();
                slot.state.lock().unwrap().auth_key = Some(key);
                sessions.push(session);
                key
            }
        };
        let missing = count.saturating_sub(sessions.len());
        let connect = |_| Session::connect_transfer(dc, Some(key), None, options, events.clone());
        let opened = try_join_all((0..missing).map(connect)).await?;
        sessions.extend(opened);
        slot.state.lock().unwrap().sessions = sessions.iter().map(Session::downgrade).collect();
        sessions.truncate(count);
        Ok(Lease { sessions })
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let sessions = std::mem::take(&mut self.sessions);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                tokio::time::sleep(IDLE).await;
                drop(sessions);
            });
        }
    }
}

pub(crate) async fn invoke_part<R: RemoteCall>(session: &Session, request: &R) -> Result<R::Return> {
    let mut timeouts = 0;
    loop {
        match session.invoke_with_timeout(request, PART_TIMEOUT).await {
            Err(Error::Rpc(e)) if e.code == 420 => {
                tokio::time::sleep(Duration::from_secs(e.value.unwrap_or(1).into())).await;
            }
            Err(Error::Timeout) if timeouts + 1 < MAX_TIMEOUTS => timeouts += 1,
            result => return result,
        }
    }
}

pub(crate) fn workers(parts: i32, part_size: u64, budget: u64) -> usize {
    ((budget / part_size) as usize).clamp(1, parts.max(1) as usize)
}

pub(crate) fn take(next: &AtomicI32, count: i32) -> Option<i32> {
    let part = next.fetch_add(1, Ordering::Relaxed);
    (part < count).then_some(part)
}

pub(crate) type Report<'a> = Box<dyn FnMut(u64, u64) + Send + 'a>;

// The callback runs under the lock, so `done` only grows even when parts finish out of order.
pub(crate) struct Progress<'a> {
    total: u64,
    state: Mutex<(u64, Option<Report<'a>>)>,
}

impl<'a> Progress<'a> {
    pub(crate) fn new(report: Option<Report<'a>>, total: u64) -> Self {
        Progress { total, state: Mutex::new((0, report)) }
    }

    pub(crate) fn add(&self, bytes: usize) {
        let (done, report) = &mut *self.state.lock().unwrap();
        *done += bytes as u64;
        if let Some(report) = report {
            report(*done, self.total);
        }
    }
}

pub(crate) async fn try_join_all<T, F: Future<Output = Result<T>>>(futures: impl Iterator<Item = F>) -> Result<Vec<T>> {
    let mut futures: Vec<_> = futures.map(|f| Some(Box::pin(f))).collect();
    let mut outputs: Vec<Option<T>> = futures.iter().map(|_| None).collect();
    poll_fn(|cx| {
        let mut pending = false;
        for (slot, output) in futures.iter_mut().zip(&mut outputs) {
            let Some(future) = slot else { continue };
            match future.as_mut().poll(cx) {
                Poll::Ready(Ok(value)) => (*output, *slot) = (Some(value), None),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => pending = true,
            }
        }
        if pending { Poll::Pending } else { Poll::Ready(Ok(())) }
    })
    .await?;
    Ok(outputs.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicI32;

    use super::{Progress, take, try_join_all, workers};
    use crate::errors::{Error, Result};

    #[test]
    fn caps_parts_in_flight_by_bytes() {
        let w = |parts, budget| workers(parts, 512 * 1024, budget);
        assert_eq!((w(1, 4 << 20), w(3, 4 << 20), w(100, 4 << 20), w(100, 2 << 20), w(0, 2 << 20)), (1, 3, 8, 4, 1));
    }

    #[test]
    fn reports_growing_progress_from_parallel_parts() {
        let mut seen = Vec::new();
        let progress = Progress::new(Some(Box::new(|done, total| seen.push((done, total)))), 2400);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| (0..100).for_each(|i| progress.add(1 + i % 5)));
            }
        });
        drop(progress);
        assert_eq!((seen.len(), seen.last().copied()), (800, Some((2400, 2400))));
        assert!(seen.windows(2).all(|w| w[0].0 < w[1].0 && w[1].1 == 2400));
    }

    #[test]
    fn hands_out_each_part_once() {
        let next = AtomicI32::new(0);
        let taken: Vec<_> = std::iter::from_fn(|| take(&next, 3)).collect();
        assert_eq!(taken, [0, 1, 2]);
        assert_eq!(take(&next, 3), None);
    }

    #[tokio::test]
    async fn joins_in_order_and_stops_at_the_first_error() {
        let next = AtomicI32::new(0);
        let worker = |id: i32| {
            let next = &next;
            async move {
                let mut mine = Vec::new();
                while let Some(part) = take(next, 10) {
                    mine.push(part);
                    tokio::task::yield_now().await;
                }
                Result::Ok((id, mine))
            }
        };
        let done = try_join_all((0..3).map(worker)).await.unwrap();
        assert_eq!(done.iter().map(|d| d.0).collect::<Vec<_>>(), [0, 1, 2]);
        let mut parts: Vec<_> = done.into_iter().flat_map(|d| d.1).collect();
        parts.sort();
        assert_eq!(parts, (0..10).collect::<Vec<_>>());

        let failing = [Err(Error::Timeout), Ok(1)].map(|r| async move { r });
        assert!(matches!(try_join_all(failing.into_iter()).await, Err(Error::Timeout)));
    }
}
