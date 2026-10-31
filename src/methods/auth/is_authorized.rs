use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    /// Asks Telegram; 401 answers such as a revoked session or a pending 2FA password give false.
    pub async fn is_authorized(&self) -> Result<bool> {
        match self.invoke(&functions::updates::GetState {}).await {
            Ok(_) => Ok(true),
            Err(Error::Rpc(e)) if e.code == 401 => Ok(false),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};

    fn state() -> Vec<u8> {
        let state = types::updates::State { pts: 1, qts: 0, date: 0, seq: 0, unread_count: 0 };
        base::updates::State::State(Box::new(state)).to_bytes()
    }

    fn logged_in(reply: Result<Vec<u8>>) -> Client {
        let (client, _sent) = Client::scripted(vec![reply]);
        client.0.me.lock().unwrap().id = 1001;
        let peer = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(peer)));
        client
    }

    #[tokio::test]
    async fn asks_for_the_update_state() {
        let (client, mut sent) = Client::recording();
        let _ = client.is_authorized().await;
        assert_eq!(sent.recv().await.unwrap(), functions::updates::GetState {}.to_bytes());
    }

    #[tokio::test]
    async fn any_state_means_authorized() {
        assert!(logged_in(Ok(state())).is_authorized().await.unwrap());
    }

    #[tokio::test]
    async fn unauthorized_answers_are_false_and_change_nothing() {
        for name in ["AUTH_KEY_UNREGISTERED", "SESSION_PASSWORD_NEEDED", "SESSION_REVOKED", "USER_DEACTIVATED"] {
            let client = logged_in(Err(Error::Rpc(RpcError::new(401, name))));
            assert!(!client.is_authorized().await.unwrap(), "{name}");
            // The answer is only reported: the cached account and peers stay for the caller to act on.
            assert_eq!(client.0.me.lock().unwrap().id, 1001);
            assert_eq!(client.0.peers.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn other_errors_stay_errors() {
        for (code, name) in [(400, "X"), (500, "INTERNAL")] {
            let result = logged_in(Err(Error::Rpc(RpcError::new(code, name)))).is_authorized().await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(code, name)), "{result:?}");
        }
    }
}
