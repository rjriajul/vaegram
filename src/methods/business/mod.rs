mod check_quick_reply_shortcut;
mod create_business_chat_link;
mod delete_business_away_message;
mod delete_business_chat_link;
mod delete_business_greeting_message;
mod delete_business_intro;
mod delete_business_location;
mod delete_business_messages;
mod delete_business_opening_hours;
mod delete_business_story;
mod delete_quick_reply_messages;
mod delete_quick_reply_shortcut;
mod disable_peer_connected_bot;
mod edit_business_chat_link;
mod edit_business_story;
mod edit_quick_reply_shortcut;
mod get_business_account_star_balance;
mod get_business_chat_links;
mod get_business_connection;
mod get_connected_bots;
mod get_quick_replies;
mod post_business_story;
mod read_business_message;
mod remove_business_account_profile_photo;
mod remove_connected_bot;
mod reorder_quick_replies;
mod repost_business_story;
mod resolve_business_chat_link;
mod send_quick_reply_messages;
mod set_business_account_bio;
mod set_business_account_gift_settings;
mod set_business_account_name;
mod set_business_account_profile_photo;
mod set_business_account_username;
mod set_business_away_message;
mod set_business_greeting_message;
mod set_business_intro;
mod set_business_location;
mod set_business_opening_hours;
mod toggle_connected_bot_paused;
mod transfer_business_account_stars;
mod update_connected_bot;

