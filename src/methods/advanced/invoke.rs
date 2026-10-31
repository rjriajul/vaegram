use std::time::Duration;

use crate::Client;
use crate::errors::{Error, Result, RpcError};
use crate::raw::{RemoteCall, functions};
use crate::session::Session;

const MAX_MIGRATIONS: usize = 3;

impl Client {
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.invoke_with_timeout(request, self.session().options().request_timeout).await
    }

    /// A request that times out is forgotten: the server is asked not to send its answer, and the connection
    /// carries on. FLOOD_WAITs up to `flood_sleep_threshold` are slept off and retried, each try with the full
    /// `timeout`, at most `max_flood_sleeps` times; the next one goes to the caller. `tokio::time::timeout` around
    /// the call bounds the whole wait.
    pub async fn invoke_with_timeout<R: RemoteCall>(&self, request: &R, timeout: Duration) -> Result<R::Return> {
        invoke_sleeping_floods(&self.session(), request, timeout).await
    }

    /// Hands the request back: a short "sent" update only carries what Telegram changed, the rest comes from it.
    pub(crate) async fn invoke_business<R: RemoteCall>(
        &self,
        connection_id: Option<&str>,
        request: R,
    ) -> Result<(R::Return, R)> {
        match connection_id {
            Some(connection_id) => {
                let call =
                    functions::InvokeWithBusinessConnection { connection_id: connection_id.into(), query: request };
                Ok((self.invoke(&call).await?, call.query))
            }
            None => Ok((self.invoke(&request).await?, request)),
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

pub(crate) async fn invoke_sleeping_floods<R: RemoteCall>(
    session: &Session,
    request: &R,
    timeout: Duration,
) -> Result<R::Return> {
    let mut slept = 0;
    loop {
        let result = session.invoke_with_timeout(request, timeout).await;
        match result.as_ref().err().and_then(|e| session.flood_sleep(e, None)) {
            Some(wait) if slept < session.options().max_flood_sleeps => {
                slept += 1;
                tokio::time::sleep(wait).await;
            }
            _ => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;
    use tokio::time::Instant;

    use super::*;
    use crate::connection::Options;
    use crate::errors::RpcError;
    use crate::raw::{Serializable, base, types};

    const FOREIGN_DC: i32 = 4;

    fn flood(seconds: u32) -> Error {
        Error::Rpc(RpcError::new(420, &format!("FLOOD_WAIT_{seconds}")))
    }

    // Answers FLOOD_WAIT_1 first, then a pong.
    fn flooded(flood_sleep_threshold: Duration) -> (Client, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (mut client, sent) =
            Client::scripted(vec![Err(flood(1)), Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes())]);
        client.set_options(Options { flood_sleep_threshold, ..Options::default() });
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
        Err(Error::Rpc(RpcError::new(420, message)))
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
        let two = || Options { max_flood_sleeps: 2, ..Options::default() };
        let (result, sends) = ping_with(two(), (0..3).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 3);
        let (result, sends) = ping_with(two(), (0..2).map(|_| rpc_420("FLOOD_WAIT_1")).collect()).await;
        assert!(result.is_ok());
        assert_eq!(sends, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn zero_flood_sleeps_hand_every_wait_to_the_caller() {
        let none = Options { max_flood_sleeps: 0, ..Options::default() };
        let (result, sends) = ping_with(none, vec![rpc_420("FLOOD_WAIT_1")]).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.is("FLOOD_WAIT")));
        assert_eq!(sends, 1);
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
}
