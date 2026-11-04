use crate::Client;
use crate::errors::Result;
use crate::methods::pages::DialogPages;
use crate::raw::{base, functions};
use crate::types::Dialog;
use crate::utils::get_peer_id;

pub struct GetDialogs<'a> {
    client: &'a Client,
    folder_id: Option<i32>,
    // The count of the last page, or what a one-dialog request said before any.
    total: Option<i32>,
    pages: DialogPages<Dialog>,
}

impl Client {
    pub fn get_dialogs(&self) -> GetDialogs<'_> {
        GetDialogs { client: self, folder_id: None, total: None, pages: DialogPages::new() }
    }
}

impl GetDialogs<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub fn chat_list(mut self, folder_id: i32) -> Self {
        self.folder_id = Some(folder_id);
        self
    }

    /// How many dialogs there are in the list, whatever `limit` says: the count Telegram gave with the last page,
    /// or, before any page, from one request for a single dialog. Either way it's asked for once.
    pub async fn total(&mut self) -> Result<i32> {
        if let Some(total) = self.total {
            return Ok(total);
        }
        let request = self.request(1, false);
        let (dialogs, count) = Dialog::unpack(self.client, self.client.invoke(&request).await?);
        let total = count.unwrap_or(dialogs.len() as i32);
        self.total = Some(total);
        Ok(total)
    }

    pub async fn next(&mut self) -> Result<Option<Dialog>> {
        loop {
            if let Some(dialog) = self.pages.pop() {
                return Ok(Some(dialog));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = self.request(limit, true);
            let page = self.client.invoke(&request).await?;
            let (last_page, next) = page_end(&page, limit);
            let (dialogs, count) = Dialog::unpack(self.client, page);
            // Only a slice says how many there are; a whole list that follows one is just the last few.
            if let Some(count) = count {
                self.total = Some(count);
            } else if self.total.is_none() {
                self.total = Some(dialogs.len() as i32);
            }
            self.pages.refill(self.client, dialogs, last_page, next).await?;
        }
    }

    /// `paged` continues from the last page; without it the request starts at the top of the list.
    fn request(&self, limit: i32, paged: bool) -> functions::messages::GetDialogs {
        let (offset_date, offset_id, offset_peer) = if paged {
            (self.pages.offset_date, self.pages.offset_id, self.pages.offset_peer.clone())
        } else {
            (0, 0, base::InputPeer::InputPeerEmpty)
        };
        functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id: self.folder_id,
            offset_date,
            offset_id,
            offset_peer,
            limit,
            hash: 0,
        }
    }
}

// Read the raw page: the archive folder entry is dropped while parsing but still fills the limit, and a chat whose
// last message is a service message vaegram doesn't parse still marks where the page ends.
fn page_end(page: &base::messages::Dialogs, limit: i32) -> (bool, Option<(i32, i32, i64)>) {
    let base::messages::Dialogs::DialogsSlice(d) = page else { return (true, None) };
    // Communities have no peer or top message, so the cursor comes from the last chat or folder. Trailing communities
    // then come back on the next page, but parsing drops them, so no dialog is returned twice.
    let last = d.dialogs.iter().rev().find_map(|x| match x {
        base::Dialog::Dialog(c) => Some((&c.peer, c.top_message)),
        base::Dialog::DialogFolder(f) => Some((&f.peer, f.top_message)),
        base::Dialog::DialogCommunity(_) => None,
    });
    let next = last.and_then(|(peer, top)| {
        let chat_id = get_peer_id(peer);
        // Channel message ids overlap, so the chat has to match too.
        let date = d.messages.iter().find_map(|m| match m {
            base::Message::Message(m) if m.id == top && get_peer_id(&m.peer_id) == chat_id => Some(m.date),
            base::Message::MessageService(m) if m.id == top && get_peer_id(&m.peer_id) == chat_id => Some(m.date),
            _ => None,
        })?;
        Some((date, top, chat_id))
    });
    (d.dialogs.len() < limit as usize, next)
}

#[cfg(test)]
mod tests {
    use super::page_end;
    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::message::tests::service;

    fn peer(chat_id: i64) -> base::Peer {
        base::Peer::PeerChat(types::PeerChat { chat_id })
    }

