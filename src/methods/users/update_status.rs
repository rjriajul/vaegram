use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Marks your account online, or offline when `offline` is true.
    ///
    /// Telegram answers `false` when it takes no action (seen live), so either answer means the call went through.
    pub async fn update_status(&self, offline: bool) -> Result<()> {
        self.invoke(&functions::account::UpdateStatus { offline }).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn a_false_answer_is_ok() {
        let (client, mut sent) = Client::scripted(vec![Ok(false.to_bytes())]);
        client.update_status(true).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), account::UpdateStatus { offline: true }.to_bytes());
    }
}
