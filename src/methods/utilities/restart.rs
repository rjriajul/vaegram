use crate::Client;
use crate::errors::Result;

impl Client {
    /// Reconnects the main connection now, as after a network change: waiting requests are checked and sent
    /// again, missed updates are fetched and `ClientEvent::Reconnected` fires. `run` keeps going. File
    /// connections aren't touched. After `disconnect` it fails with `Error::Disconnected`.
    pub fn restart(&self) -> Result<()> {
        self.session().restart()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::enums::ClientEvent;
    use crate::raw::functions;
    use crate::session::test_dc::{PING, TestDc};

    #[tokio::test]
    async fn restart_moves_the_main_connection_to_a_new_socket() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let events = client.0.events.record();

        client.restart().unwrap();
        let reconnected = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match events.try_recv() {
                    Ok(event @ ClientEvent::Reconnected { .. }) => return event,
                    _ => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        });
        let event = reconnected.await.expect("no reconnect");
        assert!(matches!(event, ClientEvent::Reconnected { dc: 2, attempt: 1 }), "{event:?}");

        client.invoke(&functions::Ping { ping_id: 1 }).await.unwrap();
        let second: Vec<Vec<u32>> = dc.seen().into_iter().filter(|s| s.connection == 2).map(|s| s.ids).collect();
        assert_eq!(second, [vec![PING]]);
    }
}
