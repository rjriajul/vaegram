use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: i64,
    pub device_model: String,
    pub platform: String,
    pub system_version: String,
    pub api_id: i32,
    pub application_name: String,
    pub application_version: String,
    pub log_in_date: i32,
    pub last_active_date: i32,
    pub ip_address: String,
    pub location: String,
    pub is_current: bool,
    pub is_official_application: bool,
    pub is_password_pending: bool,
    pub is_unconfirmed: bool,
    pub can_accept_secret_chats: bool,
    pub can_accept_calls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSessions {
    pub inactive_session_ttl_days: i32,
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
