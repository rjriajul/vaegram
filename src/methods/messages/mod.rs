mod approve_suggested_post;
mod clear_all_drafts;
mod copy_message;
mod copy_messages;
mod decline_suggested_post;
mod delete_all_message_reactions;
mod delete_ephemeral_message;
mod delete_message_reaction;
mod delete_messages;
mod delete_saved_history;
mod delete_scheduled_messages;
mod download_media;
mod edit_ephemeral_message_caption;
mod edit_ephemeral_message_media;
mod edit_ephemeral_message_reply_markup;
mod edit_ephemeral_message_text;
mod edit_inline_media;
mod edit_inline_reply_markup;
mod edit_inline_text;
mod edit_message_caption;
mod edit_message_checklist;
mod edit_message_live_location;
mod edit_message_media;
mod edit_message_reply_markup;
mod edit_message_text;
mod forward_messages;
mod get_all_drafts;
mod get_chat_history;
mod get_chat_history_count;
mod get_custom_emoji_stickers;
mod get_message_read_date;
mod get_messages;
mod get_messages_views;
mod get_saved_dialogs;
mod get_saved_history;
mod get_scheduled_messages;
mod get_user_personal_chat_messages;
mod pages;
mod pin_chat_message;
mod pin_saved_dialog;
mod read_chat_history;
mod read_mentions;
mod read_reactions;
mod save_draft;
mod search_messages;
mod send_animation;
mod send_audio;
mod send_chat_action;
mod send_checklist;
mod send_contact;
mod send_dice;
mod send_document;
mod send_ephemeral_message;
mod send_live_photo;
mod send_location;
mod send_media_group;
mod send_message;
mod send_message_draft;
mod send_options;
mod send_paid_media;
mod send_photo;
mod send_poll;
mod send_reaction;
mod send_rich_message;
mod send_scheduled_now;
mod send_sticker;
mod send_venue;
mod send_video;
mod send_video_note;
mod send_voice;
mod stop_message_live_location;
mod stop_poll;
mod stream_message;
mod unpin_all_chat_messages;
mod unpin_chat_message;
mod unpin_saved_dialog;
mod upload_media;

pub use approve_suggested_post::ApproveSuggestedPost;
pub use copy_message::CopyMessage;
pub use copy_messages::CopyMessages;
pub use decline_suggested_post::DeclineSuggestedPost;
pub(crate) use download_media::DownloadSource;
pub use download_media::{DownloadMedia, MediaStream};
pub use edit_ephemeral_message_text::EditEphemeralMessage;
pub use edit_inline_media::EditInlineMedia;
pub use edit_inline_text::EditInlineText;
pub use edit_message_checklist::EditMessageChecklist;
pub use edit_message_live_location::EditMessageLiveLocation;
pub use edit_message_media::EditMessageMedia;
pub use edit_message_reply_markup::EditMessageReplyMarkup;
pub use edit_message_text::EditMessageText;
pub use forward_messages::ForwardMessages;
pub use get_chat_history::GetChatHistory;
pub use get_saved_dialogs::GetSavedDialogs;
pub use get_saved_history::GetSavedHistory;
pub use pin_chat_message::PinChatMessage;
pub use save_draft::SaveDraft;
pub use search_messages::SearchMessages;
pub use send_animation::SendAnimation;
pub use send_audio::SendAudio;
pub use send_chat_action::SendChatAction;
pub use send_checklist::SendChecklist;
pub use send_contact::SendContact;
pub use send_dice::SendDice;
pub use send_document::SendDocument;
pub use send_ephemeral_message::SendEphemeralMessage;
pub use send_live_photo::SendLivePhoto;
pub use send_location::SendLocation;
pub(crate) use send_location::geo_point;
pub use send_media_group::SendMediaGroup;
pub use send_message::SendMessage;
pub use send_message_draft::SendMessageDraft;
pub(crate) use send_options::{SendOptions, caption_setters, send_builder};
pub use send_paid_media::SendPaidMedia;
pub use send_photo::SendPhoto;
pub use send_poll::SendPoll;
pub use send_reaction::SendReaction;
pub use send_rich_message::SendRichMessage;
pub use send_sticker::SendSticker;
pub use send_venue::SendVenue;
pub(crate) use send_venue::input_venue;
pub use send_video::SendVideo;
pub use send_video_note::SendVideoNote;
pub use send_voice::SendVoice;
pub use stop_message_live_location::StopMessageLiveLocation;
pub use stop_poll::StopPoll;
pub(crate) use stream_message::StreamShared;
pub use stream_message::{MessageStream, StreamMessage};
pub use unpin_chat_message::UnpinChatMessage;

