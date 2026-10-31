use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetContactNote<'a> {
    client: &'a Client,
    user_id: PeerRef,
    note: Text,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Empty text clears the note.
    pub fn set_contact_note(&self, user_id: impl Into<PeerRef>, note: impl Into<Text>) -> SetContactNote<'_> {
        SetContactNote { client: self, user_id: user_id.into(), note: note.into(), parse_mode: None }
    }
}

impl SetContactNote<'_> {
    async fn send(self) -> Result<()> {
        let id = self.client.resolve_user(&self.user_id).await?;
        let (text, entities) = self.client.write_text(self.note, self.parse_mode).await?;
        let note = base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities }));
        self.client.invoke(&functions::contacts::UpdateContactNote { id, note }).await?;
        Ok(())
    }
}

caption_setters!(SetContactNote);
into_future!(SetContactNote, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    fn seeded(replies: Vec<Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(replies);
        let mut peers = client.0.peers.lock().unwrap();
        let user = types::InputPeerUser { user_id: 7, access_hash: 2 };
        peers.insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 3 };
        peers.insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        drop(peers);
        (client, sent)
    }

    fn request(text: &str, entities: Vec<base::MessageEntity>) -> Vec<u8> {
        let id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 2 }));
        let note = types::TextWithEntities { text: text.into(), entities };
        functions::contacts::UpdateContactNote { id, note: base::TextWithEntities::TextWithEntities(Box::new(note)) }
            .to_bytes()
    }

    #[tokio::test]
    async fn a_formatted_note_goes_out_as_text_and_entities() {
        let (client, mut sent) = seeded(vec![Ok(true.to_bytes())]);
        client.set_contact_note(7, "<b>hi</b> there").html().await.unwrap();
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        assert_eq!(sent.recv().await.unwrap(), request("hi there", vec![bold]));
    }

    #[tokio::test]
    async fn empty_text_is_sent_to_clear_the_note() {
        let (client, mut sent) = seeded(vec![Ok(true.to_bytes())]);
        client.set_contact_note(7, "").await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), request("", Vec::new()));
    }

    #[tokio::test]
    async fn a_channel_has_no_contact_note() {
        let (client, mut sent) = seeded(Vec::new());
        let result = client.set_contact_note(get_channel_id(9), "note").await;
        assert!(matches!(result, Err(Error::PeerNotFound(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = seeded(vec![Err(Error::Rpc(RpcError::new(400, "CONTACT_MISSING")))]);
        let result = client.set_contact_note(7, "note").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("CONTACT_MISSING")), "{result:?}");
    }
}
