use std::time::Duration;

use crate::Client;
use crate::errors::{Error, Result, RpcError};
use crate::raw::{RemoteCall, functions};
use crate::session::Session;

const MAX_MIGRATIONS: usize = 3;
// FLOOD_WAITs up to this many seconds are slept off and retried; longer ones go to the caller.
const FLOOD_SLEEP_THRESHOLD: u32 = 60;

impl Client {
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        invoke_sleeping_floods(&self.session(), request).await
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

pub(crate) async fn invoke_sleeping_floods<R: RemoteCall>(session: &Session, request: &R) -> Result<R::Return> {
    loop {
        match session.invoke(request).await {
            Err(Error::Rpc(RpcError { code: 420, value: Some(wait), .. })) if wait <= FLOOD_SLEEP_THRESHOLD => {
                tokio::time::sleep(Duration::from_secs(wait.max(1).into())).await;
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::{Duration, Instant};

    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, functions};
    use crate::session::Session;

    const FOREIGN_DC: i32 = 4;

    fn flood(seconds: u32) -> Error {
        Error::Rpc(RpcError::new(420, &format!("FLOOD_WAIT_{seconds}")))
    }

    async fn invoke_on_foreign_dc(replies: Vec<crate::errors::Result<Vec<u8>>>) -> crate::errors::Result<bool> {
        let client = Client::offline();
        let (foreign, _sent) = Session::scripted(replies);
        client.0.transfers.insert(client.dc(FOREIGN_DC).unwrap(), &foreign);
        client.invoke_inline(FOREIGN_DC, functions::account::UpdateStatus { offline: false }).await
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
        let result = invoke_on_foreign_dc(vec![Err(flood(super::FLOOD_SLEEP_THRESHOLD + 1))]).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.code == 420));
        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
