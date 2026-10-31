mod add_bot_preview_media;
mod allow_bot_send_message;
mod answer_callback_query;
mod answer_chat_join_request_query;
mod answer_guest_query;
mod answer_inline_query;
mod answer_pre_checkout_query;
mod answer_shipping_query;
mod answer_web_app_query;
mod can_bot_send_message;
mod check_bot_username;
mod create_bot;
mod create_invoice_link;
mod delete_bot_commands;
mod delete_bot_preview_media;
mod edit_bot_preview_media;
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
mod get_bot_preview_medias;
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
mod reorder_bot_preview_medias;
mod replace_managed_bot_token;
mod request_callback_answer;
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

pub use answer_callback_query::AnswerCallbackQuery;
pub use answer_inline_query::AnswerInlineQuery;
pub use answer_pre_checkout_query::AnswerPreCheckoutQuery;
pub use answer_shipping_query::AnswerShippingQuery;
pub use create_bot::CreateBot;
pub use create_invoice_link::CreateInvoiceLink;
pub use get_inline_bot_results::GetInlineBotResults;
pub use get_popular_app_bots::GetPopularAppBots;
pub use request_callback_answer::RequestCallbackAnswer;
pub use save_prepared_inline_message::SavePreparedInlineMessage;
pub use send_game::SendGame;
pub use send_inline_bot_result::SendInlineBotResult;
pub use send_invoice::SendInvoice;
pub use set_game_score::SetGameScore;
pub use set_inline_game_score::SetInlineGameScore;
pub use set_user_emoji_status::SetUserEmojiStatus;
pub use toggle_bot_in_attach_menu::ToggleBotInAttachMenu;
pub use verify_chat::VerifyChat;

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    const BOT: i64 = 42;

    fn bot_peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: BOT, access_hash: 7 }))
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
