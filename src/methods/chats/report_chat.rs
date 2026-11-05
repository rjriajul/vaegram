use crate::Client;
use crate::enums::ReportReason;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Reports a chat to Telegram moderators with a reason and a free-form message.
    ///
    /// Returns the `bool` Telegram sends back.
    pub async fn report_chat(&self, chat_id: impl Into<PeerRef>, reason: ReportReason, message: &str) -> Result<bool> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::account::ReportPeer { peer, reason: reason.write(), message: message.into() };
        self.invoke(&request).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ReportReason;
    use crate::raw::functions::account::ReportPeer;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn the_reason_and_message_are_sent() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes())]);
        assert!(client.report_chat(-5_i64, ReportReason::Fake, "not the real shop").await.unwrap());
        let expected = ReportPeer {
            peer: base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 }),
            reason: base::ReportReason::InputReportReasonFake,
            message: "not the real shop".into(),
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
