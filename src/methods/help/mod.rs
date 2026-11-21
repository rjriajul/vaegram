mod get_app_config;
mod get_config;
mod get_countries_list;
mod get_nearest_dc;
mod get_promo_data;
mod get_support;
mod get_support_name;
mod get_timezones_list;
mod hide_promo_data;

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{Json, NearestDc, Timezone};

    #[tokio::test]
    async fn sends_requests_and_parses_replies() {
        let config = base::JSONValue::JsonObject(Box::new(types::JsonObject { value: Vec::new() }));
        let config = base::help::AppConfig::AppConfig(Box::new(types::help::AppConfig { hash: 1, config }));
        let dc = types::NearestDc { country: "NL".into(), this_dc: 2, nearest_dc: 4 };
        let utc = types::Timezone { id: "UTC".into(), name: "UTC".into(), utc_offset: 3600 };
        let timezones = vec![base::Timezone::Timezone(Box::new(utc))];
        let timezones = types::help::TimezonesList { timezones, hash: 1 };
        let (client, mut sent) = Client::scripted(vec![
            Ok(config.to_bytes()),
            Ok(base::help::CountriesList::CountriesListNotModified.to_bytes()),
            Ok(base::NearestDc::NearestDc(Box::new(dc)).to_bytes()),
            Ok(base::help::TimezonesList::TimezonesList(Box::new(timezones)).to_bytes()),
        ]);

        assert_eq!(client.get_app_config().await.unwrap().json(), &Json::Object(Vec::new()));
        assert_eq!(client.get_countries_list("en").await.unwrap(), []);
        let dc = NearestDc { country: "NL".into(), this_dc: 2, nearest_dc: 4 };
        assert_eq!(client.get_nearest_dc().await.unwrap(), dc);
        let utc = Timezone { id: "UTC".into(), name: "UTC".into(), utc_offset_seconds: 3600 };
        assert_eq!(client.get_timezones_list().await.unwrap(), [utc]);

        assert_eq!(sent.recv().await.unwrap(), functions::help::GetAppConfig { hash: 0 }.to_bytes());
        let countries = functions::help::GetCountriesList { lang_code: "en".into(), hash: 0 };
        assert_eq!(sent.recv().await.unwrap(), countries.to_bytes());
        assert_eq!(sent.recv().await.unwrap(), functions::help::GetNearestDc {}.to_bytes());
        assert_eq!(sent.recv().await.unwrap(), functions::help::GetTimezonesList { hash: 0 }.to_bytes());
    }

    #[tokio::test]
    async fn app_config_is_asked_again_by_hash_and_kept_when_unchanged() {
        use base::JSONValue as J;
        let field = base::JSONObjectValue::JsonObjectValue(Box::new(types::JsonObjectValue {
            key: "reactions_user_max".into(),
            value: J::JsonNumber(types::JsonNumber { value: 3.0 }),
        }));
        let config = J::JsonObject(Box::new(types::JsonObject { value: vec![field] }));
        let config = base::help::AppConfig::AppConfig(Box::new(types::help::AppConfig { hash: 7, config }));
        let (client, mut sent) =
            Client::scripted(vec![Ok(config.to_bytes()), Ok(base::help::AppConfig::AppConfigNotModified.to_bytes())]);

        let first = client.get_app_config().await.unwrap();
        assert_eq!(first.get("reactions_user_max").and_then(Json::as_i64), Some(3));
        assert_eq!(first.get("missing"), None);
        let second = client.get_app_config().await.unwrap();
        assert_eq!(second, first);

        assert_eq!(sent.recv().await.unwrap(), functions::help::GetAppConfig { hash: 0 }.to_bytes());
        assert_eq!(sent.recv().await.unwrap(), functions::help::GetAppConfig { hash: 7 }.to_bytes());
    }

    #[tokio::test]
    async fn config_keeps_the_limits_and_data_centers() {
        let option = types::DcOption {
            ipv6: true,
            media_only: false,
            tcpo_only: false,
            cdn: false,
            r#static: true,
            this_port_only: false,
            id: 2,
            ip_address: "2001:db8::2".into(),
            port: 443,
            secret: None,
        };
        let config = types::Config {
            default_p2p_contacts: false,
            preload_featured_stickers: false,
            revoke_pm_inbox: false,
            blocked_mode: false,
            force_try_ipv6: false,
            date: 1_700_000_000,
            expires: 1_700_003_600,
            test_mode: true,
            this_dc: 2,
            dc_options: vec![base::DcOption::DcOption(Box::new(option))],
            dc_txt_domain_name: String::new(),
            chat_size_max: 200,
            megagroup_size_max: 200_000,
            forwarded_count_max: 100,
            online_update_period_ms: 210_000,
            offline_blur_timeout_ms: 5000,
            offline_idle_timeout_ms: 30_000,
            online_cloud_timeout_ms: 300_000,
            notify_cloud_delay_ms: 30_000,
            notify_default_delay_ms: 1500,
            push_chat_period_ms: 60_000,
            push_chat_limit: 2,
            edit_time_limit: 172_800,
            revoke_time_limit: 86_400,
            revoke_pm_time_limit: i32::MAX,
            rating_e_decay: 2_419_200,
            stickers_recent_limit: 200,
            channels_read_media_period: 604_800,
            tmp_sessions: None,
            call_receive_timeout_ms: 20_000,
            call_ring_timeout_ms: 90_000,
            call_connect_timeout_ms: 30_000,
            call_packet_timeout_ms: 10_000,
            me_url_prefix: "https://t.me/".into(),
            autoupdate_url_prefix: None,
            gif_search_username: None,
            venue_search_username: None,
            img_search_username: None,
            static_maps_provider: None,
            caption_length_max: 1024,
            message_length_max: 4096,
            webfile_dc_id: 4,
            // These three share one flag bit, so they come together or not at all.
            suggested_lang_code: Some("en".into()),
            lang_pack_version: Some(1),
            base_lang_pack_version: Some(1),
            reactions_default: None,
            autologin_token: None,
        };
        let (client, mut sent) = Client::scripted(vec![Ok(base::Config::Config(Box::new(config)).to_bytes())]);

        let config = client.get_config().await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), functions::help::GetConfig {}.to_bytes());
        assert!(config.is_test_mode);
        assert_eq!((config.this_dc, config.expires), (2, 1_700_003_600));
        assert_eq!((config.message_length_max, config.caption_length_max), (4096, 1024));
        assert_eq!((config.edit_time_limit, config.revoke_time_limit), (172_800, 86_400));
        assert_eq!(config.me_url_prefix, "https://t.me/");
        assert_eq!(config.suggested_lang_code.as_deref(), Some("en"));
        let dc = &config.dc_options[0];
        assert_eq!((dc.id, dc.ip_address.as_str(), dc.port), (2, "2001:db8::2", 443));
        assert!(dc.is_ipv6 && dc.is_static && !dc.is_media_only && !dc.is_cdn);
    }

    #[tokio::test]
    async fn promo_chat_can_be_hidden_without_another_lookup() {
        let channel = types::ChannelForbidden {
            broadcast: true,
            megagroup: false,
            monoforum: false,
            id: 5,
            access_hash: 9,
            title: String::new(),
            until_date: None,
        };
        let promo = types::help::PromoData {
            proxy: true,
            expires: 1,
            peer: Some(base::Peer::PeerChannel(types::PeerChannel { channel_id: 5 })),
            psa_type: None,
            psa_message: None,
            pending_suggestions: Vec::new(),
            dismissed_suggestions: Vec::new(),
            custom_pending_suggestion: None,
            chats: vec![base::Chat::ChannelForbidden(Box::new(channel))],
            users: Vec::new(),
        };
        let (client, mut sent) = Client::scripted(vec![
            Ok(base::help::PromoData::PromoData(Box::new(promo)).to_bytes()),
            Ok(true.to_bytes()),
        ]);

        let promo = client.get_promo_data().await.unwrap().unwrap();
        let chat = promo.chat.unwrap();
        assert_eq!((chat.id, promo.is_proxy_sponsor), (-1_000_000_000_005, true));
        client.hide_promo_data(&chat).await.unwrap();

        assert_eq!(sent.recv().await.unwrap(), functions::help::GetPromoData {}.to_bytes());
        let peer = types::InputPeerChannel { channel_id: 5, access_hash: 9 };
        let peer = base::InputPeer::InputPeerChannel(Box::new(peer));
        assert_eq!(sent.recv().await.unwrap(), functions::help::HidePromoData { peer }.to_bytes());
    }
}
