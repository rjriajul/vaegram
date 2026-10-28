//! Perfect forward secrecy: <https://core.telegram.org/api/pfs>

use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::Instant;

use super::auth::Created;
use super::internals::{self, SeqNo};
use super::{Answer, Pending, Runner};
use crate::crypto::mtproto::{auth_key_id, encrypt_v1};
use crate::crypto::random;
use crate::errors::Error;
use crate::raw::{Deserializable, Serializable, functions, types};

// The client picks the lifetime; a day keeps rebinds rare.
pub(super) const TEMP_KEY_LIFETIME: i32 = 24 * 60 * 60;
const RENEW_BEFORE: i64 = 60 * 60;
// A brand new permanent key may not have reached every server yet.
const PERM_KEY_PROPAGATION: Duration = Duration::from_secs(60);
const FIRST_BIND_RETRY: Duration = Duration::from_secs(1);

pub(crate) struct Pfs {
    perm_key: [u8; 256],
    perm_fresh_until: Instant,
    init: Vec<u8>,
    _init_answer: Option<oneshot::Receiver<Answer>>,
    expires_at: i64,
    bind: Bind,
    retry: Duration,
    #[cfg(test)]
    pub(super) binds_sent: Vec<i64>,
}

enum Bind {
    Due,
    RetryAt(Instant),
    Sent(i64),
    Done,
    Failed(Error),
    // A new temp key would get the same answer, so the session ends.
    Refused,
}

impl Pfs {
    pub(super) fn new(perm_key: [u8; 256], perm_is_new: bool, init: Vec<u8>, expires_at: i64) -> Pfs {
        let fresh = if perm_is_new { PERM_KEY_PROPAGATION } else { Duration::ZERO };
        Pfs {
            perm_key,
            perm_fresh_until: Instant::now() + fresh,
            init,
            _init_answer: None,
            expires_at,
            bind: Bind::Due,
            retry: FIRST_BIND_RETRY,
            #[cfg(test)]
            binds_sent: Vec::new(),
        }
    }
}

impl Bind {
    fn is_due(&self) -> bool {
        match self {
            Bind::Due => true,
            Bind::RetryAt(at) => *at <= Instant::now(),
            _ => false,
        }
    }
}

fn bind_request(
    perm_key: &[u8; 256],
    temp_key_id: &[u8; 8],
    session_id: &[u8; 8],
    msg_id: i64,
    expires_at: i32,
) -> functions::auth::BindTempAuthKey {
    let nonce = i64::from_le_bytes(random());
    let perm_auth_key_id = i64::from_le_bytes(auth_key_id(perm_key));
    let inner = types::BindAuthKeyInner {
        nonce,
        temp_auth_key_id: i64::from_le_bytes(*temp_key_id),
        perm_auth_key_id,
        temp_session_id: i64::from_le_bytes(*session_id),
        expires_at,
    };
    let encrypted_message = encrypt_v1(perm_key, msg_id, &inner.to_bytes());
    functions::auth::BindTempAuthKey { perm_auth_key_id, nonce, expires_at, encrypted_message }
}

impl Runner {
    fn server_now(&self) -> i64 {
        internals::now() + self.msg_ids.time_offset
    }

    fn bind_is(&self, f: impl Fn(&Bind) -> bool) -> bool {
        self.pfs.as_ref().is_some_and(|p| f(&p.bind))
    }

    pub(super) fn unbound(&self) -> bool {
        self.bind_is(|b| !matches!(b, Bind::Done))
    }

    pub(super) fn bind_due(&self) -> bool {
        self.bind_is(Bind::is_due)
    }

    fn bind_sent_as(&self, msg_id: i64) -> bool {
        self.bind_is(|b| matches!(b, Bind::Sent(id) if *id == msg_id))
    }

    pub(super) fn temp_key_stale(&self) -> bool {
        let renew_at = self.server_now() + RENEW_BEFORE;
        self.pfs.as_ref().is_some_and(|p| !matches!(p.bind, Bind::Done) || p.expires_at <= renew_at)
    }

    /// Time to renew. A new key means a new session, so wait until nothing awaits an answer.
    pub(super) fn temp_key_expiring(&self) -> bool {
        self.pending.is_empty() && !self.unbound() && self.temp_key_stale()
    }

    /// A 404 for a temp key, bound or still binding, only means the server dropped it, so a new one is made; the
    /// permanent key is fine.
    pub(super) fn ends_session(&mut self, e: &Error) -> bool {
        let not_found = matches!(e, Error::Transport(404));
        match &mut self.pfs {
            Some(p) if matches!(p.bind, Bind::Refused) => true,
            Some(p) if not_found && !matches!(p.bind, Bind::Failed(_)) => {
                p.expires_at = 0;
                false
            }
            _ => not_found,
        }
    }

    pub(super) fn use_temp_key(&mut self, key: Created) {
        self.auth_key_id = auth_key_id(&key.auth_key);
        self.auth_key = key.auth_key;
        self.set_salt(key.salt);
        self.set_time_offset(key.time_offset);
        self.clock_synced = true;
        self.session_id = random();
        self.seq = SeqNo::default();
        let expires_at = self.server_now() + TEMP_KEY_LIFETIME as i64;
        if let Some(p) = &mut self.pfs {
            p.expires_at = expires_at;
            p.bind = Bind::Due;
            p.retry = FIRST_BIND_RETRY;
        }
    }

