use crate::Client;

impl Client {
    /// Whether the main connection is up right now. It turns `false` when the connection is lost or `restart`ed
    /// and `true` again once it is back, at the same points `on_disconnect` and `on_connect` run; renewing a
    /// `pfs` temporary key doesn't count. After `disconnect`, or once the session ended for good, it stays `false`.
    /// `stop` only ends `run`, so the connection stays up. File connections don't count.
    pub fn is_connected(&self) -> bool {
        self.session().is_connected()
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::session::test_dc::TestDc;

    #[tokio::test]
    async fn a_connected_client_reports_up_until_disconnect() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        assert!(client.is_connected());

        client.stop();
        assert!(client.is_connected(), "stop closed the connection");
        client.disconnect();
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn a_client_without_a_connection_reports_down() {
        assert!(!Client::offline().is_connected());
    }
}
