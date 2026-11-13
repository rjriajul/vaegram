use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use super::internals::data_center::Dc;
use super::{Events, Known, Salt, Session, WeakSession, init_connection};
use crate::client::quiet;
use crate::connection::Options;
use crate::crypto::rsa::PublicKey;
use crate::errors::{Error, Result};
use crate::raw::{RemoteCall, base, functions};

const IDLE: Duration = Duration::from_secs(15);
// The slowest link a transfer still survives, in bytes per second per connection.
const MIN_RATE: u64 = 8 << 10;
// The least a part gets: two 512 KiB parts at MIN_RATE, what an upload connection carries.
pub(crate) const MIN_PART_TIMEOUT: Duration = Duration::from_secs(2 * (512 << 10) / MIN_RATE);
const MAX_TIMEOUTS: usize = 3;
// Server errors a part waits out, a second for the first and twice as long for each after it: a minute in all, about
// the budget TDLib and teleproto give a file part.
const MAX_SERVER_ERRORS: u32 = 6;
const FIRST_SERVER_ERROR_WAIT: Duration = Duration::from_secs(1);
// A CDN gets at most this per wait, so one that keeps flooding is soon given up on and the master DC serves the part.
const MAX_CDN_FLOOD_WAIT: Duration = Duration::from_secs(60);

// Telegram shares a DC's auth key between its main and media-only addresses, so the key is kept per DC and the
// connections per address.
#[derive(Default)]
pub(crate) struct TransferPool {
    slots: Mutex<HashMap<(i32, bool), Arc<Slot>>>,
    // Goes up on each `close_all`, so a lease still opening connections knows to close them.
    epoch: AtomicU64,
    // Set by `shut`: no lease opens anything any more.
    shut: AtomicBool,
}

