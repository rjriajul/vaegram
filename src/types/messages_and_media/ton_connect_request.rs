use crate::raw::types;

/// A request to connect a TON wallet to a mini app, as the service message shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TonConnectRequest {
    /// Telegram's id of the connection session.
    pub session_id: i64,
    /// Unix time after which the request lapses.
    pub expiration_date: i32,
    /// `true` if the user accepted the request.
    pub is_accepted: bool,
    /// `true` if the user declined the request.
    pub is_declined: bool,
    /// Name of the app asking to connect.
    pub dapp_name: Option<String>,
    /// Topic of the connection request.
    pub topic: Option<String>,
    /// Id for tracing the request.
    pub trace_id: Option<String>,
}

impl TonConnectRequest {
    pub(crate) fn parse(raw: &types::MessageActionWalletTonConnectRequest) -> TonConnectRequest {
        TonConnectRequest {
            session_id: raw.session_id,
            expiration_date: raw.expires,
            is_accepted: raw.accepted,
            is_declined: raw.declined,
            dapp_name: raw.dapp_name.clone(),
            topic: raw.topic.clone(),
            trace_id: raw.trace_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TonConnectRequest;
    use crate::raw::types;

    fn raw(accepted: bool, declined: bool) -> types::MessageActionWalletTonConnectRequest {
        types::MessageActionWalletTonConnectRequest {
            accepted,
            declined,
            session_id: 9,
            expires: 100,
            topic: Some("t".into()),
            trace_id: None,
            dapp_name: Some("App".into()),
        }
    }

    #[test]
    fn ton_connect_request_states() {
        let pending = TonConnectRequest::parse(&raw(false, false));
        assert_eq!((pending.is_accepted, pending.is_declined, pending.expiration_date), (false, false, 100));
        assert_eq!(
            (pending.session_id, pending.dapp_name.as_deref(), pending.topic.as_deref()),
            (9, Some("App"), Some("t"))
        );
        assert_eq!(pending.trace_id, None);
        assert!(TonConnectRequest::parse(&raw(true, false)).is_accepted);
        assert!(TonConnectRequest::parse(&raw(false, true)).is_declined);
    }
}
