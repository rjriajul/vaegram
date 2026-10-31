mod add_checklist_tasks;
mod add_poll_option;
mod add_to_gifs;
mod approve_suggested_post;
mod clear_all_drafts;
mod compose_text_with_ai;
mod copy_media_group;
mod copy_message;
mod copy_messages;
mod decline_suggested_post;
mod delete_all_message_reactions;
mod delete_ephemeral_message;
mod delete_message_reaction;
mod delete_messages;
mod delete_poll_option;
mod delete_saved_history;
mod delete_scheduled_messages;
mod download_media;
mod edit_ephemeral_message_caption;
mod edit_ephemeral_message_media;
mod edit_ephemeral_message_reply_markup;
mod edit_ephemeral_message_text;
mod edit_fact_check;
mod edit_inline_media;
mod edit_inline_reply_markup;
mod edit_inline_text;
mod edit_message_caption;
mod edit_message_checklist;
mod edit_message_live_location;
mod edit_message_media;
mod edit_message_reply_markup;
mod edit_message_text;
mod export_message_link;
mod fix_text_with_ai;
mod forward_media_group;
mod forward_messages;
mod get_all_drafts;
mod get_available_effects;
mod get_available_reactions;
mod get_chat_history;
mod get_chat_history_count;
mod get_custom_emoji_stickers;
mod get_discussion_message;
mod get_discussion_replies;
mod get_discussion_replies_count;
mod get_media_group;
mod get_message_by_link;
mod get_message_reactions_list;
mod get_message_read_date;
mod get_message_read_participants;
mod get_messages;
mod get_messages_views;
mod get_poll_results;
mod get_poll_votes;
mod get_recent_reactions;
mod get_rich_message;
mod get_saved_dialogs;
mod get_saved_gifs;
mod get_saved_history;
mod get_saved_reaction_tags;
mod get_scheduled_messages;
mod get_top_reactions;
mod get_user_personal_chat_messages;
mod mark_checklist_tasks_as_done;
mod pages;
mod pin_chat_message;
mod pin_saved_dialog;
mod read_chat_history;
mod read_mentions;
mod read_message_contents;
mod read_reactions;
mod remove_from_gifs;
mod request_app_web_view;
mod request_main_web_view;
mod request_simple_web_view;
mod request_web_view;
mod retract_vote;
mod save_draft;
mod search_global;
mod search_global_count;
mod search_messages;
mod search_messages_count;
mod search_posts;
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
mod send_paid_reaction;
mod send_photo;
mod send_poll;
mod send_reaction;
mod send_rich_message;
mod send_scheduled_now;
mod send_screenshot_notification;
mod send_sticker;
mod send_venue;
mod send_video;
mod send_video_note;
mod send_voice;
mod send_web_view_data;
mod set_default_reaction;
mod stop_message_live_location;
mod stop_poll;
mod stream_message;
mod summarize_message;
mod transcribe_audio;
mod translate_message_text;
mod translate_text;
mod unpin_all_chat_messages;
mod unpin_chat_message;
mod unpin_saved_dialog;
mod update_saved_reaction_tag;
mod upload_media;
mod vote_poll;
mod web_view_options;

