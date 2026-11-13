use std::any::Any;
use std::ops::ControlFlow;
use std::time::Duration;

use crate::Client;
use crate::client::{Borrowed, quiet};
use crate::dispatcher::cache::BusinessHome;
use crate::errors::{Error, Result, RpcError};
use crate::raw::{RemoteCall, base, functions};
use crate::session::no_updates::needs_updates;
use crate::session::pacer::Pacer;
use crate::session::transfer::invoke_paced_part;
use crate::session::{RateCategory, Session, Tally};

const MAX_MIGRATIONS: usize = 3;

impl Client {
    /// Sends a raw request and returns its decoded answer.
    ///
    /// Uses the client's `request_timeout`. The request waits on the rate limiter and follows the retry policy
    /// described at [`invoke_with_timeout`](Client::invoke_with_timeout).
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.invoke_with_timeout(request, self.session().options().request_timeout).await
    }

    /// Sends a raw request and gives up on it after `timeout`.
    ///
    /// A request that times out is forgotten: the server is asked not to send its answer, and the connection
    /// carries on. After a failure the client's retry policy decides whether the request goes out again, each try
    /// with the full `timeout`: by default FLOOD_WAITs up to a minute are slept off, five times, and the next
    /// one goes to the caller. `tokio::time::timeout` around the call bounds the whole wait.
    pub async fn invoke_with_timeout<R: RemoteCall>(&self, request: &R, timeout: Duration) -> Result<R::Return> {
        self.invoke_as(R::NAME, request, timeout).await
    }

    /// `invoke_with_timeout` for a request that goes out inside a wrapper: `name` is the request the rate limiter
    /// counts it as, and the one that decides whether updates are off for it.
    pub(crate) async fn invoke_as<R: RemoteCall>(
        &self,
        name: &'static str,
        request: &R,
        timeout: Duration,
    ) -> Result<R::Return> {
        self.invoke_wrapped(name, request, timeout, self.quiet_for(name)).await
    }

    /// Whether updates are off for the request called `name`: always with `no_updates`, and for the ones that only
    /// read with `auto_no_updates`.
    fn quiet_for(&self, name: &str) -> bool {
        self.0.options.no_updates || (self.0.auto_no_updates && !needs_updates(name))
    }

    /// Sends `request` as `name`, inside `invokeWithoutUpdates` if `off`. Takeout goes outside, updates off inside.
    async fn invoke_wrapped<R: RemoteCall>(
        &self,
        name: &'static str,
        request: &R,
        timeout: Duration,
        off: bool,
    ) -> Result<R::Return> {
        self.limit(name).await;
        let session = self.session();
        let call = Borrowed(request);
        match (self.1, off) {
            (Some(takeout_id), true) => {
                let request = functions::InvokeWithTakeout { takeout_id, query: quiet(call) };
                invoke_sleeping_floods(&session, name, &request, timeout).await
            }
            (Some(takeout_id), false) => {
                let request = functions::InvokeWithTakeout { takeout_id, query: call };
                invoke_sleeping_floods(&session, name, &request, timeout).await
            }
            (None, true) => invoke_sleeping_floods(&session, name, &quiet(call), timeout).await,
            (None, false) => invoke_sleeping_floods(&session, name, request, timeout).await,
        }
    }

    /// Waits for the rate limiter's go-ahead, if there is one. It comes before the request's own timeout starts.
    async fn limit(&self, name: &str) {
        if let Some(limiter) = &self.0.limiter {
            limiter.acquire(RateCategory::of(name)).await;
        }
    }

    /// Sends `request` to data center `dc_id` under this login, which is exported there the first time. The home
    /// DC takes a plain `invoke`. An unknown one fails with `Error::InvalidDc` before anything is sent. Updates in
    /// the answer aren't dispatched. Calls in flight end with `Error::Disconnected` on `stop_transmission` or
    /// `disconnect`, which close the connections they share with file transfers.
    pub async fn invoke_in_dc<R: RemoteCall>(&self, dc_id: i32, request: &R) -> Result<R::Return> {
        if dc_id == self.session().dc_id() {
            return self.invoke(request).await;
        }
        self.invoke_away(dc_id, R::NAME, request, self.quiet_for(R::NAME)).await
    }

    /// `invoke_in_dc` for a DC that isn't home, counted as `name`, inside `invokeWithoutUpdates` if `off`.
    async fn invoke_away<R: RemoteCall>(
        &self,
        dc_id: i32,
        name: &'static str,
        request: &R,
        off: bool,
    ) -> Result<R::Return> {
        self.limit(name).await;
        let dc = self.dc(dc_id)?;
        let call = Borrowed(request);
        match (self.1, off) {
            (Some(takeout_id), true) => {
                let request = functions::InvokeWithTakeout { takeout_id, query: quiet(call) };
                self.invoke_on_file_connection(dc, name, &request).await
            }
            (Some(takeout_id), false) => {
                let request = functions::InvokeWithTakeout { takeout_id, query: call };
                self.invoke_on_file_connection(dc, name, &request).await
            }
            (None, true) => self.invoke_on_file_connection(dc, name, &quiet(call)).await,
            (None, false) => self.invoke_on_file_connection(dc, name, request).await,
        }
    }

    /// A file part on a transfer connection, inside this client's takeout session if it has one; see
    /// `part_timeout` for `timeout`, and `invoke_paced_part` for `pace`.
    pub(crate) async fn invoke_file_part<R: RemoteCall>(
        &self,
        session: &Session,
        request: &R,
        timeout: Duration,
        pace: Option<(&Pacer, u64)>,
    ) -> Result<R::Return> {
        match self.1 {
            Some(takeout_id) => {
                let request = functions::InvokeWithTakeout { takeout_id, query: Borrowed(request) };
                invoke_paced_part(session, &request, timeout, pace).await
            }
            None => invoke_paced_part(session, request, timeout, pace).await,
        }
    }

    /// Sends `request` through the business connection `connection_id`, or plainly without one, and hands the
    /// request back: a short "sent" update only carries what Telegram changed, the rest comes from it.
    ///
    /// Telegram serves a connection on the data center its `dc_id` names, which needn't be this client's. It's
    /// remembered from `updateBotBusinessConnect` and fetched once when unknown. A `*_MIGRATE_<dc>` answer moves the
    /// connection, not the client, and the request is sent once more there.
    pub(crate) async fn invoke_business<R: RemoteCall>(
        &self,
        connection_id: Option<&str>,
        request: R,
    ) -> Result<(R::Return, R)>
    where
        R::Return: 'static,
    {
        let Some(connection_id) = connection_id else { return Ok((self.invoke(&request).await?, request)) };
        let mut dc_id = self.business_home(connection_id).await?.dc_id;
        // Counted as the request inside, not as the wrapper, and updates go off inside it.
        let off = self.quiet_for(R::NAME);
        let mut moved = false;
        loop {
            let result = if off {
                let call = functions::InvokeWithBusinessConnection {
                    connection_id: connection_id.into(),
                    query: quiet(Borrowed(&request)),
                };
                self.invoke_business_in(dc_id, R::NAME, &call).await
            } else {
                let call = functions::InvokeWithBusinessConnection {
                    connection_id: connection_id.into(),
                    query: Borrowed(&request),
                };
                self.invoke_business_in(dc_id, R::NAME, &call).await
            };
            let migrate = result.as_ref().err().and_then(Error::rpc).and_then(RpcError::migrate_dc);
            match migrate {
                Some(to) if !moved && to != dc_id => {
                    self.0.caches.move_business(connection_id, to);
                    (dc_id, moved) = (to, true);
                }
                _ => {
                    let mut answer = result?;
                    name_connection(&mut answer, connection_id);
                    return Ok((answer, request));
                }
            }
        }
    }

    async fn invoke_business_in<R: RemoteCall>(&self, dc_id: i32, name: &'static str, call: &R) -> Result<R::Return> {
        if dc_id == self.session().dc_id() {
            self.invoke_wrapped(name, call, self.session().options().request_timeout, false).await
        } else {
            self.invoke_away(dc_id, name, call, false).await
        }
    }

    /// The account and data center of the business connection `connection_id`, fetched once if not seen yet.
    pub(crate) async fn business_home(&self, connection_id: &str) -> Result<BusinessHome> {
        match self.0.caches.business_home(connection_id) {
            Some(home) => Ok(home),
            None => {
                let connection = self.get_business_connection(connection_id).await?;
                Ok(BusinessHome { user_id: connection.user.id, dc_id: connection.dc_id })
            }
        }
    }

    pub(crate) async fn invoke_migrating<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        let mut migrations = 0;
        loop {
            let result = self.invoke(request).await;
            match result.as_ref().err().and_then(Error::rpc).and_then(RpcError::migrate_dc) {
                Some(dc_id) if migrations < MAX_MIGRATIONS => {
                    self.switch_dc(dc_id).await?;
                    migrations += 1;
                }
                _ => return result,
            }
        }
    }
}

