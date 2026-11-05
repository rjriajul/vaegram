use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, ReportResult};

/// Request built by [`Client::report_messages`]. Awaiting it returns the [`ReportResult`] for this step.
#[must_use = "a request does nothing until it is awaited"]
pub struct ReportMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_ids: Vec<i32>,
    option: Vec<u8>,
    comment: String,
}

impl Client {
    /// Reports messages in steps until the result is [`ReportResult::Reported`].
    ///
    /// Awaiting it without an option asks Telegram for the options to choose from, and each answer says what the
    /// next step needs.
    pub fn report_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> ReportMessages<'_> {
        ReportMessages {
            client: self,
            chat_id: chat_id.into(),
            message_ids: message_ids.to_vec(),
            option: Vec::new(),
            comment: String::new(),
        }
    }
}

impl ReportMessages<'_> {
    /// The `option` of a [`ReportOption`](crate::types::ReportOption) or of [`ReportResult::AddComment`].
    pub fn option(mut self, option: impl Into<Vec<u8>>) -> Self {
        self.option = option.into();
        self
    }

    /// Free-text comment to send with the report, for when Telegram asks for one.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    async fn send(self) -> Result<ReportResult> {
        let request = functions::messages::Report {
            peer: self.client.resolve_peer(self.chat_id).await?,
            id: self.message_ids,
            option: self.option,
            message: self.comment,
        };
        Ok(ReportResult::parse(self.client.invoke(&request).await?))
    }
}

into_future!(ReportMessages, ReportResult);

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::raw::functions::messages::Report;
    use crate::raw::{Serializable, base, types};
    use crate::types::{ReportOption, ReportResult};

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    fn setup(answers: Vec<base::ReportResult>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(answers.iter().map(|a| Ok(a.to_bytes())).collect());
        client.0.peers.lock().unwrap().insert(7, peer());
        (client, rx)
    }

    fn choose() -> base::ReportResult {
        let option = |text: &str, option: &[u8]| {
            base::MessageReportOption::MessageReportOption(Box::new(types::MessageReportOption {
                text: text.into(),
                option: option.to_vec(),
            }))
        };
        let options = vec![option("Spam", b"1"), option("Violence", b"2")];
        base::ReportResult::ReportResultChooseOption(Box::new(types::ReportResultChooseOption {
            title: "Report".into(),
            options,
        }))
    }

    fn add_comment(optional: bool) -> base::ReportResult {
        let comment = types::ReportResultAddComment { optional, option: b"2a".to_vec() };
        base::ReportResult::ReportResultAddComment(Box::new(comment))
    }

    #[tokio::test]
    async fn the_first_step_asks_for_options() {
        let (client, mut rx) = setup(vec![choose()]);
        let result = client.report_messages(7, &[5, 6]).await.unwrap();
        let expected = Report { peer: peer(), id: vec![5, 6], option: Vec::new(), message: String::new() };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        let ReportResult::ChooseOption { title, options } = result else { panic!("{result:?}") };
        let option = |text: &str, option: &[u8]| ReportOption { text: text.into(), option: option.to_vec() };
        assert_eq!((title.as_str(), options), ("Report", vec![option("Spam", b"1"), option("Violence", b"2")]));
    }

    #[tokio::test]
    async fn an_option_and_comment_are_sent() {
        let (client, mut rx) = setup(vec![base::ReportResult::ReportResultReported]);
        let result = client.report_messages(7, &[5]).option(b"2a".as_slice()).comment("posted twice").await;
        assert_eq!(result.unwrap(), ReportResult::Reported);
        let expected = Report { peer: peer(), id: vec![5], option: b"2a".to_vec(), message: "posted twice".into() };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn every_report_result_is_read() {
        let (client, _rx) =
            setup(vec![add_comment(true), add_comment(false), base::ReportResult::ReportResultReported]);
        let mut results = Vec::new();
        for _ in 0..3 {
            results.push(client.report_messages(7, &[5]).option(b"2".as_slice()).await.unwrap());
        }
        let comment = |optional| ReportResult::AddComment { optional, option: b"2a".to_vec() };
        assert_eq!(results, [comment(true), comment(false), ReportResult::Reported]);
    }
}
