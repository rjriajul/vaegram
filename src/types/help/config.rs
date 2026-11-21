use crate::raw::{base, types};

/// The parts of Telegram's server configuration a client most often needs: the data centers and the limits on
/// chats, messages and edits. [`Client::get_config`](crate::Client::get_config) returns it.
///
/// Times are in seconds and dates are Unix time. Limits for Premium accounts and most feature switches are in the
/// [app config](crate::types::AppConfig) instead.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// When the configuration was made.
    pub date: i32,
    /// When it expires; ask again after that.
    pub expires: i32,
    /// `true` on Telegram's test servers.
    pub is_test_mode: bool,
    /// Id of the data center that answered.
    pub this_dc: i32,
    /// The addresses of every data center.
    pub dc_options: Vec<DcOption>,
    /// Most members a basic group can have.
    pub chat_size_max: i32,
    /// Most members a supergroup can have.
    pub megagroup_size_max: i32,
    /// Most messages one forward can take.
    pub forwarded_count_max: i32,
    /// Most characters in a message's text.
    pub message_length_max: i32,
    /// Most characters in a media caption.
    pub caption_length_max: i32,
    /// How long after sending a message can still be edited.
    pub edit_time_limit: i32,
    /// How long after sending a message can still be deleted for everyone.
    pub revoke_time_limit: i32,
    /// How long after sending a message in a private chat can still be deleted for both sides.
    pub revoke_pm_time_limit: i32,
    /// Most stickers kept in the recently used list.
    pub stickers_recent_limit: i32,
    /// The prefix of `t.me` links, such as `https://t.me/`.
    pub me_url_prefix: String,
    /// The language Telegram suggests for this client. `None` if it suggests none.
    pub suggested_lang_code: Option<String>,
}

/// How to reach one data center.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcOption {
    /// Id of the data center.
    pub id: i32,
    /// Its IP address, v4 or v6.
    pub ip_address: String,
    /// Its port.
    pub port: i32,
    /// `true` for an IPv6 address.
    pub is_ipv6: bool,
    /// `true` if the address serves only file downloads and uploads.
    pub is_media_only: bool,
    /// `true` if the address takes only obfuscated TCP connections.
    pub is_tcpo_only: bool,
    /// `true` for a CDN data center.
    pub is_cdn: bool,
    /// `true` if the address is static: a client may store it.
    pub is_static: bool,
    /// `true` if only this port works for the address, so no other port may be tried.
    pub is_this_port_only: bool,
}

impl Config {
    pub(crate) fn parse(raw: types::Config) -> Config {
        Config {
            date: raw.date,
            expires: raw.expires,
            is_test_mode: raw.test_mode,
            this_dc: raw.this_dc,
            dc_options: raw.dc_options.into_iter().map(DcOption::parse).collect(),
            chat_size_max: raw.chat_size_max,
            megagroup_size_max: raw.megagroup_size_max,
            forwarded_count_max: raw.forwarded_count_max,
            message_length_max: raw.message_length_max,
            caption_length_max: raw.caption_length_max,
            edit_time_limit: raw.edit_time_limit,
            revoke_time_limit: raw.revoke_time_limit,
            revoke_pm_time_limit: raw.revoke_pm_time_limit,
            stickers_recent_limit: raw.stickers_recent_limit,
            me_url_prefix: raw.me_url_prefix,
            suggested_lang_code: raw.suggested_lang_code,
        }
    }
}

impl DcOption {
    fn parse(raw: base::DcOption) -> DcOption {
        let base::DcOption::DcOption(o) = raw;
        DcOption {
            id: o.id,
            ip_address: o.ip_address,
            port: o.port,
            is_ipv6: o.ipv6,
            is_media_only: o.media_only,
            is_tcpo_only: o.tcpo_only,
            is_cdn: o.cdn,
            is_static: o.r#static,
            is_this_port_only: o.this_port_only,
        }
    }
}
