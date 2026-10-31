use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::Audio;
use crate::types::messages_and_media::input_document;

impl Client {
    /// None moves it to the top.
    pub async fn set_profile_audio_position(&self, audio: &Audio, after: Option<&Audio>) -> Result<()> {
        if after.is_some_and(|a| a.id == audio.id) {
            return Err(Error::InvalidArgument("a song can't be placed after itself"));
        }
        let request = functions::account::SaveMusic {
            unsave: false,
            id: input_document(&audio.location),
            after_id: after.map(|a| input_document(&a.location)),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::add_profile_audio::tests::{song, stored};
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn moves_the_song_to_the_top_or_after_another() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.set_profile_audio_position(&song(1), None).await.unwrap();
        let top = functions::account::SaveMusic { unsave: false, id: stored(1), after_id: None };
        assert_eq!(sent.recv().await.unwrap(), top.to_bytes());

        client.set_profile_audio_position(&song(1), Some(&song(11))).await.unwrap();
        let after = functions::account::SaveMusic { unsave: false, id: stored(1), after_id: Some(stored(11)) };
        assert_eq!(sent.recv().await.unwrap(), after.to_bytes());
    }

    #[tokio::test]
    async fn a_song_after_itself_is_refused_before_sending() {
        let (client, mut sent) = Client::recording();
        let result = client.set_profile_audio_position(&song(1), Some(&song(1))).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "DOCUMENT_INVALID")))]);
        let result = client.set_profile_audio_position(&song(1), None).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("DOCUMENT_INVALID")), "{result:?}");
    }
}
