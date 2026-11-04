use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{PeerRef, User};

impl Client {
    /// Uses a token from `user_id` to message them for free. `token` is the token itself or the whole link: the
    /// `free` value of its query is taken.
    pub async fn import_no_paid_messages_token(&self, user_id: impl Into<PeerRef>, token: &str) -> Result<User> {
        let request = functions::contacts::ImportNoPaidMessagesToken {
            user_id: self.resolve_user(&user_id.into()).await?,
            token: free_token(token).to_owned(),
        };
        let user: base::User = self.invoke(&request).await?;
        User::parse_all(self, vec![user]).pop().ok_or(Error::Unexpected("the token's owner came back empty"))
    }
}

// A link carries the token as `free=<token>`; anything else is the token.
fn free_token(token: &str) -> &str {
    let token = token.trim();
    let Some((_, query)) = token.split_once('?') else { return token };
    let query = query.split('#').next().unwrap_or_default();
    query.split('&').find_map(|pair| pair.strip_prefix("free=")).unwrap_or(token)
}

#[cfg(test)]
mod tests {
    use super::free_token;
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[test]
    fn import_extracts_free_param() {
        assert_eq!(free_token("abc"), "abc");
        assert_eq!(free_token(" abc "), "abc");
        assert_eq!(free_token("https://t.me/bob?free=abc"), "abc");
        assert_eq!(free_token("https://t.me/bob?x=1&free=abc&y=2#z"), "abc");
        assert_eq!(free_token("https://t.me/bob?x=1"), "https://t.me/bob?x=1");
    }

    #[tokio::test]
    async fn import_sends_the_token_for_the_user() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let peer = types::InputPeerUser { user_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(9, base::InputPeer::InputPeerUser(Box::new(peer)));
        let _ = client.import_no_paid_messages_token(9_i64, "https://t.me/bob?free=abc").await;
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 9, access_hash: 1 }));
        let request = functions::contacts::ImportNoPaidMessagesToken { user_id, token: "abc".into() };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