#[cfg(test)]
mod saved_tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::session::Affected;
    use crate::types::messages_and_media::message::tests::header;
    use crate::types::{Draft, Message, MessageViews};
    use crate::utils::get_peer;

    const USER: i64 = 7;

    fn setup(answers: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(answers.into_iter().map(Ok).collect());
        let peer = types::InputPeerUser { user_id: USER, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(peer)));
        (client, rx)
    }

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }))
    }

    async fn sent<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    fn constructor(body: &[u8]) -> u32 {
        u32::from_le_bytes(body[..4].try_into().unwrap())
    }

    fn affected(offset: i32) -> Vec<u8> {
        let affected = types::messages::AffectedHistory { pts: 1, pts_count: 1, offset };
        base::messages::AffectedHistory::AffectedHistory(Box::new(affected)).to_bytes()
    }

    #[tokio::test]
    async fn scheduled_requests() {
        let (client, mut rx) = setup(Vec::new());
        let rx = &mut rx;

        let body = sent(rx, client.get_scheduled_messages(USER)).await;
        assert_eq!(body, messages::GetScheduledHistory { peer: user(), hash: 0 }.to_bytes());
        let body = sent(rx, client.send_scheduled_now(USER, &[3])).await;
        assert_eq!(body, messages::SendScheduledMessages { peer: user(), id: vec![3] }.to_bytes());
        let body = sent(rx, client.delete_scheduled_messages(USER, &[3])).await;
        assert_eq!(body, messages::DeleteScheduledMessages { peer: user(), id: vec![3] }.to_bytes());

        let body = sent(rx, client.edit_message_text(USER, 3, "later").schedule_date(1_900_000_000)).await;
        let expected = messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer: user(),
            id: 3,
            message: Some("later".into()),
            media: None,
            reply_markup: None,
            entities: None,
            schedule_date: Some(1_900_000_000),
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
            rich_message: None,
        };
        assert_eq!(body, expected.to_bytes());
    }

    #[tokio::test]
    async fn save_draft_request() {
        let (client, mut rx) = setup(Vec::new());
        let body = sent(&mut rx, client.save_draft(USER, "hi").reply_to(3)).await;
        let reply = types::InputReplyToMessage {
            reply_to_msg_id: 3,
            top_msg_id: None,
            reply_to_peer_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            monoforum_peer_id: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        };
        let expected = messages::SaveDraft {
            no_webpage: false,
            invert_media: false,
            reply_to: Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply))),
            peer: user(),
            message: "hi".into(),
            entities: None,
            media: None,
            effect: None,
            suggested_post: None,
            rich_message: None,
        };
        assert_eq!(body, expected.to_bytes());
    }

    #[tokio::test]
    async fn batched_history_calls_repeat_until_done() {
        let (client, mut rx) = setup(vec![affected(100), affected(0), affected(0), affected(0)]);
        client.read_mentions(USER).await.unwrap();
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::ReadMentions::CONSTRUCTOR_ID);
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::ReadMentions::CONSTRUCTOR_ID);

        client.read_reactions(USER).await.unwrap();
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::ReadReactions::CONSTRUCTOR_ID);
        client.delete_saved_history(USER).await.unwrap();
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::DeleteSavedHistory::CONSTRUCTOR_ID);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn history_reads_move_the_pts() {
        let (client, _rx) = setup(vec![affected(0), affected(0), affected(0)]);
        let moved = Some(Affected { channel_id: None, pts: 1, pts_count: 1 });
        client.read_mentions(USER).await.unwrap();
        assert_eq!(client.0.events.take_affected(), moved);
        client.read_reactions(USER).await.unwrap();
        assert_eq!(client.0.events.take_affected(), moved);
        client.delete_saved_history(USER).await.unwrap();
        assert_eq!(client.0.events.take_affected(), moved);
    }

    #[tokio::test]
    async fn parses_views() {
        let views = |views, replies: Option<i32>| {
            let replies = replies.map(|replies| {
                base::MessageReplies::MessageReplies(Box::new(types::MessageReplies {
                    comments: false,
                    replies,
                    replies_pts: 0,
                    recent_repliers: None,
                    channel_id: None,
                    max_id: None,
                    read_max_id: None,
                }))
            });
            base::MessageViews::MessageViews(Box::new(types::MessageViews { views, forwards: Some(1), replies }))
        };
        let answer = types::messages::MessageViews {
            views: vec![views(Some(10), Some(2)), views(None, None)],
            chats: Vec::new(),
            users: Vec::new(),
        };
        let answer = base::messages::MessageViews::MessageViews(Box::new(answer)).to_bytes();
        let (client, mut rx) = setup(vec![answer]);

        let found = client.get_messages_views(USER, &[1, 2]).await.unwrap();
        let expected = [
            MessageViews { views: Some(10), forwards: Some(1), replies: Some(2) },
            MessageViews { views: None, forwards: Some(1), replies: None },
        ];
        assert_eq!(found, expected);
        let request = messages::GetMessagesViews { peer: user(), id: vec![1, 2], increment: false };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn empty_id_lists_are_refused_without_a_request() {
        let (client, mut rx) = setup(Vec::new());
        let refused = |r: Result<()>| matches!(r, Err(Error::InvalidArgument(_)));
        let deleted = refused(client.delete_scheduled_messages(USER, &[]).await);
        let sent_now = refused(client.send_scheduled_now(USER, &[]).await.map(drop));
        let views = refused(client.get_messages_views(USER, &[]).await.map(drop));
        assert_eq!((deleted, sent_now, views, rx.try_recv().is_err()), (true, true, true, true));
    }

    #[tokio::test]
    async fn an_unread_message_has_no_read_date() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "MESSAGE_NOT_READ_YET")))]);
        client.0.peers.lock().unwrap().insert(USER, user());
        assert_eq!(client.get_message_read_date(USER, 3).await.ok(), Some(None));
    }

    #[tokio::test]
    async fn a_scheduled_send_returns_the_scheduled_message() {
        let message = base::Message::Message(Box::new(Message::bare(3, get_peer(USER), 1_900_000_000)));
        let update = base::Update::UpdateNewScheduledMessage(Box::new(types::UpdateNewScheduledMessage { message }));
        let updates = types::Updates { updates: vec![update], users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
        let (client, _rx) = setup(vec![base::Updates::Updates(Box::new(updates)).to_bytes()]);
        let sent = client.send_message(USER, "later").schedule_date(1_900_000_000).await;
        assert_eq!(sent.map(|m| m.id).ok(), Some(3));
    }

    #[tokio::test]
    async fn parses_drafts() {
        let draft = |message: &str| {
            let reply_to = types::InputReplyToMessage {
                reply_to_msg_id: 5,
                top_msg_id: None,
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
                quote_rich: None,
            };
            base::DraftMessage::DraftMessage(Box::new(types::DraftMessage {
                no_webpage: false,
                invert_media: false,
                reply_to: Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply_to))),
                message: message.into(),
                entities: None,
                media: None,
                date: 100,
                effect: None,
                suggested_post: None,
                rich_message: None,
            }))
        };
        let update = |peer, top_msg_id, draft| {
            base::Update::UpdateDraftMessage(Box::new(types::UpdateDraftMessage {
                peer,
                top_msg_id,
                saved_peer_id: None,
                draft,
            }))
        };
        let user_peer = base::Peer::PeerUser(types::PeerUser { user_id: USER });
        let group = base::Peer::PeerChat(types::PeerChat { chat_id: 9 });
        let cleared = base::DraftMessage::DraftMessageEmpty(types::DraftMessageEmpty { date: None });
        let updates = types::Updates {
            updates: vec![
                update(user_peer, None, draft("hello")),
                update(group.clone(), Some(4), draft("topic")),
                update(group, None, cleared),
            ],
            users: Vec::new(),
            chats: Vec::new(),
            date: 0,
            seq: 0,
        };
        let (client, _rx) = setup(vec![base::Updates::Updates(Box::new(updates)).to_bytes()]);

        let drafts = client.get_all_drafts().await.unwrap();
        let drafts: Vec<_> = drafts.into_iter().map(|(chat, draft)| (chat.id, draft)).collect();
        let saved = |text: &str, topic_id| Draft {
            text: text.into(),
            entities: None,
            reply_to_message_id: Some(5),
            topic_id,
            date: 100,
        };
        let expected = [(USER, saved("hello", None)), (-9, saved("topic", Some(4)))];
        assert_eq!(drafts, expected);
    }

    #[tokio::test]
    async fn saved_dialogs_page_ends_on_short_page() {
        let dialog = |user_id, pinned| {
            base::SavedDialog::SavedDialog(Box::new(types::SavedDialog {
                pinned,
                peer: base::Peer::PeerUser(types::PeerUser { user_id }),
                top_message: 1,
            }))
        };
        let page = types::messages::SavedDialogs {
            dialogs: vec![dialog(USER, true), dialog(8, false)],
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let page = base::messages::SavedDialogs::SavedDialogs(Box::new(page)).to_bytes();
        let (client, mut rx) = setup(vec![page]);

        let mut dialogs = client.get_saved_dialogs();
        let first = dialogs.next().await.unwrap().unwrap();
        assert_eq!((first.chat.id, first.is_pinned, first.top_message.is_none()), (USER, true, true));
        assert_eq!(dialogs.next().await.unwrap().unwrap().chat.id, 8);
        assert!(dialogs.next().await.unwrap().is_none());
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::GetSavedDialogs::CONSTRUCTOR_ID);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn saved_dialogs_page_by_raw_entries() {
        let peer = |user_id| base::Peer::PeerUser(types::PeerUser { user_id });
        let channel_dm = types::MonoForumDialog {
            unread_mark: false,
            nopaid_messages_exception: false,
            peer: peer(8),
            top_message: 3,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 0,
            unread_reactions_count: 0,
            draft: None,
        };
        let pin = types::MessageService {
            out: true,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 5,
            from_id: None,
            peer_id: peer(USER),
            saved_peer_id: None,
            reply_to: Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(types::MessageReplyHeader {
                reply_to_msg_id: Some(4),
                ..header()
            }))),
            date: 50,
            action: base::MessageAction::MessageActionPinMessage,
            reactions: None,
            ttl_period: None,
        };
        let page = types::messages::SavedDialogs {
            dialogs: vec![
                base::SavedDialog::MonoForumDialog(Box::new(channel_dm)),
                base::SavedDialog::SavedDialog(Box::new(types::SavedDialog {
                    pinned: false,
                    peer: peer(USER),
                    top_message: 5,
                })),
            ],
            messages: vec![base::Message::MessageService(Box::new(pin))],
            chats: Vec::new(),
            users: Vec::new(),
        };
        let page = base::messages::SavedDialogs::SavedDialogs(Box::new(page)).to_bytes();
        let empty = types::messages::SavedDialogs {
            dialogs: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let empty = base::messages::SavedDialogs::SavedDialogs(Box::new(empty)).to_bytes();
        let (client, mut rx) = setup(vec![page, empty]);

        // A full raw page whose last top message is a service message still moves the cursor on.
        let mut dialogs = client.get_saved_dialogs().limit(2);
        let only = dialogs.next().await.unwrap().unwrap();
        assert_eq!((only.chat.id, only.top_message.map(|m| m.id)), (USER, Some(5)));
        assert!(dialogs.next().await.unwrap().is_none());
        assert_eq!(constructor(&rx.try_recv().unwrap()), messages::GetSavedDialogs::CONSTRUCTOR_ID);
        let second = messages::GetSavedDialogs {
            exclude_pinned: false,
            parent_peer: None,
            offset_date: 50,
            offset_id: 5,
            offset_peer: user(),
            limit: 1,
            hash: 0,
        };
        assert_eq!(rx.try_recv().unwrap(), second.to_bytes());
    }
}