pub use create_business_chat_link::CreateBusinessChatLink;
pub use edit_business_chat_link::EditBusinessChatLink;
pub use edit_business_story::EditBusinessStory;
pub use post_business_story::PostBusinessStory;
pub use repost_business_story::RepostBusinessStory;
pub use set_business_account_gift_settings::SetBusinessAccountGiftSettings;
pub use set_business_away_message::SetBusinessAwayMessage;
pub use set_business_location::SetBusinessLocation;

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::{account, messages};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{
        AcceptedGiftTypes, BusinessBotRights, BusinessOpeningHours, BusinessRecipients, InputStoryContent, QuickReply,
        Text, Upload,
    };

    async fn sent<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    #[tokio::test]
    async fn settings_requests() {
        let (client, mut rx) = Client::recording();
        let rx = &mut rx;

        let hours = BusinessOpeningHours::new("Europe/Berlin", [(540, 1020)]);
        let open = types::BusinessWeeklyOpen { start_minute: 540, end_minute: 1020 };
        let open = base::BusinessWeeklyOpen::BusinessWeeklyOpen(open);
        let work_hours =
            types::BusinessWorkHours { open_now: false, timezone_id: "Europe/Berlin".into(), weekly_open: vec![open] };
        let business_work_hours = Some(base::BusinessWorkHours::BusinessWorkHours(Box::new(work_hours)));
        let body = sent(rx, client.set_business_opening_hours(&hours)).await;
        assert_eq!(body, account::UpdateBusinessWorkHours { business_work_hours }.to_bytes());
        let body = sent(rx, client.delete_business_opening_hours()).await;
        assert_eq!(body, account::UpdateBusinessWorkHours { business_work_hours: None }.to_bytes());

        let body = sent(rx, client.set_business_location("Main St 1")).await;
        let address = Some("Main St 1".into());
        assert_eq!(body, account::UpdateBusinessLocation { geo_point: None, address }.to_bytes());
        let body = sent(rx, client.set_business_location("Main St 1").location(52.5, 13.4)).await;
        let point = types::InputGeoPoint { lat: 52.5, long: 13.4, accuracy_radius: None };
        let geo_point = Some(base::InputGeoPoint::InputGeoPoint(Box::new(point)));
        let address = Some("Main St 1".into());
        assert_eq!(body, account::UpdateBusinessLocation { geo_point, address }.to_bytes());
        let body = sent(rx, client.delete_business_intro()).await;
        assert_eq!(body, account::UpdateBusinessIntro { intro: None }.to_bytes());
        let body = sent(rx, client.reorder_quick_replies([2, 1])).await;
        assert_eq!(body, messages::ReorderQuickReplies { order: vec![2, 1] }.to_bytes());
    }

    #[tokio::test]
    async fn quick_replies_are_parsed() {
        let reply = types::QuickReply { shortcut_id: 4, shortcut: "hello".into(), top_message: 9, count: 2 };
        let replies = types::messages::QuickReplies {
            quick_replies: vec![base::QuickReply::QuickReply(Box::new(reply))],
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let answer = base::messages::QuickReplies::QuickReplies(Box::new(replies)).to_bytes();
        let (client, _rx) = Client::scripted(vec![Ok(answer)]);
        let expected = QuickReply { shortcut_id: 4, shortcut: "hello".into(), top_message: 9, count: 2 };
        assert_eq!(client.get_quick_replies().await.unwrap(), [expected]);
    }

    fn refused<T>(result: Result<T, Error>) -> bool {
        matches!(result, Err(Error::InvalidArgument(_)))
    }

    fn recording_client() -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    fn everyone() -> BusinessRecipients {
        BusinessRecipients::new().existing_chats(true)
    }

    #[tokio::test]
    async fn a_location_needs_a_short_address_and_real_coordinates() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.set_business_location("").await));
        assert!(refused(client.set_business_location("x".repeat(97)).await));
        assert!(refused(client.set_business_location("Main St 1").location(91.0, 0.0).await));
        assert!(refused(client.set_business_location("Main St 1").location(0.0, f64::NAN).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_greeting_waits_a_whole_number_of_weeks() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.set_business_greeting_message(1, &everyone(), 10).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn greetings_and_away_messages_take_no_excluded_users() {
        let (client, mut rx) = recording_client();
        let excluding = everyone().exclude_users([42]);
        assert!(refused(client.set_business_greeting_message(1, &excluding, 7).await));
        assert!(refused(client.set_business_away_message(1, &excluding).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_connected_bot_needs_a_recipient() {
        let (client, mut rx) = recording_client();
        let nobody = BusinessRecipients::default();
        assert!(refused(client.update_connected_bot(42, BusinessBotRights::default(), &nobody).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn deleting_no_quick_reply_messages_is_refused() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.delete_quick_reply_messages(1, &[]).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn reordering_no_quick_replies_is_refused() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.reorder_quick_replies([]).await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_quick_reply_shortcut_needs_a_name() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.edit_quick_reply_shortcut(1, "").await));
        assert!(refused(client.check_quick_reply_shortcut("").await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_chat_link_call_needs_its_slug() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.delete_business_chat_link("").await));
        assert!(refused(client.resolve_business_chat_link("").await));
        assert!(refused(client.edit_business_chat_link("", "hi").await));
        assert!(rx.try_recv().is_err());
    }

    fn privacy(display_gifts_button: bool, accepted: Option<AcceptedGiftTypes>) -> base::GlobalPrivacySettings {
        base::GlobalPrivacySettings::GlobalPrivacySettings(Box::new(types::GlobalPrivacySettings {
            archive_and_mute_new_noncontact_peers: false,
            keep_archived_unmuted: false,
            keep_archived_folders: false,
            hide_read_marks: true,
            new_noncontact_peers_require_premium: false,
            display_gifts_button,
            noncontact_peers_paid_stars: Some(25),
            disallowed_gifts: accepted.map(AcceptedGiftTypes::write),
        }))
    }

    #[tokio::test]
    async fn gift_settings_change_only_what_is_set() {
        let current = privacy(true, None);
        let no_unique = AcceptedGiftTypes::new().unique_gifts(false);
        let cases = [
            (Some(false), None, privacy(false, None)),
            (None, Some(no_unique), privacy(true, Some(no_unique))),
            (Some(false), Some(no_unique), privacy(false, Some(no_unique))),
        ];
        for (show_gift_button, accepted, settings) in cases {
            let (client, mut rx) = Client::scripted(vec![Ok(current.to_bytes())]);
            let mut request = client.set_business_account_gift_settings("c");
            if let Some(show) = show_gift_button {
                request = request.show_gift_button(show);
            }
            if let Some(accepted) = accepted {
                request = request.accepted_gift_types(accepted);
            }
            let _ = request.await;
            let get = account::GetGlobalPrivacySettings {};
            let get = functions::InvokeWithBusinessConnection { connection_id: "c".into(), query: get };
            assert_eq!(rx.try_recv().unwrap(), get.to_bytes());
            let set = account::SetGlobalPrivacySettings { settings };
            let set = functions::InvokeWithBusinessConnection { connection_id: "c".into(), query: set };
            assert_eq!(rx.try_recv().unwrap(), set.to_bytes());
        }
    }

    #[tokio::test]
    async fn gift_settings_with_nothing_to_change_are_refused() {
        let (client, mut rx) = recording_client();
        assert!(refused(client.set_business_account_gift_settings("c").await));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_built_story_caption_keeps_its_entities() {
        let connection = types::BotBusinessConnection {
            disabled: false,
            connection_id: "c".into(),
            user_id: 42,
            dc_id: 2,
            date: 1,
            rights: None,
        };
        let connection = base::BotBusinessConnection::BotBusinessConnection(Box::new(connection));
        let update = types::UpdateBotBusinessConnect { connection, qts: 0 };
        let update = base::Update::UpdateBotBusinessConnect(Box::new(update));
        let connected = base::Updates::UpdateShort(Box::new(types::UpdateShort { update, date: 0 })).to_bytes();
        let bold = vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 })];
        let mut caption = String::from("hi").to_bytes();
        caption.extend(bold.to_bytes());
        for edit in [true, false] {
            let (client, mut rx) = Client::scripted(vec![Ok(connected.clone()), Ok(true.to_bytes())]);
            let user = types::InputPeerUser { user_id: 42, access_hash: 7 };
            client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(user)));
            let photo = InputStoryContent::photo(Upload::bytes("a.jpg", b"x"));
            let text = Text::new().bold("hi");
            let _ = if edit {
                client.edit_business_story("c", 1, photo).caption(text).await
            } else {
                client.post_business_story("c", photo, 86400).caption(text).await
            };
            let bodies: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
            assert!(bodies.iter().any(|body| body.windows(caption.len()).any(|w| w == caption)), "edit: {edit}");
        }
    }
}
