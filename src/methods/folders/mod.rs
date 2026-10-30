mod create_folder;
mod delete_folder;
mod edit_folder;
mod get_folders;
mod reorder_folders;

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};
    use crate::types::Folder;
    use crate::utils::get_channel_id;

    fn title(text: &str) -> base::TextWithEntities {
        let title = types::TextWithEntities { text: text.into(), entities: Vec::new() };
        base::TextWithEntities::TextWithEntities(Box::new(title))
    }

    #[tokio::test]
    async fn folders_round_trip_through_the_peer_cache() {
        let client = Client::offline();
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let channel = base::InputPeer::InputPeerChannel(Box::new(channel));
        let chat = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 3 });
        let raw = types::DialogFilter {
            contacts: true,
            non_contacts: false,
            groups: false,
            broadcasts: true,
            bots: false,
            exclude_muted: true,
            exclude_read: false,
            exclude_archived: false,
            title_noanimate: false,
            id: 4,
            title: title("Work"),
            emoticon: Some("💼".into()),
            color: None,
            pinned_peers: vec![channel.clone()],
            include_peers: vec![channel, chat],
            exclude_peers: vec![base::InputPeer::InputPeerEmpty],
        };
        let folder = Folder::parse(&client, base::DialogFilter::DialogFilter(Box::new(raw))).unwrap();
        let channel_id = get_channel_id(9);
        assert_eq!(
            (folder.id, folder.title.as_str(), folder.channels, folder.excluded_chats.is_empty()),
            (4, "Work", true, true)
        );
        assert_eq!((&folder.pinned_chats, &folder.included_chats), (&vec![channel_id], &vec![channel_id, -3]));

        // The channel's access hash came with the folder, so writing it back needs no request.
        let base::DialogFilter::DialogFilter(back) = folder.write(&client).await.unwrap() else {
            panic!("not a plain folder")
        };
        assert_eq!(
            (back.include_peers.len(), Folder::parse(&client, base::DialogFilter::DialogFilterDefault)),
            (2, None)
        );
    }

    #[tokio::test]
    async fn titles_keep_custom_emoji_and_animation_flag() {
        let client = Client::offline();
        let emoji = types::MessageEntityCustomEmoji { offset: 0, length: 2, document_id: 77 };
        let bold = types::MessageEntityBold { offset: 3, length: 4 };
        let title = types::TextWithEntities {
            text: "🙂 Work".into(),
            entities: vec![
                base::MessageEntity::MessageEntityCustomEmoji(Box::new(emoji)),
                base::MessageEntity::MessageEntityBold(bold),
            ],
        };
        let raw = types::DialogFilterChatlist {
            has_my_invites: false,
            title_noanimate: true,
            id: 5,
            title: base::TextWithEntities::TextWithEntities(Box::new(title.clone())),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: Vec::new(),
        };
        let folder = Folder::parse(&client, base::DialogFilter::DialogFilterChatlist(Box::new(raw.clone()))).unwrap();
        assert_eq!(folder.title_entities[0].custom_emoji_id, Some(77));
        let back = folder.write(&client).await.unwrap();
        assert_eq!(back, base::DialogFilter::DialogFilterChatlist(Box::new(raw)));
    }

    #[tokio::test]
    async fn every_title_entity_kind_is_written_back() {
        use base::MessageEntity as E;
        let client = Client::offline();
        let (offset, length) = (0, 1);
        let date = types::MessageEntityFormattedDate {
            relative: false,
            short_time: true,
            long_time: false,
            short_date: false,
            long_date: true,
            day_of_week: true,
            offset,
            length,
            date: 9,
        };
        let quote = types::MessageEntityBlockquote { collapsed: true, offset, length };
        let entities = vec![
            E::MessageEntityUnknown(types::MessageEntityUnknown { offset, length }),
            E::MessageEntityMention(types::MessageEntityMention { offset, length }),
            E::MessageEntityHashtag(types::MessageEntityHashtag { offset, length }),
            E::MessageEntityBotCommand(types::MessageEntityBotCommand { offset, length }),
            E::MessageEntityUrl(types::MessageEntityUrl { offset, length }),
            E::MessageEntityEmail(types::MessageEntityEmail { offset, length }),
            E::MessageEntityBold(types::MessageEntityBold { offset, length }),
            E::MessageEntityItalic(types::MessageEntityItalic { offset, length }),
            E::MessageEntityCode(types::MessageEntityCode { offset, length }),
            E::MessageEntityPre(Box::new(types::MessageEntityPre { offset, length, language: "rust".into() })),
            E::MessageEntityTextUrl(Box::new(types::MessageEntityTextUrl { offset, length, url: "https://a".into() })),
            E::MessageEntityPhone(types::MessageEntityPhone { offset, length }),
            E::MessageEntityCashtag(types::MessageEntityCashtag { offset, length }),
            E::MessageEntityUnderline(types::MessageEntityUnderline { offset, length }),
            E::MessageEntityStrike(types::MessageEntityStrike { offset, length }),
            E::MessageEntityBankCard(types::MessageEntityBankCard { offset, length }),
            E::MessageEntitySpoiler(types::MessageEntitySpoiler { offset, length }),
            E::MessageEntityCustomEmoji(Box::new(types::MessageEntityCustomEmoji { offset, length, document_id: 7 })),
            E::MessageEntityBlockquote(Box::new(quote)),
            E::MessageEntityFormattedDate(Box::new(date)),
            E::MessageEntityDiffInsert(types::MessageEntityDiffInsert { offset, length }),
            E::MessageEntityDiffReplace(Box::new(types::MessageEntityDiffReplace {
                offset,
                length,
                old_text: "was".into(),
            })),
            E::MessageEntityDiffDelete(types::MessageEntityDiffDelete { offset, length }),
            E::MessageEntityTonAddress(types::MessageEntityTonAddress { offset, length }),
        ];
        let title = types::TextWithEntities { text: "x".into(), entities };
        let raw = types::DialogFilterChatlist {
            has_my_invites: false,
            title_noanimate: false,
            id: 6,
            title: base::TextWithEntities::TextWithEntities(Box::new(title)),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: Vec::new(),
        };
        let folder = Folder::parse(&client, base::DialogFilter::DialogFilterChatlist(Box::new(raw.clone()))).unwrap();
        assert_eq!(folder.write(&client).await.unwrap(), base::DialogFilter::DialogFilterChatlist(Box::new(raw)));
    }

    #[tokio::test]
    async fn reordering_no_folders_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.reorder_folders(&[]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn shareable_folders_write_as_chatlists() {
        let client = Client::offline();
        let folder = Folder { id: 2, title: "Shared".into(), is_shareable: true, ..Folder::default() };
        let raw = folder.write(&client).await.unwrap();
        assert!(matches!(raw, base::DialogFilter::DialogFilterChatlist(_)));
    }
}
