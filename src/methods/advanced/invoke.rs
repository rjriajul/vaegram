use std::time::Duration;

use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{RemoteCall, functions};
use crate::session::Session;

const MAX_MIGRATIONS: usize = 3;

impl Client {
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.invoke_with_timeout(request, self.session().options().request_timeout).await
    }

    /// A request that times out is forgotten: the server is asked not to send its answer, and the connection
    /// carries on. FLOOD_WAITs up to `flood_sleep_threshold` are slept off and retried, each try with the full
    /// `timeout`.
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
            match self.invoke(request).await {
                Err(Error::Rpc(e)) if e.code == 303 && migrations < MAX_MIGRATIONS => {
                    let dc_id = e.value.ok_or(Error::Unexpected("migrate error without a DC number"))?;
                    self.switch_dc(dc_id as i32).await?;
                    migrations += 1;
                }
                result => return result,
            }
        }
    }
}

pub(crate) async fn invoke_sleeping_floods<R: RemoteCall>(
    session: &Session,
    request: &R,
    timeout: Duration,
) -> Result<R::Return> {
    loop {
        let result = session.invoke_with_timeout(request, timeout).await;
        match result.as_ref().err().and_then(|e| session.flood_sleep(e, None)) {
            Some(wait) => tokio::time::sleep(wait).await,
            None => return result,
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

    #[tokio::test]
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
}