pub use add_checklist_tasks::AddChecklistTasks;
pub use approve_suggested_post::ApproveSuggestedPost;
pub use compose_text_with_ai::ComposeTextWithAi;
pub use copy_media_group::CopyMediaGroup;
pub use copy_message::CopyMessage;
pub use copy_messages::CopyMessages;
pub use decline_suggested_post::DeclineSuggestedPost;
pub(crate) use download_media::DownloadSource;
pub use download_media::{DownloadMedia, MediaStream};
pub use edit_ephemeral_message_text::EditEphemeralMessage;
pub use edit_fact_check::EditFactCheck;
pub use edit_inline_media::EditInlineMedia;
pub use edit_inline_text::EditInlineText;
pub use edit_message_checklist::EditMessageChecklist;
pub use edit_message_live_location::EditMessageLiveLocation;
pub use edit_message_media::EditMessageMedia;
pub use edit_message_reply_markup::EditMessageReplyMarkup;
pub use edit_message_text::EditMessageText;
pub use export_message_link::ExportMessageLink;
pub use forward_media_group::ForwardMediaGroup;
pub use forward_messages::ForwardMessages;
pub use get_chat_history::GetChatHistory;
pub use get_discussion_replies::GetDiscussionReplies;
pub use get_message_reactions_list::GetMessageReactionsList;
pub use get_poll_votes::GetPollVotes;
pub use get_saved_dialogs::GetSavedDialogs;
pub use get_saved_history::GetSavedHistory;
pub use pin_chat_message::PinChatMessage;
pub use request_app_web_view::RequestAppWebView;
pub use request_main_web_view::RequestMainWebView;
pub use request_simple_web_view::RequestSimpleWebView;
pub use request_web_view::RequestWebView;
pub use save_draft::SaveDraft;
pub use search_global::SearchGlobal;
pub use search_messages::SearchMessages;
pub use search_messages_count::SearchMessagesCount;
pub use search_posts::SearchPosts;
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
pub use send_paid_reaction::SendPaidReaction;
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
pub use summarize_message::SummarizeMessage;
pub use translate_text::TranslateText;
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

#[cfg(test)]
mod search_tests {
    use crate::Client;
    use crate::client::sent;
    use crate::enums::MessagesFilter;
    use crate::errors::Error;
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;

    const GROUP: i64 = -5;

    #[tokio::test]
    async fn each_call_sends_its_request() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        let rx = &mut rx;

        assert_eq!(sent(rx, client.search_global("x").next()).await, messages::SearchGlobal::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.search_posts().hashtag("x").next()).await, channels::SearchPosts::CONSTRUCTOR_ID);
        let count = client.search_global_count("x", MessagesFilter::Url);
        assert_eq!(sent(rx, count).await, messages::SearchGlobal::CONSTRUCTOR_ID);
        let photos = client.search_messages_count(GROUP).filter(MessagesFilter::Photo);
        assert_eq!(sent(rx, photos).await, messages::GetSearchCounters::CONSTRUCTOR_ID);
        let matches = client.search_messages_count(GROUP).query("x").filter(MessagesFilter::Photo);
        assert_eq!(sent(rx, matches).await, messages::Search::CONSTRUCTOR_ID);
        let all = client.search_messages_count(GROUP);
        assert_eq!(sent(rx, all).await, messages::Search::CONSTRUCTOR_ID);

        let post = client.get_discussion_message(channel, 1);
        assert_eq!(sent(rx, post).await, messages::GetDiscussionMessage::CONSTRUCTOR_ID);
        let mut comments = client.get_discussion_replies(channel, 1);
        assert_eq!(sent(rx, comments.next()).await, messages::GetReplies::CONSTRUCTOR_ID);
        let comments = client.get_discussion_replies_count(channel, 1);
        assert_eq!(sent(rx, comments).await, messages::GetReplies::CONSTRUCTOR_ID);
        let read = client.get_message_read_participants(GROUP, 1);
        assert_eq!(sent(rx, read).await, messages::GetMessageReadParticipants::CONSTRUCTOR_ID);

        let text = client.translate_text("es", "hi");
        assert_eq!(sent(rx, text).await, messages::TranslateText::CONSTRUCTOR_ID);
        let posted = client.translate_message_text("es", GROUP, &[1, 2]);
        assert_eq!(sent(rx, posted).await, messages::TranslateText::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.transcribe_audio(GROUP, 1)).await, messages::TranscribeAudio::CONSTRUCTOR_ID);

        assert_eq!(sent(rx, client.get_chat_online_count(GROUP)).await, messages::GetOnlines::CONSTRUCTOR_ID);
        let similar = client.get_similar_channels(channel);
        assert_eq!(sent(rx, similar).await, channels::GetChannelRecommendations::CONSTRUCTOR_ID);
        let recommended = client.get_recommended_channels();
        assert_eq!(sent(rx, recommended).await, channels::GetChannelRecommendations::CONSTRUCTOR_ID);

        let refused = client.get_similar_channels(GROUP).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}

