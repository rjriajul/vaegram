use crate::Client;

impl Client {
    /// Closes every connection for good. `run` returns, and requests waiting or made later fail with
    /// `Error::Disconnected`, on every data center; so does `restart`. Build a new client to connect again.
    ///
    /// It doesn't wait for `run`, so a handler can call it. To let the running handlers finish first: `stop`,
    /// await `run`, then `disconnect`.
    pub fn disconnect(&self) {
        self.stop();
        self.session().close();
        self.0.transfers.shut();
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::Error;
    use crate::raw::functions;
    use crate::session::test_dc::TestDc;

    #[tokio::test]
    async fn disconnect_ends_every_connection() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let ping = functions::Ping { ping_id: 1 };
        assert!(client.invoke(&ping).await.is_ok());
        let lease = client.transfer_sessions(client.dc(4).unwrap(), 1).await.unwrap();

        client.disconnect();
        assert!(matches!(client.invoke(&ping).await, Err(Error::Disconnected)));
        assert!(matches!(lease.sessions[0].invoke(&ping).await, Err(Error::Disconnected)));
        assert!(matches!(client.restart(), Err(Error::Disconnected)));
        assert!(*client.0.stop.borrow(), "a running run() wouldn't stop");
        let home = client.dc(2).unwrap();
        assert!(
            matches!(client.transfer_sessions(home, 1).await, Err(Error::Disconnected)),
            "a download could still run"
        );
    }
}