/// Telegram answers a request sent through a business connection with business updates whose connection id is
/// empty; they get the one the request went through, so the messages parsed from them carry it.
fn name_connection(answer: &mut dyn Any, connection_id: &str) {
    let Some(updates) = answer.downcast_mut::<base::Updates>() else { return };
    let updates = match updates {
        base::Updates::Updates(u) => u.updates.as_mut_slice(),
        base::Updates::UpdatesCombined(u) => u.updates.as_mut_slice(),
        base::Updates::UpdateShort(u) => std::slice::from_mut(&mut u.update),
        _ => return,
    };
    for update in updates {
        let named = match update {
            base::Update::UpdateBotNewBusinessMessage(u) => &mut u.connection_id,
            base::Update::UpdateBotEditBusinessMessage(u) => &mut u.connection_id,
            _ => continue,
        };
        if named.is_empty() {
            connection_id.clone_into(named);
        }
    }
}

tokio::task_local! {
    static NO_RETRIES: ();
}

/// Runs `future` with every request it sends going out once, whatever the retry policy says: a failure, a
/// FLOOD_WAIT included, goes straight back to the caller.
pub(crate) async fn without_retries<F: Future>(future: F) -> F::Output {
    NO_RETRIES.scope((), future).await
}

/// Sends `request`, asking the session's retry policy after every failure whether to send it again. `name` is
/// what the policy is told the request is.
pub(crate) async fn invoke_sleeping_floods<R: RemoteCall>(
    session: &Session,
    name: &'static str,
    request: &R,
    timeout: Duration,
) -> Result<R::Return> {
    let mut tally = Tally::default();
    loop {
        let result = session.invoke_with_timeout(request, timeout).await;
        let Err(error) = &result else { return result };
        if NO_RETRIES.try_with(|_| ()).is_ok() {
            return result;
        }
        tally.failed(error);
        match session.should_retry(name, error, tally, None, false) {
            ControlFlow::Continue(wait) => {
                tally.slept += wait;
                tokio::time::sleep(wait).await;
            }
            ControlFlow::Break(()) => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc;
    use tokio::time::Instant;

    use super::*;
    use crate::connection::Options;
    use crate::enums::ChatAction;
    use crate::errors::RpcError;
    use crate::raw::{Serializable, base, types};
    use crate::session::RateCategory::{Account, Admin, Bulk, Global, Media, Message, Query};
    use crate::session::{AutoSleep, NoRetries, RateLimiter, RateLimits, Retry, RetryContext, RetryPolicy};

    const FOREIGN_DC: i32 = 4;

    fn flood(seconds: u32) -> Error {
        Error::Rpc(RpcError::parse(420, &format!("FLOOD_WAIT_{seconds}")))
    }

    // Answers FLOOD_WAIT_1 first, then a pong.
    fn flooded(flood_sleep_threshold: Duration) -> (Client, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (mut client, sent) =
            Client::scripted(vec![Err(flood(1)), Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes())]);
        let retry = Retry::new(AutoSleep::new().threshold(flood_sleep_threshold));
        client.set_options(Options::new().retry(retry));
        (client, sent)
    }

    fn count(sent: &mut mpsc::UnboundedReceiver<Vec<u8>>) -> usize {
        std::iter::from_fn(|| sent.try_recv().ok()).count()
    }

    async fn invoke_on_foreign_dc(replies: Vec<Result<Vec<u8>>>) -> Result<bool> {
        let client = Client::offline();
        let (foreign, _sent) = Session::scripted(replies);
        client.0.transfers.insert(client.dc(FOREIGN_DC).unwrap(), &foreign);
        client.invoke_inline(FOREIGN_DC, functions::account::UpdateStatus { offline: false }).await
    }

    #[tokio::test]
    async fn zero_threshold_hands_flood_waits_to_the_caller() {
        let (client, mut sent) = flooded(Duration::ZERO);
        let e = client.invoke(&functions::Ping { ping_id: 7 }).await.unwrap_err();
        let Error::Rpc(e) = e else { panic!("{e}") };
        assert_eq!((e.name.as_str(), e.value, count(&mut sent)), ("FLOOD_WAIT", Some(1), 1));
    }

    #[tokio::test]
    async fn invoke_in_dc_on_the_home_dc_is_a_plain_invoke() {
        let (client, mut sent) = Client::scripted(vec![Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes())]);
        let ping = functions::Ping { ping_id: 7 };
        assert!(client.invoke_in_dc(2, &ping).await.is_ok());
        assert_eq!(sent.try_recv().unwrap(), ping.to_bytes());
    }

    #[tokio::test]
    async fn a_takeout_client_wraps_every_request() {
        let pong = || Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes());
        let (client, mut sent) = Client::scripted(vec![pong(), pong()]);
        let takeout = Client(client.0.clone(), Some(7));
        let ping = functions::Ping { ping_id: 7 };
        let wrapped = functions::InvokeWithTakeout { takeout_id: 7, query: functions::Ping { ping_id: 7 } }.to_bytes();
        assert!(takeout.invoke(&ping).await.is_ok());
        assert_eq!(sent.try_recv().unwrap(), wrapped);
        assert!(takeout.clone().invoke(&ping).await.is_ok());
        assert_eq!(sent.try_recv().unwrap(), wrapped);
    }

    #[tokio::test]
    async fn invoke_in_dc_from_a_takeout_client_wraps_inside_init_connection() {
        let client = Client::offline();
        let (foreign, mut sent) = Session::scripted(vec![Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes())]);
        client.0.transfers.insert(client.dc(FOREIGN_DC).unwrap(), &foreign);
        let takeout = Client(client.0.clone(), Some(7));
        assert!(takeout.invoke_in_dc(FOREIGN_DC, &functions::Ping { ping_id: 7 }).await.is_ok());
        let wrapped = functions::InvokeWithTakeout { takeout_id: 7, query: functions::Ping { ping_id: 7 } };
        let expected = crate::session::init_connection(client.0.api_id, &client.0.options, wrapped).to_bytes();
        assert_eq!(sent.try_recv().unwrap(), expected);
    }

    #[tokio::test]
    async fn invoke_in_dc_imports_the_login_once() {
        use crate::session::test_dc::INVOKE_WITH_LAYER as LAYER;
        use crate::session::test_dc::{EXPORT_AUTHORIZATION, IMPORT_AUTHORIZATION, INIT_CONNECTION, PING, TestDc};
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        for ping_id in 1..=2 {
            let base::Pong::Pong(pong) = client.invoke_in_dc(FOREIGN_DC, &functions::Ping { ping_id }).await.unwrap();
            assert_eq!(pong.ping_id, ping_id);
        }
        let calls = |connection| -> Vec<Vec<u32>> {
            dc.seen().into_iter().filter(|s| s.connection == connection).map(|s| s.ids).collect()
        };
        assert_eq!(calls(1).iter().filter(|ids| **ids == [EXPORT_AUTHORIZATION]).count(), 1);
        let import = vec![LAYER, INIT_CONNECTION, IMPORT_AUTHORIZATION];
        let ping = vec![LAYER, INIT_CONNECTION, PING];
        assert_eq!(calls(2), [import, ping.clone(), ping]);
    }

    #[tokio::test]
    async fn the_observer_sees_the_innermost_tag_of_each_call() {
        use crate::observe::tagged;
        use crate::session::test_dc::TestDc;
        let dc = TestDc::start([5; 256], false).await;
        let (client, tags) = dc.observed_client([5; 256]).await;
        let client = &client;
        let ping = |ping_id| async move { client.invoke(&functions::Ping { ping_id }).await };
        ping(1).await.unwrap();
        tagged(7, ping(2)).await.unwrap();
        tagged(7, async {
            tagged(8, ping(3)).await?;
            ping(4).await
        })
        .await
        .unwrap();
        let pings: Vec<_> = tags.lock().unwrap().iter().filter(|(name, _)| *name == "ping").map(|c| c.1).collect();
        assert_eq!(pings, [None, Some(7), Some(8), Some(7)]);
    }

    #[tokio::test]
    async fn the_login_a_call_moves_to_another_dc_carries_its_tag() {
        use crate::observe::tagged;
        use crate::session::test_dc::TestDc;
        let dc = TestDc::start([5; 256], false).await;
        let (client, tags) = dc.observed_client([5; 256]).await;
        let before = tags.lock().unwrap().len();
        tagged(3, client.invoke_in_dc(FOREIGN_DC, &functions::Ping { ping_id: 1 })).await.unwrap();
        let calls = tags.lock().unwrap()[before..].to_vec();
        // The export on the home DC, then the import and the ping on the new connection.
        assert_eq!(
            calls,
            [("auth.exportAuthorization", Some(3)), ("invokeWithLayer", Some(3)), ("invokeWithLayer", Some(3))]
        );
    }

    #[tokio::test]
    async fn a_no_updates_client_wraps_what_it_sends_to_another_dc() {
        use crate::session::test_dc::INVOKE_WITH_LAYER as LAYER;
        use crate::session::test_dc::{
            EXPORT_AUTHORIZATION, IMPORT_AUTHORIZATION, INIT_CONNECTION, INVOKE_WITHOUT_UPDATES as QUIET, PING, TestDc,
        };
        let dc = TestDc::start([5; 256], false).await;
        let builder = Client::builder("quiet-dc", 1, "hash").session_string(dc.session_string([5; 256]));
        let client = builder.no_updates().connect().await.unwrap();
        client.invoke_in_dc(FOREIGN_DC, &functions::Ping { ping_id: 1 }).await.unwrap();

        // Telegram describes invokeWithoutUpdates as "Invoke a request without subscribing the used connection for
        // updates (this is enabled by default for file queries)" and doesn't say what an unwrapped request does to
        // a connection's subscription, so the export and the import of the login are wrapped like the rest. Behind
        // the layer and initConnection of a fresh connection, every request is.
        let seen: Vec<Vec<u32>> = dc.seen().into_iter().map(|s| s.ids).collect();
        assert!(seen.contains(&vec![QUIET, EXPORT_AUTHORIZATION]), "{seen:?}");
        assert!(seen.contains(&vec![LAYER, INIT_CONNECTION, QUIET, IMPORT_AUTHORIZATION]), "{seen:?}");
        assert!(seen.contains(&vec![LAYER, INIT_CONNECTION, QUIET, PING]), "{seen:?}");
        for ids in &seen {
            let inner: Vec<u32> = ids.iter().copied().filter(|id| ![LAYER, INIT_CONNECTION].contains(id)).collect();
            assert_eq!(inner[0], QUIET, "{ids:?}");
        }
    }

    #[tokio::test]
    async fn invoke_in_dc_on_an_unknown_dc_fails_before_any_request() {
        let (client, mut sent) = Client::recording();
        let result = client.invoke_in_dc(99, &functions::Ping { ping_id: 7 }).await;
        assert!(matches!(result, Err(Error::InvalidDc(99))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn short_flood_waits_are_slept_off() {
        let (client, mut sent) = flooded(Duration::from_secs(60));
        let base::Pong::Pong(pong) = client.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
        assert_eq!((pong.ping_id, count(&mut sent)), (7, 2));
    }

    #[tokio::test(start_paused = true)]
    async fn inline_call_on_another_dc_sleeps_off_a_short_flood_wait() {
        let start = Instant::now();
        let result = invoke_on_foreign_dc(vec![Err(flood(3)), Ok(true.to_bytes())]).await;
        assert!(matches!(result, Ok(true)), "{result:?}");
        assert_eq!(start.elapsed(), Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn inline_call_on_another_dc_returns_a_long_flood_wait() {
        let start = Instant::now();
        let result = invoke_on_foreign_dc(vec![Err(flood(61))]).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.code == 420));
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    fn rpc_420(message: &str) -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError::parse(420, message)))
    }

    async fn ping(replies: Vec<Result<Vec<u8>>>) -> (Result<base::Pong>, usize) {
        ping_with(Options::default(), replies).await
    }

    async fn ping_with(options: Options, mut replies: Vec<Result<Vec<u8>>>) -> (Result<base::Pong>, usize) {
        replies.push(Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes()));
        let (mut client, mut sent) = Client::scripted(replies);
        client.set_options(options);
        let result = client.invoke(&functions::Ping { ping_id: 7 }).await;
        (result, count(&mut sent))
    }

    #[tokio::test(start_paused = true)]
    async fn at_most_five_flood_waits_are_slept_per_call() {
        let (result, sends) = ping((0..6).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 6);
        let (result, sends) = ping((0..5).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(result.is_ok());
        assert_eq!(sends, 6);
    }

    #[tokio::test(start_paused = true)]
    async fn a_setting_of_two_sleeps_two_flood_waits() {
        let two = || Options::new().retry(Retry::new(AutoSleep::new().tries(2)));
        let (result, sends) = ping_with(two(), (0..3).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 3);
        let (result, sends) = ping_with(two(), (0..2).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(result.is_ok());
        assert_eq!(sends, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn zero_flood_sleeps_hand_every_wait_to_the_caller() {
        let none = Options::new().retry(Retry::new(AutoSleep::new().tries(0)));
        let (result, sends) = ping_with(none, vec![rpc_420("FLOOD_WAIT_1")]).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 1);
    }

    fn with_policy(policy: impl RetryPolicy) -> Options {
        Options::new().retry(Retry::new(policy))
    }

    #[tokio::test(start_paused = true)]
    async fn no_retries_hands_back_the_first_flood() {
        let (result, sends) = ping_with(with_policy(NoRetries), vec![rpc_420("FLOOD_WAIT_1")]).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 1);
    }

    // Retries twice after a second, and writes down what it was told each time.
    type Asked = (&'static str, u32, Duration, Option<Duration>);
    struct Watching(Arc<std::sync::Mutex<Vec<Asked>>>);

    impl RetryPolicy for Watching {
        fn should_retry(&self, ctx: &RetryContext<'_>) -> ControlFlow<(), Duration> {
            self.0.lock().unwrap().push((ctx.request, ctx.fail_count, ctx.slept_so_far, ctx.flood_wait));
            if ctx.fail_count <= 2 { ControlFlow::Continue(Duration::from_secs(1)) } else { ControlFlow::Break(()) }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_custom_policy_sees_the_request_name_and_counts() {
        let seen = Arc::default();
        let floods = (0..3).map(|_| rpc_420("FLOOD_WAIT_9")).collect();
        let start = Instant::now();
        let (result, sends) = ping_with(with_policy(Watching(Arc::clone(&seen))), floods).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        // The policy's own wait was slept, not Telegram's.
        assert_eq!((sends, start.elapsed()), (3, Duration::from_secs(2)));
        let nine = Some(Duration::from_secs(9));
        let second = Duration::from_secs(1);
        let expected = [("ping", 1, Duration::ZERO, nine), ("ping", 2, second, nine), ("ping", 3, second * 2, nine)];
        assert_eq!(*seen.lock().unwrap(), expected);
    }

    #[tokio::test(start_paused = true)]
    async fn a_wrapped_request_is_named_by_the_request_inside() {
        let seen = Arc::default();
        let (mut client, _sent) = Client::scripted(vec![rpc_420("FLOOD_WAIT_9"), Ok(true.to_bytes())]);
        client.set_options(with_policy(Watching(Arc::clone(&seen))));
        client.0.caches.know_business("c", 7, 2);
        client.invoke_business(Some("c"), functions::account::UpdateStatus { offline: true }).await.unwrap();
        assert_eq!(seen.lock().unwrap()[0].0, "account.updateStatus");
    }

    #[tokio::test(start_paused = true)]
    async fn only_real_waits_within_the_threshold_are_slept() {
        let caller_gets = [
            "FLOOD_WAIT_61",
            "2FA_CONFIRM_WAIT_604800",
            "PREMIUM_SUB_ACTIVE_UNTIL_1700000000",
            "STORY_SEND_FLOOD_60",
            "FROZEN_METHOD_INVALID",
            "ADDRESS_INVALID",
        ];
        for name in caller_gets {
            let (result, sends) = ping(vec![rpc_420(name)]).await;
            assert!(result.is_err() && sends == 1, "{name}");
        }
        for name in ["SLOWMODE_WAIT_10", "FLOOD_PREMIUM_WAIT_5", "FLOOD_TEST_PHONE_WAIT_3", "FLOOD_SKIP_FAILED_WAIT"] {
            let (result, sends) = ping(vec![rpc_420(name)]).await;
            assert!(result.is_ok() && sends == 2, "{name}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_caller_timeout_ends_the_flood_sleep() {
        let pong = Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes());
        let (client, mut sent) = Client::scripted(vec![rpc_420("FLOOD_WAIT_30"), pong]);
        let call = client.invoke(&functions::Ping { ping_id: 7 });
        assert!(tokio::time::timeout(Duration::from_secs(10), call).await.is_err());
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(count(&mut sent), 1);
    }

    const CATEGORIES: [RateCategory; 7] = [Message, Media, Query, Admin, Bulk, Account, Global];

    // One request's worth of every bucket, refilling slowly: what's missing afterwards was taken.
    fn limited(client: &mut Client) {
        let limits = CATEGORIES.iter().fold(RateLimits::default(), |limits, &c| limits.limit(c, 0.001, 5.0));
        Arc::get_mut(&mut client.0).unwrap().limiter = Some(RateLimiter::new(&limits));
    }

    fn taken(client: &Client) -> Vec<RateCategory> {
        let limiter = client.rate_limiter().unwrap();
        CATEGORIES.into_iter().filter(|&category| limiter.available(category) < 4.5).collect()
    }

    fn set_typing() -> functions::messages::SetTyping {
        functions::messages::SetTyping {
            peer: base::InputPeer::InputPeerSelf,
            top_msg_id: None,
            action: ChatAction::Typing.write(),
        }
    }

    #[tokio::test]
    async fn business_calls_use_the_inner_name() {
        let (mut client, _sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        limited(&mut client);
        client.0.caches.know_business("c", 7, 2);
        client.invoke_business(Some("c"), set_typing()).await.unwrap();
        // `messages.setTyping` is an admin call; the wrapper's own name would have counted as a query.
        assert_eq!(taken(&client), [Admin, Global]);
    }

    #[tokio::test]
    async fn file_parts_and_difference_are_not_limited() {
        let replies = (0..4).map(|_| Ok(true.to_bytes())).collect();
        let (mut client, _sent) = Client::scripted(replies);
        limited(&mut client);
        let session = client.session();
        // File parts go to their connection, and the dispatcher's getDifference straight to the session.
        let timeout = crate::session::transfer::MIN_PART_TIMEOUT;
        client.invoke_file_part(&session, &set_typing(), timeout, None).await.unwrap();
        session.invoke(&set_typing()).await.unwrap();
        assert!(taken(&client).is_empty());

        // A plain call does take tokens, once, whether or not it's addressed to the home DC.
        client.invoke(&set_typing()).await.unwrap();
        assert_eq!(taken(&client), [Admin, Global]);
        client.invoke_in_dc(session.dc_id(), &set_typing()).await.unwrap();
        let limiter = client.rate_limiter().unwrap();
        assert_eq!((limiter.available(Admin).round(), limiter.available(Global).round()), (3.0, 3.0));
    }

    fn quiet_client(replies: usize) -> (Client, mpsc::UnboundedReceiver<Vec<u8>>) {
        let pong = || Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes());
        let (mut client, sent) = Client::scripted((0..replies).map(|_| pong()).collect());
        client.set_options(Options::new().no_updates(true));
        (client, sent)
    }

    #[tokio::test]
    async fn no_updates_wraps_every_call() {
        let (client, mut sent) = quiet_client(1);
        let wrapped = functions::InvokeWithoutUpdates { query: functions::Ping { ping_id: 7 } };
        client.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), wrapped.to_bytes());
    }

    #[tokio::test]
    async fn takeout_wraps_outside() {
        let (client, mut sent) = quiet_client(1);
        let takeout = Client(client.0.clone(), Some(7));
        takeout.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
        let wrapped = functions::InvokeWithTakeout {
            takeout_id: 7,
            query: functions::InvokeWithoutUpdates { query: functions::Ping { ping_id: 7 } },
        };
        assert_eq!(sent.try_recv().unwrap(), wrapped.to_bytes());
    }

    fn auto_client(replies: usize, no_updates: bool) -> (Client, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (mut client, sent) = Client::scripted((0..replies).map(|_| Ok(true.to_bytes())).collect());
        client.set_options(Options::new().no_updates(no_updates));
        Arc::get_mut(&mut client.0).unwrap().auto_no_updates = true;
        (client, sent)
    }

    fn status() -> functions::account::UpdateStatus {
        functions::account::UpdateStatus { offline: true }
    }

    #[tokio::test]
    async fn auto_wraps_reads_not_writes() {
        let (client, mut sent) = auto_client(2, false);
        client.invoke(&set_typing()).await.unwrap();
        client.invoke(&status()).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::InvokeWithoutUpdates { query: set_typing() }.to_bytes());
        assert_eq!(sent.try_recv().unwrap(), status().to_bytes());
    }

    #[tokio::test]
    async fn business_and_takeout_use_the_inner_name() {
        let (client, mut sent) = auto_client(2, false);
        client.0.caches.know_business("c", 7, 2);
        client.invoke_business(Some("c"), set_typing()).await.unwrap();
        let business = functions::InvokeWithBusinessConnection {
            connection_id: "c".into(),
            query: functions::InvokeWithoutUpdates { query: set_typing() },
        };
        assert_eq!(sent.try_recv().unwrap(), business.to_bytes());

        // A takeout holds the quiet request, and the request inside a takeout is still judged by its own name.
        let takeout = Client(client.0.clone(), Some(7));
        takeout.invoke(&set_typing()).await.unwrap();
        let wrapped = functions::InvokeWithTakeout {
            takeout_id: 7,
            query: functions::InvokeWithoutUpdates { query: set_typing() },
        };
        assert_eq!(sent.try_recv().unwrap(), wrapped.to_bytes());
    }

    #[tokio::test]
    async fn service_calls_are_never_auto_wrapped() {
        let pong = types::Pong { msg_id: 0, ping_id: 7 }.to_bytes();
        let (mut client, mut sent) = Client::scripted(vec![Ok(pong)]);
        Arc::get_mut(&mut client.0).unwrap().auto_no_updates = true;
        client.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::Ping { ping_id: 7 }.to_bytes());
    }

    #[tokio::test]
    async fn no_updates_wins_over_auto() {
        let (client, mut sent) = auto_client(1, true);
        client.invoke(&status()).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::InvokeWithoutUpdates { query: status() }.to_bytes());
    }

    #[tokio::test]
    async fn a_field_too_long_for_tl_is_an_invalid_argument() {
        let (client, mut sent) = Client::recording();
        let text = "a".repeat(16 << 20);
        let result = client.send_message(crate::types::PeerRef::Me, text.as_str()).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
        // The check is only on for that call: a raw serialization still panics as documented.
        assert!(std::panic::catch_unwind(|| text.as_bytes().to_bytes()).is_err());
    }

    #[tokio::test]
    async fn a_request_over_16_mib_is_an_invalid_argument() {
        let (client, mut sent) = Client::recording();
        // 8 bytes each and no field too long: only the total is over the cap.
        let id = vec![base::InputMessage::InputMessageID(types::InputMessageID { id: 1 }); 2_200_000];
        let result = client.invoke(&functions::messages::GetMessages { id }).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    fn business_typing() -> functions::InvokeWithBusinessConnection<functions::messages::SetTyping> {
        functions::InvokeWithBusinessConnection { connection_id: "c".into(), query: set_typing() }
    }

    /// `updateBotBusinessConnect` for the connection `c` of user 7 on `dc_id`.
    fn connected(dc_id: i32) -> base::Update {
        let connection = types::BotBusinessConnection {
            disabled: false,
            connection_id: "c".into(),
            user_id: 7,
            dc_id,
            date: 1,
            rights: None,
        };
        let connection = base::BotBusinessConnection::BotBusinessConnection(Box::new(connection));
        base::Update::UpdateBotBusinessConnect(Box::new(types::UpdateBotBusinessConnect { connection, qts: 0 }))
    }

    /// What `getBotBusinessConnection` answers for that connection.
    fn connection_on(dc_id: i32) -> Vec<u8> {
        base::Updates::UpdateShort(Box::new(types::UpdateShort { update: connected(dc_id), date: 0 })).to_bytes()
    }

    type Sent = mpsc::UnboundedReceiver<Vec<u8>>;

    /// A client at home on DC 2 with a scripted session for `FOREIGN_DC`, which lives as long as the returned one.
    fn with_foreign(home: Vec<Result<Vec<u8>>>, foreign: Vec<Result<Vec<u8>>>) -> (Client, Sent, Sent, Session) {
        let (client, home) = Client::scripted(home);
        let (session, away) = Session::scripted(foreign);
        client.0.transfers.insert(client.dc(FOREIGN_DC).unwrap(), &session);
        (client, home, away, session)
    }

    fn on_file_connection<R: RemoteCall>(client: &Client, request: R) -> Vec<u8> {
        crate::session::init_connection(client.0.api_id, &client.0.options, request).to_bytes()
    }

    #[tokio::test]
    async fn business_calls_go_to_the_connections_dc() {
        let (client, mut home, mut away, _foreign) =
            with_foreign(vec![Ok(connection_on(FOREIGN_DC))], vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        for _ in 0..2 {
            client.invoke_business(Some("c"), set_typing()).await.unwrap();
            assert_eq!(away.try_recv().unwrap(), on_file_connection(&client, business_typing()));
        }
        // The connection was fetched once, at home, and the client stayed there.
        let fetch = functions::account::GetBotBusinessConnection { connection_id: "c".into() };
        assert_eq!(home.try_recv().unwrap(), fetch.to_bytes());
        assert!(home.try_recv().is_err());
        assert_eq!(client.session().dc_id(), 2);
    }

    #[tokio::test]
    async fn a_connection_seen_in_an_update_is_not_fetched() {
        let (client, mut home, mut away, _foreign) = with_foreign(vec![Ok(true.to_bytes())], Vec::new());
        client.0.caches.observe(&connected(2), &Arc::default());
        client.invoke_business(Some("c"), set_typing()).await.unwrap();
        assert_eq!(home.try_recv().unwrap(), business_typing().to_bytes());
        assert!(home.try_recv().is_err() && away.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_migrate_answer_moves_the_connection_not_the_client() {
        let migrate = Err(Error::Rpc(RpcError::parse(303, "USER_MIGRATE_4")));
        let (client, mut home, mut away, _foreign) =
            with_foreign(vec![migrate], vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.0.caches.know_business("c", 7, 2);
        client.invoke_business(Some("c"), set_typing()).await.unwrap();
        assert_eq!(home.try_recv().unwrap(), business_typing().to_bytes());
        assert_eq!(away.try_recv().unwrap(), on_file_connection(&client, business_typing()));
        assert_eq!(client.session().dc_id(), 2);

        // The next call goes straight to the DC the answer named.
        client.invoke_business(Some("c"), set_typing()).await.unwrap();
        assert!(home.try_recv().is_err());
        assert_eq!(away.try_recv().unwrap(), on_file_connection(&client, business_typing()));
    }

    #[tokio::test]
    async fn a_business_call_moves_only_once() {
        let migrate = |dc: i32| Err(Error::Rpc(RpcError::parse(303, &format!("USER_MIGRATE_{dc}"))));
        let (client, _home, mut away, _foreign) = with_foreign(vec![migrate(FOREIGN_DC)], vec![migrate(5)]);
        client.0.caches.know_business("c", 7, 2);
        let result = client.invoke_business(Some("c"), set_typing()).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.value == Some(5)), "{result:?}");
        assert_eq!(std::iter::from_fn(|| away.try_recv().ok()).count(), 1);
    }
}
