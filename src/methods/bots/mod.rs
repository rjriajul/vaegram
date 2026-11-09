mod add_bot_preview_media;
mod allow_bot_send_message;
mod answer_callback_query;
mod answer_chat_join_request_query;
mod answer_guest_query;
mod answer_inline_query;
mod answer_pre_checkout_query;
mod answer_shipping_query;
mod answer_web_app_query;
mod answer_widget_query;
mod can_bot_send_message;
mod check_bot_username;
mod create_bot;
mod create_invoice_link;
mod delete_bot_commands;
mod delete_bot_preview_media;
mod edit_bot_preview_media;
mod edit_callback_message;
mod edit_user_star_subscription;
mod get_attach_menu_bot;
mod get_attach_menu_bots;
mod get_bot_app;
mod get_bot_commands;
mod get_bot_default_privileges;
mod get_bot_description;
mod get_bot_info;
mod get_bot_name;
mod get_bot_preview_info;
mod get_bot_preview_media;
mod get_bot_short_description;
mod get_chat_menu_button;
mod get_game_high_scores;
mod get_inline_bot_results;
mod get_inline_game_high_scores;
mod get_managed_bot_access_settings;
mod get_managed_bot_token;
mod get_owned_bots;
mod get_popular_app_bots;
mod get_similar_bots;
mod get_user_chat_boosts;
mod invoke_web_view_custom_method;
mod refund_star_payment;
mod remove_chat_verification;
mod remove_user_verification;
mod reorder_bot_preview_media;
mod replace_managed_bot_token;
mod request_callback_answer;
mod request_widget_query_answer;
mod save_prepared_inline_message;
mod save_prepared_keyboard_button;
mod send_chat_join_request_web_app;
mod send_game;
mod send_inline_bot_result;
mod send_invoice;
mod set_bot_commands;
mod set_bot_default_privileges;
mod set_bot_description;
mod set_bot_name;
mod set_bot_short_description;
mod set_chat_menu_button;
mod set_game_score;
mod set_inline_game_score;
mod set_managed_bot_access_settings;
mod set_user_emoji_status;
mod start_bot;
mod toggle_bot_in_attach_menu;
mod verify_chat;
mod verify_user;