#[derive(Default)]
struct Slot {
    opening: tokio::sync::Mutex<()>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    auth_key: Option<[u8; 256]>,
    key_made: Option<tokio::time::Instant>,
    // The salt a connection on this key last had, so a new one doesn't start from 0 and bounce its first parts.
    salt: Salt,
    sessions: HashMap<Dc, Vec<WeakSession>>,
    // Numbers handed out so far, so an observer can tell this DC's connections apart.
    opened: u32,
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
        self.slots.lock().unwrap().clear();
    }

    pub(crate) fn forget(&self, dc: Dc) {
        self.slots.lock().unwrap().remove(&(dc.id, dc.test));
    }

    /// Closes every file connection, also the ones a lease is still opening, and forgets the keys, so the next
    /// lease starts over.
    pub(crate) fn close_all(&self) {
        // Bumped under the map lock, so a lease either gets a slot this drains or sees the new epoch.
        let slots: Vec<Arc<Slot>> = {
            let mut map = self.slots.lock().unwrap();
            self.epoch.fetch_add(1, Ordering::AcqRel);
            map.drain().map(|(_, slot)| slot).collect()
        };
        for slot in slots {
            let state = slot.state.lock().unwrap();
            state.sessions.values().flatten().filter_map(WeakSession::upgrade).for_each(|s| s.close());
        }
    }

    /// `close_all` for good: later leases fail with `Error::Disconnected`.
    pub(crate) fn shut(&self) {
        self.shut.store(true, Ordering::Release);
        self.close_all();
    }

    #[cfg(test)]
    pub(crate) fn insert(&self, dc: Dc, session: &Session) {
        let slot = self.slots.lock().unwrap().entry((dc.id, dc.test)).or_default().clone();
        let mut state = slot.state.lock().unwrap();
        (state.auth_key, state.key_made, state.salt) = (Some(*session.auth_key()), session.key_made, session.salt());
        state.sessions.entry(dc).or_default().push(session.downgrade());
    }

    pub(crate) async fn lease(
        &self,
        dc: Dc,
        count: usize,
        auth: Auth<'_>,
        options: &Options,
        events: &Arc<Events>,
    ) -> Result<Lease> {
        self.lease_keyed_at(dc, dc, count, auth, options, events).await
    }

    /// [`lease`](Self::lease) of connections to `dc` that, when the DC has no key yet, makes the permanent key and
    /// imports the authorization on `main`, the DC's main address, as tdlib and mtcute do: a media-only address only
    /// serves file requests, and under PFS each of its connections makes and binds a temporary key of its own.
    pub(crate) async fn lease_keyed_at(
        &self,
        main: Dc,
        dc: Dc,
        count: usize,
        auth: Auth<'_>,
        options: &Options,
        events: &Arc<Events>,
    ) -> Result<Lease> {
        // CDN DCs keep no permanent key to bind a temporary one to.
        let rebind = || match auth {
            Auth::Home(home, _) => home.pfs.clone(),
            Auth::Cdn(_) => None,
        };
        if self.shut.load(Ordering::Acquire) {
            return Err(Error::Disconnected);
        }
        let (epoch, slot) = {
            let mut map = self.slots.lock().unwrap();
            (self.epoch.load(Ordering::Acquire), map.entry((dc.id, dc.test)).or_default().clone())
        };
        let _opening = slot.opening.lock().await;
        let number = || {
            let mut state = slot.state.lock().unwrap();
            state.opened += 1;
            state.opened
        };
        let (mut sessions, key) = {
            let state = slot.state.lock().unwrap();
            let live: Vec<Session> =
                state.sessions.get(&dc).into_iter().flatten().filter_map(WeakSession::upgrade).collect();
            let home = match auth {
                Auth::Home(home, _) if home.dc_id() == dc.id => Some(home.known_key()),
                _ => None,
            };
            let salt = live.first().map_or(state.salt, Session::salt);
            (live, home.or(state.auth_key.map(|key| Known { key, salt, made: state.key_made })))
        };
        // The connection the key was made on, when that was the main address rather than `dc`.
        let mut keyed = None;
        let key = match key {
            Some(key) => key,
            None => {
                let cdn_key = match auth {
                    Auth::Cdn(key) => Some(key),
                    Auth::Home(..) => None,
                };
                let session =
                    Session::connect_transfer(main, None, cdn_key, options, events.clone(), rebind(), number()).await?;
                if let Auth::Home(home, api_id) = auth {
                    let export = functions::auth::ExportAuthorization { dc_id: dc.id };
                    // Same as everything else this client sends, when it wants no updates.
                    let base::auth::ExportedAuthorization::ExportedAuthorization(e) = match options.no_updates {
                        true => home.invoke(&quiet(export)).await?,
                        false => home.invoke(&export).await?,
                    };
                    let import = functions::auth::ImportAuthorization { id: e.id, bytes: e.bytes };
                    match options.no_updates {
                        true => session.invoke(&init_connection(api_id, options, quiet(import))).await?,
                        false => session.invoke(&init_connection(api_id, options, import)).await?,
                    };
                }
                let key = session.known_key();
                let mut state = slot.state.lock().unwrap();
                (state.auth_key, state.key_made) = (Some(key.key), key.made);
                drop(state);
                match main == dc {
                    true => sessions.push(session),
                    false => keyed = Some(session),
                }
                key
            }
        };
        let missing = count.saturating_sub(sessions.len());
        let connect = |_| {
            let opening = Session::connect_transfer(dc, Some(key), None, options, events.clone(), rebind(), number());
            async move { Ok(opening.await) }
        };
        let mut failed = None;
        for opened in try_join_all((0..missing).map(connect)).await? {
            match opened {
                Ok(session) => sessions.push(session),
                Err(e) => failed = Some(e),
            }
        }
        // Fewer connections than asked for still carry every part: the caller spreads its workers over the ones there
        // are. With none at all there is nothing to carry them.
        if let Some(e) = failed.filter(|_| sessions.is_empty()) {
            return Err(e);
        }
        let mut state = slot.state.lock().unwrap();
        // Checked under the lock close_all takes after its bump: either it sees these sessions, or this sees the bump.
        if self.shut.load(Ordering::Acquire) || self.epoch.load(Ordering::Acquire) != epoch {
            drop(state);
            sessions.iter().chain(&keyed).for_each(Session::close);
            return Err(Error::Disconnected);
        }
        state.salt = sessions.first().map_or(key.salt, Session::salt);
        state.sessions.insert(dc, sessions.iter().map(Session::downgrade).collect());
        if let Some(keyed) = &keyed {
            state.sessions.entry(main).or_default().push(keyed.downgrade());
        }
        drop(state);
        // Released like a lease: it idles on the main address a while, for a lease there to pick up.
        drop(keyed.map(|session| Lease { sessions: vec![session] }));
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

/// How long a part may take on a connection that has `in_flight` bytes of parts asked for at once: all of them at
/// MIN_RATE, as this one may come last, and never under MIN_PART_TIMEOUT.
pub(crate) fn part_timeout(in_flight: u64) -> Duration {
    MIN_PART_TIMEOUT.max(Duration::from_secs(in_flight / MIN_RATE))
}

pub(crate) async fn invoke_part<R: RemoteCall>(session: &Session, request: &R, timeout: Duration) -> Result<R::Return> {
    invoke_part_on(session, request, timeout, false).await
}

pub(crate) async fn invoke_cdn_part<R: RemoteCall>(
    session: &Session,
    request: &R,
    timeout: Duration,
) -> Result<R::Return> {
    invoke_part_on(session, request, timeout, true).await
}

// One part that gives up fails a transfer of thousands, so besides what the retry policy asks for, timeouts and
// server errors get bounded retries of their own. A CDN part that gives up only hands its download to the master DC,
// so it gets the policy's usual tries and no server error retries. A pause shared by a DC's parts would go before
// each send here.
async fn invoke_part_on<R: RemoteCall>(
    session: &Session,
    request: &R,
    timeout: Duration,
    cdn: bool,
) -> Result<R::Return> {
    // Parts go straight to their session, so the user's request_timeout still has to count.
    let timeout = timeout.max(session.options().request_timeout);
    let cap = cdn.then_some(MAX_CDN_FLOOD_WAIT);
    let (mut timeouts, mut server_errors, mut retries, mut slept) = (0, 0, 0u32, Duration::ZERO);
    loop {
        let result = session.invoke_with_timeout(request, timeout).await;
        let Err(e) = &result else { return result };
        let wait = match session.should_retry(R::NAME, e, retries + 1, slept, cap, !cdn) {
            ControlFlow::Continue(wait) => {
                retries += 1;
                wait
            }
            ControlFlow::Break(()) if matches!(e, Error::Timeout) && timeouts + 1 < MAX_TIMEOUTS => {
                timeouts += 1;
                // The part most likely went down with a connection the server stopped answering on: open a fresh
                // one, which sends the other parts waiting on it again too, as TDLib and mtcute do.
                session.restart()?;
                Duration::ZERO
            }
            // As TDLib, mtcute, teleproto and gogram send a part again after a pause: Telegram's trouble, not the
            // request's.
            ControlFlow::Break(()) if !cdn && is_server_error(e) && server_errors < MAX_SERVER_ERRORS => {
                server_errors += 1;
                FIRST_SERVER_ERROR_WAIT * 2u32.pow(server_errors - 1)
            }
            ControlFlow::Break(()) => return result,
        };
        slept += wait;
        tokio::time::sleep(wait).await;
    }
}

// A 500 such as INTERNAL or WORKER_BUSY_TOO_LONG_RETRY, or a negative code such as -503 Timeout.
fn is_server_error(e: &Error) -> bool {
    matches!(e, Error::Rpc(e) if e.code >= 500 || e.code < 0 || e.is("WORKER_BUSY_TOO_LONG_RETRY"))
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
    use std::ops::ControlFlow;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::time::Duration;

    use super::{
        Auth, MAX_CDN_FLOOD_WAIT, MIN_PART_TIMEOUT, Progress, Session, TransferPool, invoke_cdn_part, invoke_part,
        part_timeout, take, try_join_all, workers,
    };
    use crate::Client;
    use crate::connection::{Options, Transport};
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::session::internals::data_center::Dc;
    use crate::session::test_dc::TestDc;
    use crate::session::{AutoSleep, Request, Retry, RetryContext, RetryPolicy};

    fn part() -> functions::upload::SaveFilePart {
        functions::upload::SaveFilePart { file_id: 1, file_part: 0, bytes: vec![0; 4] }
    }

    fn rpc_error(code: i32, message: &str) -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError::parse(code, message)))
    }

    fn count(sent: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> usize {
        std::iter::from_fn(|| sent.try_recv().ok()).count()
    }

    #[tokio::test]
    async fn a_dcs_media_address_reuses_the_key_of_its_main_address() {
        let client = Client::offline();
        // Nothing listens on these ports, so a lease that tried to open a connection would fail.
        let dc = |port| Dc { id: 4, addr: ([127, 0, 0, 1], port).into(), test: false, media: false };
        let (foreign, _sent) = Session::scripted(Vec::new());
        let pool = TransferPool::default();
        pool.insert(dc(1), &foreign);
        let home = client.session();
        let lease = pool.lease(dc(2), 0, Auth::Home(&home, 1), &client.0.options, &client.0.events).await;
        assert!(lease.is_ok_and(|l| l.sessions.is_empty()));
    }

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
    fn a_part_gets_the_time_for_every_byte_its_connection_has_asked_for() {
        // Two 512 KiB parts at 8 KiB/s at least: an upload connection's two, or a download's one.
        let least = Duration::from_secs(128);
        assert_eq!([MIN_PART_TIMEOUT, part_timeout(512 << 10), part_timeout(1 << 20)], [least; 3]);
        // Four 512 KiB chunks on one connection, as Premium downloads put there.
        assert_eq!(part_timeout(2 << 20), Duration::from_secs(256));
    }

    #[tokio::test(start_paused = true)]
    async fn a_flood_wait_over_the_threshold_comes_back_without_sleeping() {
        let (mut session, mut sent) = Session::scripted(vec![rpc_error(420, "FLOOD_WAIT_120")]);
        let retry = Retry::new(AutoSleep::new().threshold(Duration::from_secs(60)));
        session.set_options(Options::new().retry(retry));

        let start = tokio::time::Instant::now();
        let e = invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap_err();
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert!(matches!(e, Error::Rpc(e) if (e.name.as_str(), e.value) == ("FLOOD_WAIT", Some(120))));
        assert_eq!(count(&mut sent), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn parts_follow_the_flood_sleep_setting_four_times_over() {
        let (mut session, mut sent) = Session::scripted((0..5).map(|_| rpc_error(420, "FLOOD_WAIT_1")).collect());
        session.set_options(Options::new().retry(Retry::new(AutoSleep::new().tries(1))));
        let e = invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap_err();
        assert!(matches!(e, Error::Rpc(e) if e.is("FLOOD_WAIT")));
        assert_eq!(count(&mut sent), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn a_part_outlasts_the_floods_that_would_fail_a_request() {
        let mut replies: Vec<_> = (0..6).map(|_| rpc_error(420, "FLOOD_WAIT_2")).collect();
        replies.push(Ok(true.to_bytes()));
        let (session, mut sent) = Session::scripted(replies);
        assert!(invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap());
        assert_eq!(count(&mut sent), 7);
    }

    #[tokio::test(start_paused = true)]
    async fn server_errors_are_waited_out_a_growing_while() {
        let replies = vec![
            rpc_error(500, "INTERNAL"),
            rpc_error(-503, "Timeout"),
            rpc_error(500, "WORKER_BUSY_TOO_LONG_RETRY"),
            Ok(true.to_bytes()),
        ];
        let (session, mut sent) = Session::scripted(replies);
        let start = tokio::time::Instant::now();
        assert!(invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap());
        assert_eq!((start.elapsed(), count(&mut sent)), (Duration::from_secs(1 + 2 + 4), 4));
    }

    #[tokio::test(start_paused = true)]
    async fn server_errors_are_given_up_on_after_a_minute() {
        let (session, mut sent) = Session::scripted((0..7).map(|_| rpc_error(500, "INTERNAL")).collect());
        let start = tokio::time::Instant::now();
        let e = invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap_err();
        assert!(matches!(e, Error::Rpc(e) if e.code == 500));
        assert_eq!((start.elapsed(), count(&mut sent)), (Duration::from_secs(1 + 2 + 4 + 8 + 16 + 32), 7));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cdn_part_hands_a_server_error_over_at_once() {
        let (session, mut sent) = Session::scripted(vec![rpc_error(500, "INTERNAL")]);
        assert!(matches!(invoke_cdn_part(&session, &part(), MIN_PART_TIMEOUT).await, Err(Error::Rpc(_))));
        assert_eq!(count(&mut sent), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_part_that_times_out_restarts_its_connection_and_goes_again() {
        // Never answers the first request, as a connection the server went quiet on; answers the rest.
        let (requests, mut rx) = tokio::sync::mpsc::unbounded_channel::<Request>();
        tokio::spawn(async move {
            let Some(lost) = rx.recv().await else { return };
            while let Some(request) = rx.recv().await {
                let _ = request.answer.send(Ok(true.to_bytes()));
            }
            drop(lost);
        });
        let session = Session::local(requests);
        let start = tokio::time::Instant::now();
        assert!(invoke_part(&session, &part(), MIN_PART_TIMEOUT).await.unwrap());
        assert!((MIN_PART_TIMEOUT..MIN_PART_TIMEOUT + Duration::from_secs(1)).contains(&start.elapsed()));
        assert!(session.control.restart.load(Ordering::Acquire), "the part went again on the same connection");
    }

    type Asked = (&'static str, u32, Option<Duration>, bool);
    struct Record(Arc<Mutex<Vec<Asked>>>);

    impl RetryPolicy for Record {
        fn should_retry(&self, ctx: &RetryContext<'_>) -> ControlFlow<(), Duration> {
            self.0.lock().unwrap().push((ctx.request, ctx.fail_count, ctx.flood_wait, ctx.file_part));
            ControlFlow::Break(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cdn_floods_reach_the_policy_capped_and_not_as_file_parts() {
        let seen = Arc::default();
        let flood = || rpc_error(420, "FLOOD_WAIT_300");
        let (mut session, _sent) = Session::scripted(vec![flood(), flood()]);
        session.set_options(Options::new().retry(Retry::new(Record(Arc::clone(&seen)))));
        let _ = invoke_part(&session, &part(), MIN_PART_TIMEOUT).await;
        let _ = invoke_cdn_part(&session, &part(), MIN_PART_TIMEOUT).await;
        let name = "upload.saveFilePart";
        let expected = [(name, 1, Some(Duration::from_secs(300)), true), (name, 1, Some(MAX_CDN_FLOOD_WAIT), false)];
        assert_eq!(*seen.lock().unwrap(), expected);
    }

    #[tokio::test]
    async fn a_lease_goes_on_with_the_connections_that_opened() {
        let client = Client::offline();
        // Nothing listens here, so every new connection fails: the one already open has to carry the lease.
        let dc = Dc { id: 4, addr: ([127, 0, 0, 1], 1).into(), test: false, media: false };
        let (open, _sent) = Session::scripted(Vec::new());
        let pool = TransferPool::default();
        pool.insert(dc, &open);
        let home = client.session();
        let lease = pool.lease(dc, 3, Auth::Home(&home, 1), &client.0.options, &client.0.events).await.unwrap();
        assert_eq!(lease.sessions.len(), 1);
    }

    #[tokio::test]
    async fn a_key_exchange_on_a_media_address_negates_the_dc() {
        let dc = TestDc::start([5; 256], false).await;
        let options = Options::new().transport(Transport::Intermediate);
        for (n, media) in [(1, dc.dc(4).media(true)), (2, dc.dc(4).test(true).media(true))] {
            Session::connect_transfer(media, None, None, &options, Arc::default(), None, n).await.unwrap();
        }
        assert_eq!(dc.handshakes(), [(1, -4, false), (2, -10004, false)]);
    }

    #[tokio::test]
    async fn a_media_lease_makes_the_key_on_the_main_address_and_temporary_keys_on_its_own() {
        use crate::session::test_dc::IMPORT_AUTHORIZATION;
        let dc = TestDc::start([5; 256], false).await;
        let export = types::auth::ExportedAuthorization { id: 1, bytes: vec![1] };
        let export = base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(export)).to_bytes();
        let (home, _sent) = Session::scripted(vec![Ok(export)]);
        let init = crate::session::init_connection(1, &Options::default(), functions::help::GetNearestDc {});
        let home = Session { pfs: Some(Arc::from(init.to_bytes())), ..home };
        let options = Options::new().transport(Transport::Intermediate);
        let pool = TransferPool::default();
        let (main, media) = (dc.dc(4), dc.dc(4).media(true));
        let lease = pool.lease_keyed_at(main, media, 2, Auth::Home(&home, 1), &options, &Arc::default()).await.unwrap();
        for session in &lease.sessions {
            session.invoke(&functions::Ping { ping_id: 1 }).await.unwrap();
        }
        // Connection 1 is on the main address: the permanent key, the import and its own temporary key. The two
        // media connections only make temporary keys, with the DC's id negated.
        let mut handshakes = dc.handshakes();
        handshakes.sort();
        assert_eq!(handshakes, [(1, 4, false), (1, 4, true), (2, -4, true), (3, -4, true)]);
        let imports = dc.seen().into_iter().filter(|s| s.ids.contains(&IMPORT_AUTHORIZATION)).map(|s| s.connection);
        assert_eq!(imports.collect::<Vec<_>>(), [1]);
        assert_eq!(lease.sessions.len(), 2);
    }

    #[tokio::test]
    async fn close_all_fails_leased_connections_and_empties_the_pool() {
        let dc = TestDc::start([5; 256], false).await;
        let export = types::auth::ExportedAuthorization { id: 1, bytes: vec![1] };
        let export = base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(export)).to_bytes();
        let (home, _sent) = Session::scripted(vec![Ok(export)]);
        let options = Options::new().transport(Transport::Intermediate);
        let pool = TransferPool::default();
        let lease = pool.lease(dc.dc(4), 1, Auth::Home(&home, 1), &options, &Arc::default()).await.unwrap();
        let ping = functions::Ping { ping_id: 1 };
        assert!(lease.sessions[0].invoke(&ping).await.is_ok());

        pool.close_all();
        assert!(matches!(lease.sessions[0].invoke(&ping).await, Err(Error::Disconnected)));
        assert!(pool.slots.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_lease_opening_during_close_all_is_closed_too() {
        let client = Client::offline();
        // Nothing listens here: the lease must get by on the session it already has.
        let dc = Dc { id: 4, addr: ([127, 0, 0, 1], 1).into(), test: false, media: false };
        let (foreign, _sent) = Session::scripted(Vec::new());
        let pool = Arc::new(TransferPool::default());
        pool.insert(dc, &foreign);
        let slot = pool.slots.lock().unwrap()[&(4, false)].clone();
        let opening = slot.opening.lock().await;
        let lease = tokio::spawn({
            let (pool, home, options, events) =
                (pool.clone(), client.session(), client.0.options.clone(), client.0.events.clone());
            async move { pool.lease(dc, 1, Auth::Home(&home, 1), &options, &events).await.map(|l| l.sessions.len()) }
        });
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        pool.close_all();
        drop(opening);
        assert!(matches!(lease.await.unwrap(), Err(Error::Disconnected)));
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
