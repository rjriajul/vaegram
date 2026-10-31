use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Audio;
use crate::types::messages_and_media::input_document;

impl Client {
    pub async fn remove_profile_audio(&self, audio: &Audio) -> Result<()> {
        let request =
            functions::account::SaveMusic { unsave: true, id: input_document(&audio.location), after_id: None };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::add_profile_audio::tests::{song, stored};
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn unsaves_the_song() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.remove_profile_audio(&song(1)).await.unwrap();
        let expected = functions::account::SaveMusic { unsave: true, id: stored(1), after_id: None };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }
}
