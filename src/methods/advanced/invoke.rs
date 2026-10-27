use std::time::Duration;

use crate::Client;
use crate::errors::{Error, Result, RpcError};
use crate::raw::RemoteCall;

const MAX_MIGRATIONS: usize = 3;
// FLOOD_WAITs up to this many seconds are slept off and retried; longer ones go to the caller.
const FLOOD_SLEEP_THRESHOLD: u32 = 60;

impl Client {
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        loop {
            match self.session().invoke(request).await {
                Err(Error::Rpc(RpcError { code: 420, value: Some(wait), .. })) if wait <= FLOOD_SLEEP_THRESHOLD => {
                    tokio::time::sleep(Duration::from_secs(wait.max(1).into())).await;
                }
                result => return result,
            }
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
