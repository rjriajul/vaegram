use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use super::internals::data_center::Dc;
use super::{Session, WeakSession, init_connection};
use crate::connection::Options;
use crate::errors::{Error, Result};
use crate::raw::{RemoteCall, base, functions};

const IDLE: Duration = Duration::from_secs(15);
const PART_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TIMEOUTS: usize = 3;

#[derive(Default)]
pub(crate) struct TransferPool(Mutex<HashMap<i32, Arc<Slot>>>);

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

pub(crate) struct Lease {
    pub(crate) sessions: Vec<Session>,
}

impl TransferPool {
    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) async fn lease(
        &self,
        dc: Dc,
        count: usize,
        home: &Session,
        api_id: i32,
        options: &Options,
    ) -> Result<Lease> {
        let slot = self.0.lock().unwrap().entry(dc.id).or_default().clone();
        let _opening = slot.opening.lock().await;
        let (mut sessions, key) = {
            let state = slot.state.lock().unwrap();
            let live: Vec<Session> = state.sessions.iter().filter_map(WeakSession::upgrade).collect();
            (live, if dc.id == home.dc_id() { Some(*home.auth_key()) } else { state.auth_key })
        };
        let key = match key {
            Some(key) => key,
            None => {
                let session = Session::connect_transfer(dc, None, options).await?;
                let base::auth::ExportedAuthorization::ExportedAuthorization(e) =
                    home.invoke(&functions::auth::ExportAuthorization { dc_id: dc.id }).await?;
                let import = functions::auth::ImportAuthorization { id: e.id, bytes: e.bytes };
                session.invoke(&init_connection(api_id, options, import)).await?;
                let key = *session.auth_key();
                slot.state.lock().unwrap().auth_key = Some(key);
                sessions.push(session);
                key
            }
        };
        let missing = count.saturating_sub(sessions.len());
        let opened = try_join_all((0..missing).map(|_| Session::connect_transfer(dc, Some(key), options))).await?;
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

    use super::{take, try_join_all, workers};
    use crate::errors::{Error, Result};

    #[test]
    fn caps_parts_in_flight_by_bytes() {
        let w = |parts, budget| workers(parts, 512 * 1024, budget);
        assert_eq!((w(1, 4 << 20), w(3, 4 << 20), w(100, 4 << 20), w(100, 2 << 20), w(0, 2 << 20)), (1, 3, 8, 4, 1));
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
