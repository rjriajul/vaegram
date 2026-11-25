use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Sets the inactivity period, in seconds, after which push notifications stop showing message texts.
    pub async fn update_device_locked(&self, period: i32) -> Result<()> {
        accepted(self.invoke(&functions::account::UpdateDeviceLocked { period }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn sends_the_period() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes())]);
        client.update_device_locked(3600).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), account::UpdateDeviceLocked { period: 3600 }.to_bytes());
    }
}
