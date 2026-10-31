use crate::Client;
use crate::enums::MediaKind;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::InputMedia;

impl Client {
    pub async fn add_profile_audio(&self, media: &InputMedia<'_>) -> Result<()> {
        if media.kind != MediaKind::Audio {
            return Err(Error::InvalidArgument("profile music is an audio file"));
        }
        let peer = base::InputPeer::InputPeerSelf;
        let stored = self.upload_media(&peer, self.input_media(&peer, media, None, None).await?, None).await?;
        let base::InputMedia::InputMediaDocument(d) = stored else {
            return Err(Error::Unexpected("messages.uploadMedia returned no audio document"));
        };
        self.invoke(&functions::account::SaveMusic { unsave: false, id: d.id, after_id: None }).await?;
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{Audio, FileSource, InputMedia, Media, Upload};

    pub(crate) fn audio_attribute() -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
            voice: false,
            duration: 42,
            title: None,
            performer: None,
            waveform: None,
        }))
    }

    /// A song as a message carries it, with the stored reference `{id, id + 1, [3]}`.
    pub(crate) fn song(id: i64) -> Audio {
        let document = types::Document {
            id,
            access_hash: id + 1,
            file_reference: vec![3],
            date: 4,
            mime_type: "audio/mpeg".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![audio_attribute()],
        };
        let Some(Media::Audio(song)) = Media::from_document(&base::Document::Document(Box::new(document))) else {
            panic!("not a song")
        };
        song
    }

    pub(crate) fn stored(id: i64) -> base::InputDocument {
        base::InputDocument::InputDocument(Box::new(types::InputDocument {
            id,
            access_hash: id + 1,
            file_reference: vec![3],
        }))
    }

    fn save(id: base::InputDocument) -> Vec<u8> {
        functions::account::SaveMusic { unsave: false, id, after_id: None }.to_bytes()
    }

    #[tokio::test]
    async fn only_uploaded_or_stored_songs_are_accepted() {
        let (client, mut sent) = Client::recording();
        let url = FileSource::url("https://example.com/song.mp3").unwrap();
        let refused = [
            InputMedia::document("song.mp3"),
            InputMedia::voice(Upload::bytes("voice", b"x")),
            InputMedia::photo("cover.jpg"),
            InputMedia::audio(url),
        ];
        for media in &refused {
            let result = client.add_profile_audio(media).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_stored_song_is_saved_without_an_upload() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.add_profile_audio(&InputMedia::audio(&song(1))).await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), save(stored(1)));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_upload_goes_through_the_own_chat_before_it_is_saved() {
        let uploaded = document_media("audio/mpeg", vec![audio_attribute()]);
        let (client, mut sent) =
            Client::scripted(vec![Ok(true.to_bytes()), Ok(uploaded.to_bytes()), Ok(true.to_bytes())]);
        client.0.me.lock().unwrap().premium = Some(false);
        client.add_profile_audio(&InputMedia::audio(Upload::bytes("song.mp3", b"x"))).await.unwrap();
        let bodies: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        assert_eq!(bodies.len(), 3);
        assert_eq!(bodies[0][..4], functions::upload::SaveFilePart::CONSTRUCTOR_ID.to_le_bytes());
        assert_eq!(bodies[1][..4], functions::messages::UploadMedia::CONSTRUCTOR_ID.to_le_bytes());
        assert_eq!(bodies[1][8..12], base::InputPeer::InputPeerSelf.to_bytes()[..]);
        let audio = types::DocumentAttributeAudio::CONSTRUCTOR_ID.to_le_bytes();
        assert!(bodies[1].windows(4).any(|w| w == audio), "the upload lost its audio attribute");
        // document_media answers with the stored document {1, 2, [3]}.
        assert_eq!(bodies[2], save(stored(1)));
    }

    #[tokio::test]
    async fn an_upload_answer_without_a_document_is_not_saved() {
        let (client, mut sent) =
            Client::scripted(vec![Ok(true.to_bytes()), Ok(base::MessageMedia::MessageMediaEmpty.to_bytes())]);
        client.0.me.lock().unwrap().premium = Some(false);
        let result = client.add_profile_audio(&InputMedia::audio(Upload::bytes("song.mp3", b"x"))).await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
        assert_eq!(std::iter::from_fn(|| sent.try_recv().ok()).count(), 2);
    }
}