#[cfg(test)]
mod reaction_tests {
    use crate::Client;
    use crate::client::sent;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};

    const USER: i64 = 7;

    fn setup(answers: Vec<Vec<u8>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(answers.into_iter().map(Ok).collect());
        let peer = types::InputPeerUser { user_id: USER, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(peer)));
        (client, rx)
    }

    #[tokio::test]
    async fn reaction_calls_hit_their_methods() {
        let (client, mut rx) = setup(Vec::new());
        let rx = &mut rx;
        let mut reactors = client.get_message_reactions_list(USER, 1).reaction("👍");

        assert_eq!(sent(rx, reactors.next()).await, messages::GetMessageReactionsList::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.send_paid_reaction(USER, 1, 5)).await, messages::SendPaidReaction::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_default_reaction("👍")).await, messages::SetDefaultReaction::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_available_reactions()).await, messages::GetAvailableReactions::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_top_reactions(10)).await, messages::GetTopReactions::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_recent_reactions(10)).await, messages::GetRecentReactions::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_saved_reaction_tags()).await, messages::GetSavedReactionTags::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.update_saved_reaction_tag("👍", "")).await,
            messages::UpdateSavedReactionTag::CONSTRUCTOR_ID
        );
        assert_eq!(sent(rx, client.get_available_effects()).await, messages::GetAvailableEffects::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn poll_votes_follow_next_offset() {
        let page = |options: &[u8], next_offset: Option<&str>| {
            let peer = base::Peer::PeerUser(types::PeerUser { user_id: USER });
            let votes = options.iter().map(|&o| {
                let vote = types::MessagePeerVote { peer: peer.clone(), option: vec![o], date: 1 };
                base::MessagePeerVote::MessagePeerVote(Box::new(vote))
            });
            let list = types::messages::VotesList {
                count: 3,
                votes: votes.collect(),
                chats: Vec::new(),
                users: Vec::new(),
                next_offset: next_offset.map(Into::into),
            };
            base::messages::VotesList::VotesList(Box::new(list)).to_bytes()
        };
        let (client, mut rx) = setup(vec![page(&[0, 1], Some("next")), page(&[2], None)]);
        let peer = || base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }));

        let mut votes = client.get_poll_votes(USER, 9);
        let mut options = Vec::new();
        while let Some(vote) = votes.next().await.unwrap() {
            options.extend(vote.options);
        }
        assert_eq!(options, [vec![0], vec![1], vec![2]]);
        let first = messages::GetPollVotes { peer: peer(), id: 9, option: None, offset: None, limit: 50 };
        let second =
            messages::GetPollVotes { peer: peer(), id: 9, option: None, offset: Some("next".into()), limit: 50 };
        assert_eq!((rx.try_recv().unwrap(), rx.try_recv().unwrap()), (first.to_bytes(), second.to_bytes()));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn filtered_votes_carry_the_queried_option() {
        let peer = base::Peer::PeerUser(types::PeerUser { user_id: 8 });
        let vote = types::MessagePeerVoteInputOption { peer, date: 1 };
        let list = types::messages::VotesList {
            count: 1,
            votes: vec![base::MessagePeerVote::MessagePeerVoteInputOption(Box::new(vote))],
            chats: Vec::new(),
            users: Vec::new(),
            next_offset: None,
        };
        let (client, _rx) = setup(vec![base::messages::VotesList::VotesList(Box::new(list)).to_bytes()]);

        let vote = client.get_poll_votes(USER, 9).option(vec![1]).next().await.unwrap().unwrap();
        assert_eq!((vote.voter_id, vote.options), (8, vec![vec![1]]));
    }
}

#[cfg(test)]
mod media_group_tests {
    use crate::Client;
    use crate::client::sent;
    use crate::errors::Error;
    use crate::raw::functions::{channels, contacts, messages};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{Message, SuggestedPostParameters};
    use crate::utils::{get_channel_id, get_peer};

