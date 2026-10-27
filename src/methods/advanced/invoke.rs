use crate::Client;
use crate::errors::Result;
use crate::raw::RemoteCall;

impl Client {
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.session().invoke(request).await
    }
}