    /// The bind packet, when one is due. It goes alone because its msg_id is sealed inside.
    pub(super) fn bind_packet(&mut self) -> Option<Vec<u8>> {
        let p = self.pfs.as_mut().filter(|p| p.bind.is_due())?;
        let msg_id = self.msg_ids.next();
        #[cfg(test)]
        p.binds_sent.push(msg_id);
        let request = bind_request(&p.perm_key, &self.auth_key_id, &self.session_id, msg_id, p.expires_at as i32);
        p.bind = Bind::Sent(msg_id);
        let seq_no = self.seq.next(true);
        Some(self.encrypt(msg_id, seq_no, &request.to_bytes()))
    }

    pub(super) fn rebind_if_sent_as(&mut self, msg_id: i64) -> bool {
        let lost = self.bind_sent_as(msg_id);
        if let (true, Some(p)) = (lost, &mut self.pfs) {
            p.bind = Bind::Due;
        }
        lost
    }

    pub(super) fn bind_answered(&mut self, req_msg_id: i64, answer: Answer) -> Option<Answer> {
        if !self.bind_sent_as(req_msg_id) {
            return Some(answer);
        }
        let p = self.pfs.as_mut()?;
        p.bind = match answer.and_then(|b| Ok(bool::from_bytes(&b)?)) {
            Ok(true) => {
                let (answer, rx) = oneshot::channel();
                p._init_answer = Some(rx);
                let (name, start, body) = ("invokeWithLayer", std::time::Instant::now(), p.init.clone());
                self.queue.push_front(Pending { name, body, answer, start, container: 0, acked: false });
                Bind::Done
            }
            Ok(false) => Bind::Failed(Error::Security("server refused to bind the temporary key")),
            // Per the spec, only an established permanent key is dead after this; a new one may still be syncing.
            Err(Error::Rpc(e)) if e.name == "ENCRYPTED_MESSAGE_INVALID" => {
                if Instant::now() < p.perm_fresh_until {
                    let at = Instant::now() + p.retry;
                    p.retry *= 2;
                    Bind::RetryAt(at)
                } else {
                    Bind::Failed(Error::Transport(404))
                }
            }
            Err(e) => Bind::Failed(e),
        };
        None
    }

    pub(super) fn bind_failure(&mut self) -> Option<Error> {
        let p = self.pfs.as_mut().filter(|p| matches!(p.bind, Bind::Failed(_)))?;
        match std::mem::replace(&mut p.bind, Bind::Refused) {
            Bind::Failed(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mtproto::kdf_v1;
    use crate::crypto::{aes, sha1};

    fn key() -> [u8; 256] {
        std::array::from_fn(|i| i as u8)
    }

    // The spec's v1 formulas for this key and msg_key = [0xab; 16].
    const KDF_KEY: [u8; 32] = [
        0xd1, 0x4b, 0x59, 0x9a, 0xff, 0x20, 0x86, 0xd3, 0x99, 0x5c, 0xd4, 0x1c, 0x55, 0x2c, 0x82, 0xb2, 0x38, 0x78,
        0x45, 0xde, 0xcb, 0x78, 0x50, 0x89, 0x2c, 0xcf, 0xe7, 0xc8, 0x62, 0x62, 0x08, 0xa0,
    ];
    const KDF_IV: [u8; 32] = [
        0xc6, 0x8d, 0xee, 0x91, 0x9c, 0x27, 0x38, 0xc3, 0x70, 0xe8, 0x49, 0x70, 0x26, 0x82, 0xdc, 0x95, 0x4f, 0xf9,
        0x04, 0x23, 0x5a, 0x2c, 0xc1, 0x85, 0x53, 0x95, 0xf7, 0xea, 0x21, 0x8a, 0x86, 0x1f,
    ];

    #[test]
    fn kdf_v1_matches_reference() {
        assert_eq!(kdf_v1(&key(), &[0xab; 16]), (KDF_KEY, KDF_IV));
    }

    #[test]
    fn bind_message_is_v1_sealed_with_the_perm_key() {
        let perm = key();
        let (temp_id, session_id, msg_id) = ([1; 8], [2; 8], 0x6000_0000_0000_0004);
        let request = bind_request(&perm, &temp_id, &session_id, msg_id, 1_900_000_000);
        let message = &request.encrypted_message;
        assert_eq!(request.perm_auth_key_id, i64::from_le_bytes(auth_key_id(&perm)));
        assert_eq!(message.len(), 8 + 16 + 80);
        assert_eq!(message[..8], auth_key_id(&perm));

        let msg_key: [u8; 16] = message[8..24].try_into().unwrap();
        let (aes_key, iv) = kdf_v1(&perm, &msg_key);
        let mut plain = message[24..].to_vec();
        aes::ige256_decrypt(&mut plain, &aes_key, &iv).unwrap();
        assert_eq!(sha1(&[&plain[..72]])[4..], msg_key);
        assert_eq!(i64::from_le_bytes(plain[16..24].try_into().unwrap()), msg_id);
        assert_eq!(plain[24..28], [0; 4]);
        assert_eq!(u32::from_le_bytes(plain[28..32].try_into().unwrap()), 40);
        let inner = types::BindAuthKeyInner {
            nonce: request.nonce,
            temp_auth_key_id: i64::from_le_bytes(temp_id),
            perm_auth_key_id: request.perm_auth_key_id,
            temp_session_id: i64::from_le_bytes(session_id),
            expires_at: 1_900_000_000,
        };
        assert_eq!(plain[32..72], inner.to_bytes());
        assert_eq!(request.expires_at, 1_900_000_000);
    }
}