pub use add_bot_preview_media::AddBotPreviewMedia;
pub use answer_callback_query::AnswerCallbackQuery;
pub use answer_guest_query::AnswerGuestQuery;
pub use answer_inline_query::AnswerInlineQuery;
pub use answer_pre_checkout_query::AnswerPreCheckoutQuery;
pub use answer_shipping_query::AnswerShippingQuery;
pub use answer_widget_query::AnswerWidgetQuery;
pub use create_bot::CreateBot;
pub use create_invoice_link::CreateInvoiceLink;
pub use delete_bot_commands::DeleteBotCommands;
pub use delete_bot_preview_media::DeleteBotPreviewMedia;
pub use edit_bot_preview_media::EditBotPreviewMedia;
pub use edit_callback_message::{EditCallbackMessageMedia, EditCallbackMessageText};
pub use get_bot_commands::GetBotCommands;
pub use get_bot_description::GetBotDescription;
pub use get_bot_info::GetBotInfo;
pub use get_bot_name::GetBotName;
pub use get_bot_preview_info::GetBotPreviewInfo;
pub use get_bot_short_description::GetBotShortDescription;
pub use get_inline_bot_results::GetInlineBotResults;
pub use get_popular_app_bots::GetPopularAppBots;
pub use reorder_bot_preview_media::ReorderBotPreviewMedia;
pub use request_callback_answer::RequestCallbackAnswer;
pub use request_widget_query_answer::RequestWidgetQueryAnswer;
pub use save_prepared_inline_message::SavePreparedInlineMessage;
pub use send_game::SendGame;
pub use send_inline_bot_result::SendInlineBotResult;
pub use send_invoice::SendInvoice;
pub use set_bot_commands::SetBotCommands;
pub use set_bot_description::SetBotDescription;
pub use set_bot_name::SetBotName;
pub use set_bot_short_description::SetBotShortDescription;
pub use set_game_score::SetGameScore;
pub use set_inline_game_score::SetInlineGameScore;
pub use set_managed_bot_access_settings::SetManagedBotAccessSettings;
pub use set_user_emoji_status::SetUserEmojiStatus;
pub use toggle_bot_in_attach_menu::ToggleBotInAttachMenu;
pub use verify_chat::VerifyChat;

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{BotCommand, BotCommandScope, BotPreviewMedia, InputMedia, Media};

    const BOT: i64 = 42;
    const USER: i64 = 43;

    fn bot_peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: BOT, access_hash: 7 }))
    }

    fn input_user(user_id: i64) -> base::InputUser {
        base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: 7 }))
    }

    fn recording() -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        for user_id in [BOT, USER] {
            let peer = types::InputPeerUser { user_id, access_hash: 7 };
            client.0.peers.lock().unwrap().insert(user_id, base::InputPeer::InputPeerUser(Box::new(peer)));
        }
        (client, rx)
    }

    async fn sent<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    #[tokio::test]
    async fn commands_default_to_the_default_scope_and_language() {
        let (client, mut rx) = recording();
        let rx = &mut rx;
        let start = || BotCommand::new("start", "Start");
        let default = || base::BotCommandScope::BotCommandScopeDefault;

        let body = sent(rx, client.set_bot_commands([start()])).await;
        let expected = functions::bots::SetBotCommands {
            scope: default(),
            lang_code: String::new(),
            commands: vec![start().write()],
        };
        assert_eq!(body, expected.to_bytes());
        let body =
            sent(rx, client.set_bot_commands([start()]).scope(BotCommandScope::AllPrivateChats).language_code("en"))
                .await;
        let expected = functions::bots::SetBotCommands {
            scope: base::BotCommandScope::BotCommandScopeUsers,
            lang_code: "en".into(),
            commands: vec![start().write()],
        };
        assert_eq!(body, expected.to_bytes());

        let body = sent(rx, client.get_bot_commands()).await;
        assert_eq!(body, functions::bots::GetBotCommands { scope: default(), lang_code: String::new() }.to_bytes());
        let body =
            sent(rx, client.get_bot_commands().scope(BotCommandScope::Chat(BOT.into())).language_code("de")).await;
        let peer =
            base::BotCommandScope::BotCommandScopePeer(Box::new(types::BotCommandScopePeer { peer: bot_peer() }));
        assert_eq!(body, functions::bots::GetBotCommands { scope: peer, lang_code: "de".into() }.to_bytes());

        let body = sent(rx, client.delete_bot_commands()).await;
        assert_eq!(body, functions::bots::ResetBotCommands { scope: default(), lang_code: String::new() }.to_bytes());
        let body =
            sent(rx, client.delete_bot_commands().scope(BotCommandScope::AllGroupChats).language_code("en")).await;
        let chats = base::BotCommandScope::BotCommandScopeChats;
        assert_eq!(body, functions::bots::ResetBotCommands { scope: chats, lang_code: "en".into() }.to_bytes());
    }

    #[tokio::test]
    async fn bot_texts_default_to_every_language() {
        let (client, mut rx) = recording();
        let rx = &mut rx;
        let set = |lang_code: &str, name: Option<&str>, about: Option<&str>, description: Option<&str>| {
            functions::bots::SetBotInfo {
                bot: None,
                lang_code: lang_code.into(),
                name: name.map(Into::into),
                about: about.map(Into::into),
                description: description.map(Into::into),
            }
            .to_bytes()
        };
        let get = |bot, lang_code: &str| functions::bots::GetBotInfo { bot, lang_code: lang_code.into() }.to_bytes();

        assert_eq!(sent(rx, client.set_bot_name("Helper")).await, set("", Some("Helper"), None, None));
        assert_eq!(
            sent(rx, client.set_bot_name("Helfer").language_code("de")).await,
            set("de", Some("Helfer"), None, None)
        );
        assert_eq!(sent(rx, client.set_bot_description("Helps")).await, set("", None, None, Some("Helps")));
        let body = sent(rx, client.set_bot_description("Hilft").language_code("de")).await;
        assert_eq!(body, set("de", None, None, Some("Hilft")));
        assert_eq!(sent(rx, client.set_bot_short_description("Bot")).await, set("", None, Some("Bot"), None));
        let body = sent(rx, client.set_bot_short_description("Bot").language_code("de")).await;
        assert_eq!(body, set("de", None, Some("Bot"), None));

        assert_eq!(sent(rx, client.get_bot_name()).await, get(None, ""));
        assert_eq!(sent(rx, client.get_bot_name().language_code("de")).await, get(None, "de"));
        assert_eq!(sent(rx, client.get_bot_description()).await, get(None, ""));
        assert_eq!(sent(rx, client.get_bot_description().language_code("de")).await, get(None, "de"));
        assert_eq!(sent(rx, client.get_bot_short_description()).await, get(None, ""));
        assert_eq!(sent(rx, client.get_bot_short_description().language_code("de")).await, get(None, "de"));
        assert_eq!(sent(rx, client.get_bot_info(BOT)).await, get(Some(input_user(BOT)), ""));
        assert_eq!(sent(rx, client.get_bot_info(BOT).language_code("de")).await, get(Some(input_user(BOT)), "de"));
    }

    fn photo_preview(client: &Client) -> BotPreviewMedia {
        let size = types::PhotoSize { r#type: "x".into(), w: 10, h: 20, size: 30 };
        let photo = types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        };
        let media = types::MessageMediaPhoto {
            spoiler: false,
            live_photo: false,
            photo: Some(base::Photo::Photo(Box::new(photo))),
            ttl_seconds: None,
            video: None,
        };
        let media = base::MessageMedia::MessageMediaPhoto(Box::new(media));
        BotPreviewMedia::parse(
            client,
            base::BotPreviewMedia::BotPreviewMedia(Box::new(types::BotPreviewMedia { date: 5, media })),
        )
    }

    #[tokio::test]
    async fn preview_media_defaults_to_every_language() {
        let (client, mut rx) = recording();
        let rx = &mut rx;
        let preview = photo_preview(&client);
        let Some(Media::Photo(photo)) = &preview.media else { panic!("not a photo preview") };
        let stored = preview.input().unwrap();
        let previews = std::slice::from_ref(&preview);
        let bot = || input_user(BOT);
        // Each request once without `language_code`, which must send the default previews' empty code, then with it.
        macro_rules! in_language {
            ($request:expr, $language:expr) => {
                match $language {
                    Some(language) => $request.language_code(language),
                    None => $request,
                }
            };
        }

        for language in [None, Some("en")] {
            let lang_code = || language.unwrap_or_default().to_owned();
            let add = in_language!(client.add_bot_preview_media(BOT, InputMedia::photo(photo)), language);
            let expected =
                functions::bots::AddPreviewMedia { bot: bot(), lang_code: lang_code(), media: stored.clone() };
            assert_eq!(sent(rx, add).await, expected.to_bytes());

            let edit = in_language!(client.edit_bot_preview_media(BOT, &preview, InputMedia::photo(photo)), language);
            let expected = functions::bots::EditPreviewMedia {
                bot: bot(),
                lang_code: lang_code(),
                media: stored.clone(),
                new_media: stored.clone(),
            };
            assert_eq!(sent(rx, edit).await, expected.to_bytes());

            let delete = in_language!(client.delete_bot_preview_media(BOT, previews), language);
            let expected =
                functions::bots::DeletePreviewMedia { bot: bot(), lang_code: lang_code(), media: vec![stored.clone()] };
            assert_eq!(sent(rx, delete).await, expected.to_bytes());

            let reorder = in_language!(client.reorder_bot_preview_media(BOT, previews), language);
            let expected = functions::bots::ReorderPreviewMedias {
                bot: bot(),
                lang_code: lang_code(),
                order: vec![stored.clone()],
            };
            assert_eq!(sent(rx, reorder).await, expected.to_bytes());

            let info = in_language!(client.get_bot_preview_info(BOT), language);
            let expected = functions::bots::GetPreviewInfo { bot: bot(), lang_code: lang_code() };
            assert_eq!(sent(rx, info).await, expected.to_bytes());
        }
        let body = sent(rx, client.get_bot_preview_media(BOT)).await;
        assert_eq!(body, functions::bots::GetPreviewMedias { bot: bot() }.to_bytes());
    }

    #[tokio::test]
    async fn managed_bots_take_any_peer() {
        let (client, mut rx) = recording();
        let rx = &mut rx;
        let bot = || input_user(BOT);

        let body = sent(rx, client.get_managed_bot_token(BOT)).await;
        assert_eq!(body, functions::bots::ExportBotToken { bot: bot(), revoke: false }.to_bytes());
        let body = sent(rx, client.replace_managed_bot_token(BOT)).await;
        assert_eq!(body, functions::bots::ExportBotToken { bot: bot(), revoke: true }.to_bytes());
        let body = sent(rx, client.get_managed_bot_access_settings(BOT)).await;
        assert_eq!(body, functions::bots::GetAccessSettings { bot: bot() }.to_bytes());

        let body = sent(rx, client.set_managed_bot_access_settings(BOT).allow_everyone()).await;
        let everyone = functions::bots::EditAccessSettings { restricted: false, bot: bot(), add_users: None };
        assert_eq!(body, everyone.to_bytes());
        let body = sent(rx, client.set_managed_bot_access_settings(BOT).restrict_to([USER])).await;
        let restricted = functions::bots::EditAccessSettings {
            restricted: true,
            bot: bot(),
            add_users: Some(vec![input_user(USER)]),
        };
        assert_eq!(body, restricted.to_bytes());
        let body = sent(rx, client.set_managed_bot_access_settings(BOT).restrict_to_owner()).await;
        let owner_only =
            functions::bots::EditAccessSettings { restricted: true, bot: bot(), add_users: Some(Vec::new()) };
        assert_eq!(body, owner_only.to_bytes());
    }

    #[tokio::test]
    async fn access_settings_with_no_mode_are_refused_without_a_request() {
        let (client, mut rx) = recording();
        let unset = client.set_managed_bot_access_settings(BOT).await;
        assert!(matches!(unset, Err(crate::errors::Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn inline_results_default_to_saved_messages() {
        let (client, mut rx) = Client::recording();
        client.0.peers.lock().unwrap().insert(BOT, bot_peer());
        let _ = client.get_inline_bot_results(BOT, "cats").offset("10").await;
        let expected = functions::messages::GetInlineBotResults {
            bot: base::InputUser::InputUser(Box::new(types::InputUser { user_id: BOT, access_hash: 7 })),
            peer: base::InputPeer::InputPeerSelf,
            geo_point: None,
            query: "cats".into(),
            offset: "10".into(),
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn game_callback_sends_no_data() {
        let (client, mut rx) = Client::recording();
        client.0.peers.lock().unwrap().insert(BOT, bot_peer());
        let _ = client.request_callback_answer(BOT, 3, b"x".as_slice()).game().await;
        let expected = functions::messages::GetBotCallbackAnswer {
            game: true,
            peer: bot_peer(),
            msg_id: 3,
            data: None,
            password: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
