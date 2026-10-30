mod get_app_config;
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

        assert_eq!(client.get_app_config().await.unwrap(), Json::Object(Vec::new()));
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
        assert_eq!((chat.id, promo.proxy), (-1_000_000_000_005, true));
        assert!(client.hide_promo_data(&chat).await.unwrap());

        assert_eq!(sent.recv().await.unwrap(), functions::help::GetPromoData {}.to_bytes());
        let peer = types::InputPeerChannel { channel_id: 5, access_hash: 9 };
        let peer = base::InputPeer::InputPeerChannel(Box::new(peer));
        assert_eq!(sent.recv().await.unwrap(), functions::help::HidePromoData { peer }.to_bytes());
    }
}
