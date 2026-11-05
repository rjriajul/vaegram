use crate::raw::{base, types};

/// A device or app that is logged in to the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    /// Authorization hash that identifies the session. Pass it to `terminate_session` to log the session out.
    pub id: i64,
    /// Device model the app reported when it logged in.
    pub device_model: String,
    /// Platform the app reported, for example `Android`.
    pub platform: String,
    /// Operating system version the app reported.
    pub system_version: String,
    /// The `api_id` the session logged in with.
    pub api_id: i32,
    /// Name of the app the session logged in with.
    pub application_name: String,
    /// Version of the app the session logged in with.
    pub application_version: String,
    /// Unix time of the login.
    pub log_in_date: i32,
    /// Unix time of the session's last activity.
    pub last_active_date: i32,
    /// IP address Telegram saw the session use.
    pub ip_address: String,
    /// Region and country, joined as `Region, Country`. Holds whichever one Telegram sent if the other is empty.
    pub location: String,
    /// The session is the one this client runs on.
    pub is_current: bool,
    /// The app is an official Telegram app.
    pub is_official_application: bool,
    /// The session still has to enter the two-step verification password to finish logging in.
    pub is_password_pending: bool,
    /// The login is new and the account owner hasn't confirmed it.
    pub is_unconfirmed: bool,
    /// The session accepts secret chat requests.
    pub can_accept_secret_chats: bool,
    /// The session accepts incoming calls.
    pub can_accept_calls: bool,
}

/// The account's logged-in sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSessions {
    /// Days of inactivity after which Telegram ends a session on its own.
    pub inactive_session_ttl_days: i32,
    /// All sessions, including the current one.
    pub active_sessions: Vec<ActiveSession>,
}

impl ActiveSession {
    pub(crate) fn parse(a: types::Authorization) -> Self {
        let location = match (a.region.is_empty(), a.country.is_empty()) {
            (false, false) => format!("{}, {}", a.region, a.country),
            (true, _) => a.country,
            (false, true) => a.region,
        };
        ActiveSession {
            id: a.hash,
            device_model: a.device_model,
            platform: a.platform,
            system_version: a.system_version,
            api_id: a.api_id,
            application_name: a.app_name,
            application_version: a.app_version,
            log_in_date: a.date_created,
            last_active_date: a.date_active,
            ip_address: a.ip,
            location,
            is_current: a.current,
            is_official_application: a.official_app,
            is_password_pending: a.password_pending,
            is_unconfirmed: a.unconfirmed,
            can_accept_secret_chats: !a.encrypted_requests_disabled,
            can_accept_calls: !a.call_requests_disabled,
        }
    }
}

impl ActiveSessions {
    pub(crate) fn parse(raw: base::account::Authorizations) -> Self {
        let base::account::Authorizations::Authorizations(raw) = raw;
        ActiveSessions {
            inactive_session_ttl_days: raw.authorization_ttl_days,
            active_sessions: raw
                .authorizations
                .into_iter()
                .map(|base::Authorization::Authorization(a)| ActiveSession::parse(*a))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveSessions;
    use crate::raw::{base, types};

    #[test]
    fn maps_flags_and_location() {
        let a = types::Authorization {
            current: true,
            official_app: false,
            password_pending: false,
            encrypted_requests_disabled: true,
            call_requests_disabled: false,
            unconfirmed: true,
            hash: 42,
            device_model: "Pixel".into(),
            platform: "Android".into(),
            system_version: "15".into(),
            api_id: 6,
            app_name: "Telegram".into(),
            app_version: "11.0".into(),
            date_created: 1,
            date_active: 2,
            ip: "1.2.3.4".into(),
            country: "Bangladesh".into(),
            region: "Dhaka".into(),
        };
        let raw = types::account::Authorizations {
            authorization_ttl_days: 180,
            authorizations: vec![base::Authorization::Authorization(Box::new(a))],
        };
        let s = ActiveSessions::parse(base::account::Authorizations::Authorizations(Box::new(raw)));
        assert_eq!(s.inactive_session_ttl_days, 180);
        let one = &s.active_sessions[0];
        assert_eq!((one.id, one.location.as_str()), (42, "Dhaka, Bangladesh"));
        assert_eq!((one.is_current, one.is_unconfirmed), (true, true));
        assert_eq!((one.can_accept_secret_chats, one.can_accept_calls), (false, true));
    }
}
