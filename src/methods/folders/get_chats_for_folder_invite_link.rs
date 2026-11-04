use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::utils::get_peer;

impl Client {
    /// The chats of folder `folder_id` that an invite link can share: its supergroups and channels, pinned ones
    /// first. A folder with excluded chats or chat type flags can't be shared, so it offers none.
    pub async fn get_chats_for_folder_invite_link(&self, folder_id: i32) -> Result<Vec<i64>> {
        let folders = self.get_folders().await?;
        let folder = folders
            .into_iter()
            .find(|f| f.id == folder_id)
            .ok_or(Error::InvalidArgument("there is no folder with this id"))?;
        let filtered = !folder.excluded_chats.is_empty()
            || folder.contacts
            || folder.non_contacts
            || folder.groups
            || folder.channels
            || folder.bots
            || folder.exclude_muted
            || folder.exclude_read
            || folder.exclude_archived;
        if filtered {
            return Ok(Vec::new());
        }
        let shareable = |id: &i64| matches!(get_peer(*id), base::Peer::PeerChannel(_));
        Ok(folder.pinned_chats.into_iter().chain(folder.included_chats).filter(shareable).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    fn channel(channel_id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id, access_hash: 1 }))
    }

    fn folder(pinned: Vec<base::InputPeer>, included: Vec<base::InputPeer>, excluded: Vec<base::InputPeer>) -> Vec<u8> {
        let title = types::TextWithEntities { text: "Work".into(), entities: Vec::new() };
        let filter = types::DialogFilter {
            contacts: false,
            non_contacts: false,
            groups: false,
            broadcasts: false,
            bots: false,
            exclude_muted: false,
            exclude_read: false,
            exclude_archived: false,
            title_noanimate: false,
            id: 4,
            title: base::TextWithEntities::TextWithEntities(Box::new(title)),
            emoticon: None,
            color: None,
            pinned_peers: pinned,
            include_peers: included,
            exclude_peers: excluded,
        };
        let filters = vec![base::DialogFilter::DialogFilterDefault, base::DialogFilter::DialogFilter(Box::new(filter))];
        let reply = types::messages::DialogFilters { tags_enabled: false, filters };
        base::messages::DialogFilters::DialogFilters(Box::new(reply)).to_bytes()
    }

    #[tokio::test]
    async fn only_supergroups_and_channels_are_offered() {
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 2 }));
        let group = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 3 });
        let reply = folder(vec![channel(9)], vec![group, user, channel(8)], Vec::new());
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        let chats = client.get_chats_for_folder_invite_link(4).await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), functions::messages::GetDialogFilters {}.to_bytes());
        assert_eq!(chats, [get_channel_id(9), get_channel_id(8)]);
    }

    #[tokio::test]
    async fn a_folder_with_exclusions_offers_nothing() {
        let (client, _sent) = Client::scripted(vec![Ok(folder(vec![channel(9)], vec![channel(8)], vec![channel(7)]))]);
        assert_eq!(client.get_chats_for_folder_invite_link(4).await.unwrap(), Vec::<i64>::new());
    }

    #[tokio::test]
    async fn an_unknown_folder_is_an_invalid_argument() {
        let (client, _sent) = Client::scripted(vec![Ok(folder(vec![channel(9)], Vec::new(), Vec::new()))]);
        let result = client.get_chats_for_folder_invite_link(5).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
    }
}
