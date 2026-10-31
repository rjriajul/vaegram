use crate::Client;

impl Client {
    /// Ends every upload and download in flight with `Error::Disconnected`, and the calls in flight to other data
    /// centers, which share those connections. It doesn't wait, so a `.progress` callback can call it. The next
    /// transfer connects again. To cancel a single transfer, drop its future.
    pub fn stop_transmission(&self) {
        self.0.transfers.close_all();
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::Error;
    use crate::raw::functions;
    use crate::session::test_dc::{EXPORT_AUTHORIZATION, TestDc};

    #[tokio::test]
    async fn stop_transmission_ends_file_connections_only() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let ping = functions::Ping { ping_id: 1 };
        let lease = client.transfer_sessions(client.dc(4).unwrap(), 1).await.unwrap();
        assert!(lease.sessions[0].invoke(&ping).await.is_ok());

        client.stop_transmission();
        assert!(matches!(lease.sessions[0].invoke(&ping).await, Err(Error::Disconnected)));
        assert!(client.invoke(&ping).await.is_ok());
        drop(client.transfer_sessions(client.dc(4).unwrap(), 1).await.unwrap());
        let exports = dc.seen().into_iter().filter(|s| s.ids == [EXPORT_AUTHORIZATION]).count();
        assert_eq!(exports, 2, "the next transfer must log in on the other data center again");
    }
}