    const GROUP: i64 = -5;

    fn item(id: i32, chat_id: i64, grouped_id: Option<i64>, caption: &str) -> types::Message {
        types::Message {
            grouped_id,
            message: caption.into(),
            media: Some(document_media("application/pdf", Vec::new())),
            ..Message::bare(id, get_peer(chat_id), 1)
        }
    }

    fn found(messages: Vec<types::Message>) -> Vec<u8> {
        let messages = messages.into_iter().map(|m| base::Message::Message(Box::new(m))).collect();
        let found = types::messages::Messages { messages, topics: Vec::new(), chats: Vec::new(), users: Vec::new() };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    // Messages 10 and 11 are album 1; 12 is another album, and 9 is in album 1 of another chat.
    fn album() -> Vec<types::Message> {
        vec![
            item(9, 7, Some(1), "elsewhere"),
            item(10, GROUP, Some(1), "first"),
            item(11, GROUP, Some(1), "second"),
            item(12, GROUP, Some(2), "other"),
        ]
    }

    fn new_messages(ids: &[i32]) -> Vec<u8> {
        let updates = ids
            .iter()
            .map(|&id| {
                let message = base::Message::Message(Box::new(item(id, GROUP, Some(3), "")));
                base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 1, pts_count: 1 }))
            })
            .collect();
        let updates = types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    fn get_messages(ids: impl Iterator<Item = i32>) -> Vec<u8> {
        let id = ids.map(|id| base::InputMessage::InputMessageID(types::InputMessageID { id })).collect();
        messages::GetMessages { id }.to_bytes()
    }

    fn word(body: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(body[at..at + 4].try_into().unwrap())
    }

    fn ids(messages: &[Message]) -> Vec<i32> {
        messages.iter().map(|m| m.id).collect()
    }

    #[tokio::test]
    async fn non_positive_ids_are_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        for id in [0, -1] {
            let result = client.get_media_group(GROUP, id).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_window_covers_nine_ids_either_side() {
        let (client, mut rx) = Client::recording();
        let _ = client.get_media_group(GROUP, 3).await;
        assert_eq!(rx.try_recv().unwrap(), get_messages(1..=12));
        let _ = client.get_media_group(GROUP, 20).await;
        assert_eq!(rx.try_recv().unwrap(), get_messages(11..=29));
        let _ = client.get_media_group(GROUP, i32::MAX).await;
        assert_eq!(rx.try_recv().unwrap(), get_messages(i32::MAX - 9..=i32::MAX));

        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        assert_eq!(sent(&mut rx, client.get_media_group(channel, 3)).await, channels::GetMessages::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn returns_only_the_anchors_album_in_order() {
        let (client, _rx) = Client::scripted(vec![Ok(found(album()))]);
        assert_eq!(ids(&client.get_media_group(GROUP, 11).await.unwrap()), [10, 11]);
    }

    #[tokio::test]
    async fn a_missing_or_ungrouped_anchor_is_refused() {
        let ungrouped = found(vec![item(13, GROUP, None, "")]);
        let (client, _rx) = Client::scripted(vec![Ok(found(album())), Ok(ungrouped)]);
        let missing = client.get_media_group(GROUP, 13).await;
        assert!(matches!(missing, Err(Error::InvalidArgument("no message with this id"))), "{missing:?}");
        let ungrouped = client.get_media_group(GROUP, 13).await;
        let refused = matches!(ungrouped, Err(Error::InvalidArgument("this message isn't part of a media group")));
        assert!(refused, "{ungrouped:?}");
    }

    #[tokio::test]
    async fn forwards_the_whole_album() {
        let replies = vec![Ok(found(album())), Ok(new_messages(&[21, 20])), Ok(found(album()))];
        let (client, mut rx) = Client::scripted(replies);
        assert_eq!(ids(&client.forward_media_group(GROUP, GROUP, 11).await.unwrap()), [20, 21]);
        rx.try_recv().unwrap();
        let body = rx.try_recv().unwrap();
        assert_eq!((word(&body, 0), word(&body, 4)), (messages::ForwardMessages::CONSTRUCTOR_ID, 0));
        // from_peer is a 12-byte inputPeerChat; the id vector follows it.
        assert_eq!((word(&body, 24), word(&body, 28), word(&body, 32)), (2, 10, 11));

        let _ = client.forward_media_group(GROUP, GROUP, 11).hide_sender_name().hide_captions().await;
        rx.try_recv().unwrap();
        let body = rx.try_recv().unwrap();
        assert_eq!(
            (word(&body, 0), word(&body, 4)),
            (messages::ForwardMessages::CONSTRUCTOR_ID, (1 << 11) | (1 << 12))
        );
    }

    #[tokio::test]
    async fn a_named_source_chat_is_resolved_once() {
        let peer = base::Peer::PeerChat(types::PeerChat { chat_id: 5 });
        let resolved = types::contacts::ResolvedPeer { peer, chats: Vec::new(), users: Vec::new() };
        let resolved = base::contacts::ResolvedPeer::ResolvedPeer(Box::new(resolved)).to_bytes();
        let replies = vec![Ok(resolved), Ok(found(album())), Ok(new_messages(&[20, 21]))];
        let (client, mut rx) = Client::scripted(replies);
        assert_eq!(ids(&client.forward_media_group(GROUP, "@group", 11).await.unwrap()), [20, 21]);
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).map(|body| word(&body, 0)).collect();
        let expected = [
            contacts::ResolveUsername::CONSTRUCTOR_ID,
            messages::GetMessages::CONSTRUCTOR_ID,
            messages::ForwardMessages::CONSTRUCTOR_ID,
        ];
        assert_eq!(sent, expected);
    }

    #[tokio::test]
    async fn copies_the_album_with_a_new_caption() {
        let mut source = album();
        source[2].invert_media = true;
        let (client, mut rx) = Client::scripted(vec![Ok(found(source)), Ok(new_messages(&[21, 20]))]);
        let copied = client.copy_media_group(GROUP, GROUP, 10).caption(1, "new").await.unwrap();
        assert_eq!(ids(&copied), [20, 21]);
        rx.try_recv().unwrap();
        let body = rx.try_recv().unwrap();
        assert_eq!((word(&body, 0), word(&body, 4)), (messages::SendMultiMedia::CONSTRUCTOR_ID, 1 << 16));
        let has = |text: &str| body.windows(text.len()).any(|w| w == text.as_bytes());
        assert_eq!((has("first"), has("new"), has("second")), (true, true, false));
    }

    #[tokio::test]
    async fn bad_copies_are_refused_before_sending() {
        let ungrouped = found(vec![item(13, GROUP, None, "")]);
        let (client, mut rx) = Client::scripted(vec![Ok(found(album())), Ok(ungrouped)]);
        let past_the_end = client.copy_media_group(GROUP, GROUP, 10).caption(2, "x").await;
        assert!(matches!(past_the_end, Err(Error::InvalidArgument(_))), "{past_the_end:?}");
        let ungrouped = client.copy_media_group(GROUP, GROUP, 13).await;
        assert!(matches!(ungrouped, Err(Error::InvalidArgument(_))), "{ungrouped:?}");
        let fetches = (rx.try_recv().map(|b| word(&b, 0)), rx.try_recv().map(|b| word(&b, 0)));
        assert_eq!(fetches, (Ok(messages::GetMessages::CONSTRUCTOR_ID), Ok(messages::GetMessages::CONSTRUCTOR_ID)));
        assert!(rx.try_recv().is_err());

        let suggested = SuggestedPostParameters::default();
        let copy = client.copy_media_group(GROUP, GROUP, 10).suggested_post_parameters(suggested).await;
        let forward = client.forward_media_group(GROUP, GROUP, 10).suggested_post_parameters(suggested).await;
        for result in [copy, forward] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(rx.try_recv().is_err());
    }
}