    fn notify_settings() -> base::PeerNotifySettings {
        base::PeerNotifySettings::PeerNotifySettings(Box::new(types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        }))
    }

    fn unparsed(chat_id: i64, id: i32, date: i32) -> base::Message {
        let m = types::MessageService {
            id,
            peer_id: peer(chat_id),
            date,
            ..service(base::MessageAction::MessageActionEmpty)
        };
        base::Message::MessageService(Box::new(m))
    }

    fn slice(dialogs: usize) -> base::messages::Dialogs {
        let folder = types::DialogFolder {
            pinned: true,
            folder: base::Folder::Folder(Box::new(types::Folder {
                autofill_new_broadcasts: false,
                autofill_public_groups: false,
                autofill_new_correspondents: false,
                id: 1,
                title: "Archive".into(),
                photo: None,
            })),
            peer: peer(1),
            top_message: 1,
            unread_muted_peers_count: 0,
            unread_unmuted_peers_count: 0,
            unread_muted_messages_count: 0,
            unread_unmuted_messages_count: 0,
        };
        let page = types::messages::DialogsSlice {
            count: 500,
            dialogs: vec![base::Dialog::DialogFolder(Box::new(folder)); dialogs],
            messages: vec![unparsed(1, 1, 50)],
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Dialogs::DialogsSlice(Box::new(page))
    }

    fn whole() -> base::messages::Dialogs {
        let page = types::messages::Dialogs {
            dialogs: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Dialogs::Dialogs(Box::new(page))
    }

    #[test]
    fn raw_entries_end_the_page() {
        assert_eq!(page_end(&slice(2), 2), (false, Some((50, 1, -1))));
        assert_eq!(page_end(&slice(1), 2), (true, Some((50, 1, -1))));
        assert_eq!(page_end(&whole(), 2), (true, None));

        let base::messages::Dialogs::DialogsSlice(mut ends_in_community) = slice(1) else { unreachable!() };
        let community = types::DialogCommunity { pinned: false, community_id: 3, notify_settings: notify_settings() };
        ends_in_community.dialogs.push(base::Dialog::DialogCommunity(Box::new(community)));
        let ends_in_community = base::messages::Dialogs::DialogsSlice(ends_in_community);
        assert_eq!(page_end(&ends_in_community, 2), (false, Some((50, 1, -1))));
    }

    #[tokio::test]
    async fn a_chat_ending_in_an_unparsed_service_message_still_moves_the_offset() {
        let dialog = types::Dialog {
            pinned: false,
            unread_mark: false,
            view_forum_as_messages: false,
            peer: peer(2),
            top_message: 7,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            notify_settings: notify_settings(),
            pts: None,
            draft: None,
            folder_id: None,
            ttl_period: None,
        };
        let base::messages::Dialogs::DialogsSlice(mut first) = slice(1) else { unreachable!() };
        first.dialogs.push(base::Dialog::Dialog(Box::new(dialog)));
        first.messages.push(unparsed(2, 7, 90));
        let replies = [base::messages::Dialogs::DialogsSlice(first), whole()];
        let (client, mut rx) = Client::scripted(replies.iter().map(|r| Ok(r.to_bytes())).collect());

        let mut dialogs = client.get_dialogs().limit(2);
        while dialogs.next().await.unwrap().is_some() {}
        rx.try_recv().unwrap();
        let next = messages::GetDialogs {
            exclude_pinned: false,
            folder_id: None,
            offset_date: 90,
            offset_id: 7,
            offset_peer: base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 2 }),
            limit: 1,
            hash: 0,
        };
        assert_eq!(rx.try_recv().ok(), Some(next.to_bytes()));
    }

    #[tokio::test]
    async fn total_reuses_the_fetched_page() {
        let (client, mut rx) = Client::scripted(vec![Ok(slice(1).to_bytes())]);
        let mut dialogs = client.get_dialogs().limit(2);
        assert!(dialogs.next().await.unwrap().is_none());
        assert_eq!(dialogs.total().await.unwrap(), 500);
        assert_eq!(dialogs.total().await.unwrap(), 500);
        assert!(rx.try_recv().is_ok() && rx.try_recv().is_err(), "total asked again");
    }

    #[tokio::test]
    async fn total_before_next_makes_one_request() {
        let (client, mut rx) = Client::scripted(vec![Ok(slice(1).to_bytes())]);
        let mut dialogs = client.get_dialogs().chat_list(1);
        assert_eq!(dialogs.total().await.unwrap(), 500);
        assert_eq!(dialogs.total().await.unwrap(), 500);
        let asked = messages::GetDialogs {
            exclude_pinned: false,
            folder_id: Some(1),
            offset_date: 0,
            offset_id: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            limit: 1,
            hash: 0,
        };
        assert_eq!(rx.try_recv().ok(), Some(asked.to_bytes()));
        assert!(rx.try_recv().is_err(), "total asked twice");
    }
}
