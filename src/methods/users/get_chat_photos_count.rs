use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::base;
use crate::types::PeerRef;

impl Client {
    /// How many profile photos a user has, or how many photos a group or channel has had. A bot can't search a
    /// chat's history, so for a group or channel it gets 1 when the chat has a photo now and 0 when it doesn't.
    pub async fn get_chat_photos_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        let chat = chat_id.into();
        if self.peer_id(chat.clone()).await? > 0 {
            return self.get_chat_photos(chat).limit(1).total().await;
        }
        let is_bot = self.0.me.lock().unwrap().is_bot;
        if !is_bot {
            return self.search_messages_count(chat).filter(MessagesFilter::ChatPhoto).await;
        }
        let (full, _) = self.full_chat(self.resolve_chat(&chat).await?).await?;
        let photo = match &full {
            base::ChatFull::ChatFull(f) => f.chat_photo.as_ref(),
            base::ChatFull::ChannelFull(f) => Some(&f.chat_photo),
            base::ChatFull::CommunityFull(f) => Some(&f.chat_photo),
        };
        Ok(i32::from(matches!(photo, Some(base::Photo::Photo(_)))))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::{channels, messages, photos};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::utils::get_channel_id;

    fn known_channel(client: &Client) -> i64 {
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(peer)));
        get_channel_id(9)
    }

    // messages.chatFull wrapping a channelFull with only its required fields.
    fn channel_full(photo: base::Photo) -> Vec<u8> {
        let mut buf = Vec::new();
        types::messages::ChatFull::CONSTRUCTOR_ID.serialize(&mut buf);
        types::ChannelFull::CONSTRUCTOR_ID.serialize(&mut buf);
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        9i64.serialize(&mut buf);
        String::new().serialize(&mut buf);
        for n in [0i32, 0, 0] {
            n.serialize(&mut buf);
        }
        photo.serialize(&mut buf);
        types::PeerNotifySettings::CONSTRUCTOR_ID.serialize(&mut buf);
        0u32.serialize(&mut buf);
        Vec::<i64>::new().serialize(&mut buf);
        0i32.serialize(&mut buf);
        Vec::<i64>::new().serialize(&mut buf);
        Vec::<i64>::new().serialize(&mut buf);
        buf
    }

    fn photo() -> base::Photo {
        let photo = types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 0,
            sizes: Vec::new(),
            video_sizes: None,
            dc_id: 2,
        };
        base::Photo::Photo(Box::new(photo))
    }

    #[tokio::test]
    async fn a_users_photos_are_counted_from_one_slice() {
        let slice = types::photos::PhotosSlice { count: 5, photos: vec![photo()], users: Vec::new() };
        let (client, mut sent) =
            Client::scripted(vec![Ok(base::photos::Photos::PhotosSlice(Box::new(slice)).to_bytes())]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        assert_eq!(client.get_chat_photos_count(7).await.unwrap(), 5);
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 3 }));
        let expected = photos::GetUserPhotos { user_id, offset: 0, max_id: 0, limit: 1 };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn a_bot_counts_the_groups_photo() {
        let empty = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 });
        let (client, mut sent) = Client::scripted(vec![Ok(channel_full(photo())), Ok(channel_full(empty))]);
        let channel = known_channel(&client);
        assert_eq!(client.get_chat_photos_count(channel).await.unwrap(), 1);
        assert_eq!(client.get_chat_photos_count(channel).await.unwrap(), 0);
        let input = types::InputChannel { channel_id: 9, access_hash: 1 };
        let full = channels::GetFullChannel { channel: base::InputChannel::InputChannel(Box::new(input)) };
        assert_eq!(sent.try_recv().unwrap(), full.to_bytes());
        assert_eq!(sent.try_recv().unwrap(), full.to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_user_counts_photo_changes_in_a_group() {
        let counter = types::messages::SearchCounter {
            inexact: false,
            filter: base::MessagesFilter::InputMessagesFilterChatPhotos,
            count: 3,
        };
        let counters = vec![base::messages::SearchCounter::SearchCounter(Box::new(counter))];
        let (client, mut sent) = Client::scripted(vec![Ok(counters.to_bytes())]);
        client.0.me.lock().unwrap().is_bot = false;
        let channel = known_channel(&client);
        assert_eq!(client.get_chat_photos_count(channel).await.unwrap(), 3);
        let peer =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        let expected = messages::GetSearchCounters {
            peer,
            saved_peer_id: None,
            top_msg_id: None,
            filters: vec![base::MessagesFilter::InputMessagesFilterChatPhotos],
        };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }
}
