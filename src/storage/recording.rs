//! Every decrypted MTProto message a client sends or receives, in one file. On Unix only the owner can read it
//! (mode 0600); on Windows it gets the permissions of the directory it's in. `vaegram` and a version byte (1),
//! then per message, little-endian: body length (u32), outgoing (u8), msg_id (i64), unix time in ms (i64), body.
//! Containers are split into their messages. gzip_packed stays packed, except that a message that is itself
//! gzip_packed, or that drops a config's `autologin_token`, is stored unpacked.
//!
//! Credentials are cut down to their constructor id (answers keep `rpc_result` and `req_msg_id` too): all
//! of auth.*, account.* password, email, phone, device and takeout calls and types, wrappers carrying app
//! secrets, bot tokens, SRP proofs, payment credentials and temporary passwords, Passport data, wallet
//! recovery phrases, call relay passwords, login code hashes and URL login codes, every request carrying or
//! returning one of those, every update holding one along with its whole batch, and answers to those requests.
//! An `rpc_error` answer stays whole, since errors hold no secrets. Wrappers, gzip_packed, containers and update
//! batches are looked into first, and anything that can't be is cut too. Server salts are zeroed and the config's
//! `autologin_token` is dropped. Chats, messages and media are kept.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{CREDENTIALS, Deserializable, Error, Identifiable, Serializable, base, functions, types};

const MAGIC: &[u8; 8] = b"vaegram\x01";
const HEADER: usize = 21;
const RPC_RESULT: u32 = 0xf35c6d01;
const RPC_ERROR: u32 = <types::RpcError as Identifiable>::CONSTRUCTOR_ID;
const CONFIG: u32 = <types::Config as Identifiable>::CONSTRUCTOR_ID;
const NEW_SESSION_CREATED: u32 = <types::NewSessionCreated as Identifiable>::CONSTRUCTOR_ID;
const BAD_SERVER_SALT: u32 = <types::BadServerSalt as Identifiable>::CONSTRUCTOR_ID;
const SALTS: [u32; 3] = [NEW_SESSION_CREATED, BAD_SERVER_SALT, 0xae500895];
const MAX_DEPTH: usize = 8;
// A request this many seconds old has timed out, so no answer to it will come.
const HIDDEN_FOR_SECS: i64 = 600;

type Any = functions::Ping;
const INVOKE_AFTER_MSG: u32 = <functions::InvokeAfterMsg<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_AFTER_MSGS: u32 = <functions::InvokeAfterMsgs<Any> as Identifiable>::CONSTRUCTOR_ID;
const INIT_CONNECTION: u32 = <functions::InitConnection<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_LAYER: u32 = <functions::InvokeWithLayer<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITHOUT_UPDATES: u32 = <functions::InvokeWithoutUpdates<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_MESSAGES_RANGE: u32 = <functions::InvokeWithMessagesRange<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_TAKEOUT: u32 = <functions::InvokeWithTakeout<Any> as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_BUSINESS_CONNECTION: u32 =
    <functions::InvokeWithBusinessConnection<Any> as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_SHORT: u32 = <types::UpdateShort as Identifiable>::CONSTRUCTOR_ID;
const UPDATES: u32 = <types::Updates as Identifiable>::CONSTRUCTOR_ID;
const UPDATES_COMBINED: u32 = <types::UpdatesCombined as Identifiable>::CONSTRUCTOR_ID;
const DIFFERENCE: u32 = <types::updates::Difference as Identifiable>::CONSTRUCTOR_ID;
const DIFFERENCE_SLICE: u32 = <types::updates::DifferenceSlice as Identifiable>::CONSTRUCTOR_ID;

/// One message as recorded: `body` is its TL bytes, cut down when it held a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub outgoing: bool,
    pub msg_id: i64,
    pub unix_ms: i64,
    pub body: Vec<u8>,
}

#[cfg(feature = "json")]
impl Record {
    /// One line for a readable dump: `{"outgoing":true,"msg_id":4,"unix_ms":0,"body":{..}}`, the body in the
    /// shape of [`ToJson`](crate::raw::ToJson). A body this layer can't decode, such as one cut down to its
    /// constructor id, is `{"cut":"<TL name>"}`, or `{"cut":<id>}` for an id it doesn't know. A bare vector answer,
    /// whose element type only its request knows, is `{"cut":"vector"}`.
    pub fn to_json(&self) -> String {
        use crate::raw::ToJson;
        let mut out = format!(r#"{{"outgoing":{},"msg_id":"#, self.outgoing);
        self.msg_id.to_json(&mut out);
        out.push_str(r#","unix_ms":"#);
        self.unix_ms.to_json(&mut out);
        out.push_str(r#","body":"#);
        body_json(&self.body, &mut out, 0);
        out.push('}');
        out
    }
}

#[cfg(feature = "json")]
fn body_json(body: &[u8], out: &mut String, depth: usize) {
    use crate::raw::ToJson;
    const VECTOR: u32 = 0x1cb5c415;
    match constructor_id(body) {
        Some(gzip_packed::ID | RPC_RESULT) if depth > MAX_DEPTH => out.push_str(r#"{"cut":"too deep"}"#),
        Some(gzip_packed::ID) => match gzip_packed::unpack(body) {
            Ok(plain) => body_json(&plain, out, depth + 1),
            Err(_) => out.push_str(r#"{"cut":"gzip_packed"}"#),
        },
        Some(RPC_RESULT) if body.len() >= 12 => {
            out.push_str(r#"{"_":"rpc_result","req_msg_id":"#);
            i64::from_le_bytes(body[4..12].try_into().unwrap()).to_json(out);
            out.push_str(r#","result":"#);
            body_json(&body[12..], out, depth + 1);
            out.push('}');
        }
        Some(VECTOR) => out.push_str(r#"{"cut":"vector"}"#),
        Some(id) => match crate::raw::json_of(body) {
            Some(json) => out.push_str(&json),
            None => match crate::raw::tl_name(id) {
                Some(name) => {
                    out.push_str(r#"{"cut":"#);
                    name.to_json(out);
                    out.push('}');
                }
                None => out.push_str(&format!(r#"{{"cut":{id}}}"#)),
            },
        },
        None => out.push_str("null"),
    }
}

// Clones share the file, so every session of a client, including other DCs', lands in one recording.
#[derive(Clone, Debug)]
pub struct Recorder(Arc<Mutex<Log>>);

#[derive(Debug)]
struct Log {
    file: File,
    /// msg_ids of cut requests, whose answers are cut as well.
    hidden: HashSet<i64>,
}

impl Recorder {
    /// Replaces `path`.
    pub fn create(path: &Path) -> io::Result<Recorder> {
        let mut file = super::create_private(path)?;
        file.write_all(MAGIC)?;
        Ok(Recorder(Arc::new(Mutex::new(Log { file, hidden: HashSet::new() }))))
    }

    // One write per record, so a killed process leaves at most a cut tail, which `parse` drops.
    pub(crate) fn record(&self, outgoing: bool, msg_id: i64, body: &[u8]) -> io::Result<()> {
        let unix_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
        let mut log = self.0.lock().unwrap();
        let body = redact(outgoing, msg_id, body, &mut log.hidden);
        let mut buf = Vec::with_capacity(HEADER + body.len());
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.push(outgoing.into());
        buf.extend_from_slice(&msg_id.to_le_bytes());
        buf.extend_from_slice(&unix_ms.to_le_bytes());
        buf.extend_from_slice(&body);
        log.file.write_all(&buf)
    }

    #[cfg(test)]
    pub(crate) fn read_only(path: &Path) -> Recorder {
        let file = File::open(path).unwrap();
        Recorder(Arc::new(Mutex::new(Log { file, hidden: HashSet::new() })))
    }
}

fn redact<'a>(outgoing: bool, msg_id: i64, body: &'a [u8], hidden: &mut HashSet<i64>) -> Cow<'a, [u8]> {
    // A message-level gzip_packed is recorded unpacked, so what it holds is cut and redacted like any other message;
    // replay reads both forms.
    if constructor_id(body) == Some(gzip_packed::ID) {
        if let Ok(plain) = gzip_packed::unpack(body) {
            return Cow::Owned(redact_plain(outgoing, msg_id, &plain, hidden).into_owned());
        }
    }
    redact_plain(outgoing, msg_id, body, hidden)
}

fn redact_plain<'a>(outgoing: bool, msg_id: i64, body: &'a [u8], hidden: &mut HashSet<i64>) -> Cow<'a, [u8]> {
    // Zeroed rather than cut: a replay still has to see the new session to fetch the difference.
    if matches!(constructor_id(body), Some(NEW_SESSION_CREATED | BAD_SERVER_SALT)) && body.len() >= 28 {
        let mut b = body.to_vec();
        b[20..28].fill(0);
        return Cow::Owned(b);
    }
    let cut = secret(body, hidden, 0);
    if let (false, Some(RPC_RESULT), Some(req)) = (outgoing, constructor_id(body), body.get(4..12)) {
        hidden.remove(&i64::from_le_bytes(req.try_into().unwrap()));
    }
    if !cut {
        return match without_autologin(body, 0).filter(|_| !outgoing) {
            Some(clean) => Cow::Owned(clean),
            None => Cow::Borrowed(body),
        };
    }
    if outgoing {
        hidden.retain(|&id| (msg_id >> 32) - (id >> 32) < HIDDEN_FOR_SECS);
        hidden.insert(msg_id);
    }
    Cow::Borrowed(match constructor_id(body) {
        Some(RPC_RESULT) if body.len() >= 12 => &body[..12],
        _ => &body[..4.min(body.len())],
    })
}

fn secret(body: &[u8], hidden: &HashSet<i64>, depth: usize) -> bool {
    let Some(id) = constructor_id(body) else { return false };
    if depth > MAX_DEPTH || CREDENTIALS.binary_search(&id).is_ok() || SALTS.contains(&id) {
        return true;
    }
    let deeper = |b: &[u8]| secret(b, hidden, depth + 1);
    let mut b = &body[4..];
    let skipped = match id {
        gzip_packed::ID => return gzip_packed::unpack(body).map_or(true, |b| deeper(&b)),
        msg_container::ID => return msg_container::parse(body).map_or(true, |ms| ms.iter().any(|m| deeper(m.body))),
        RPC_RESULT => {
            let req = i64::deserialize(&mut b);
            req.map(|req| hidden.contains(&req) && constructor_id(b) != Some(RPC_ERROR))
        }
        INVOKE_WITH_LAYER => i32::deserialize(&mut b).map(|_| false),
        INVOKE_AFTER_MSG | INVOKE_WITH_TAKEOUT => i64::deserialize(&mut b).map(|_| false),
        INVOKE_AFTER_MSGS => Vec::<i64>::deserialize(&mut b).map(|_| false),
        INVOKE_WITHOUT_UPDATES => Ok(false),
        INVOKE_WITH_MESSAGES_RANGE => base::MessageRange::deserialize(&mut b).map(|_| false),
        INVOKE_WITH_BUSINESS_CONNECTION => Vec::<u8>::deserialize(&mut b).map(|_| false),
        INIT_CONNECTION => skip_init_connection(&mut b).map(|()| false),
        // A login code's hash comes as updateSentPhoneCode too; the whole batch goes.
        UPDATE_SHORT | UPDATES | UPDATES_COMBINED => {
            return base::Updates::from_bytes(body).map_or(true, |u| match u {
                base::Updates::UpdateShort(s) => has_credential(std::slice::from_ref(&s.update)),
                base::Updates::Updates(u) => has_credential(&u.updates),
                base::Updates::UpdatesCombined(u) => has_credential(&u.updates),
                _ => false,
            });
        }
        DIFFERENCE | DIFFERENCE_SLICE => {
            return base::updates::Difference::from_bytes(body).map_or(true, |d| match d {
                base::updates::Difference::Difference(d) => has_credential(&d.other_updates),
                base::updates::Difference::DifferenceSlice(d) => has_credential(&d.other_updates),
                _ => false,
            });
        }
        _ => return false,
    };
    skipped.map_or(true, |hit| hit || deeper(b))
}

fn has_credential(updates: &[base::Update]) -> bool {
    updates.iter().any(|u| CREDENTIALS.binary_search(&u.constructor_id()).is_ok())
}

// The config's autologin_token logs its holder into Telegram's web pages; the rest is what a replay needs. Looks
// through rpc_result, gzip_packed and containers, and returns the message without the token, unpacked, if it had one.
fn without_autologin(body: &[u8], depth: usize) -> Option<Vec<u8>> {
    if depth > MAX_DEPTH {
        return None;
    }
    match constructor_id(body)? {
        CONFIG => {
            let base::Config::Config(mut config) = base::Config::from_bytes(body).ok()?;
            config.autologin_token.take()?;
            Some(base::Config::Config(config).to_bytes())
        }
        gzip_packed::ID => without_autologin(&gzip_packed::unpack(body).ok()?, depth + 1),
        RPC_RESULT => Some([body.get(..12)?, &without_autologin(&body[12..], depth + 1)?].concat()),
        msg_container::ID => {
            let messages = msg_container::parse(body).ok()?;
            let clean: Vec<_> = messages.iter().map(|m| without_autologin(m.body, depth + 1)).collect();
            if clean.iter().all(Option::is_none) {
                return None;
            }
            let mut out = [msg_container::ID.to_le_bytes(), (messages.len() as u32).to_le_bytes()].concat();
            for (m, clean) in messages.iter().zip(&clean) {
                let body = clean.as_deref().unwrap_or(m.body);
                out.extend_from_slice(&m.msg_id.to_le_bytes());
                out.extend_from_slice(&m.seq_no.to_le_bytes());
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(body);
            }
            Some(out)
        }
        _ => None,
    }
}

pub(crate) fn skip_init_connection(b: &mut &[u8]) -> Result<(), Error> {
    let flags = u32::deserialize(b)?;
    i32::deserialize(b)?;
    for _ in 0..6 {
        Vec::<u8>::deserialize(b)?;
    }
    if flags & 1 != 0 {
        base::InputClientProxy::deserialize(b)?;
    }
    if flags & 2 != 0 {
        base::JSONValue::deserialize(b)?;
    }
    Ok(())
}

fn constructor_id(body: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(body.get(..4)?.try_into().unwrap()))
}

/// The records of a file written by [`Recorder`]. A record cut short at the end, as a killed process leaves it,
/// is left out without an error.
pub fn read(path: &Path) -> io::Result<Vec<Record>> {
    parse(&std::fs::read(path)?)
        .map_err(|why| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {why}", path.display())))
}

/// [`read`] for bytes already in memory.
pub fn parse(b: &[u8]) -> Result<Vec<Record>, &'static str> {
    let mut b = b.strip_prefix(MAGIC).ok_or("not a vaegram recording")?;
    let mut records = Vec::new();
    while let Some((head, rest)) = b.split_at_checked(HEADER) {
        let len = u32::from_le_bytes(head[..4].try_into().unwrap()) as usize;
        let Some(body) = rest.get(..len) else { break };
        let msg_id = i64::from_le_bytes(head[5..13].try_into().unwrap());
        let unix_ms = i64::from_le_bytes(head[13..].try_into().unwrap());
        records.push(Record { outgoing: head[4] != 0, msg_id, unix_ms, body: body.to_vec() });
        b = &rest[len..];
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::{RPC_RESULT, Record, Recorder, parse, read};
    use crate::raw::core::{gzip_packed, msg_container};
    use crate::raw::{Deserializable, Identifiable, Serializable, base, functions, types};

    fn recorded(name: &str, write: impl FnOnce(&Recorder)) -> Vec<Record> {
        let path = std::env::temp_dir().join(format!("vaegram-{name}-{}.rec", std::process::id()));
        write(&Recorder::create(&path).unwrap());
        let records = read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        records
    }

    fn rpc_result(req_msg_id: i64, result: &[u8]) -> Vec<u8> {
        [&RPC_RESULT.to_le_bytes()[..], &req_msg_id.to_le_bytes()[..], result].concat()
    }

    fn init<X>(query: X) -> functions::InvokeWithLayer<functions::InitConnection<X>> {
        let proxy = types::InputClientProxy { address: "proxy".into(), port: 1080 };
        let query = functions::InitConnection {
            api_id: 1,
            device_model: "d".into(),
            system_version: "s".into(),
            app_version: "a".into(),
            system_lang_code: "en".into(),
            lang_pack: String::new(),
            lang_code: "en".into(),
            proxy: Some(base::InputClientProxy::InputClientProxy(Box::new(proxy))),
            params: None,
            query,
        };
        functions::InvokeWithLayer { layer: 1, query }
    }

    #[test]
    fn round_trips_and_drops_a_cut_tail() {
        let path = std::env::temp_dir().join(format!("vaegram-recording-{}.rec", std::process::id()));
        let recorder = Recorder::create(&path).unwrap();
        recorder.record(true, 4, &[1, 2, 3, 4]).unwrap();
        recorder.clone().record(false, 5, &[]).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let records = parse(&bytes).unwrap();
        let summary: Vec<_> = records.iter().map(|r| (r.outgoing, r.msg_id, r.body.as_slice())).collect();
        assert_eq!(summary, [(true, 4, &[1, 2, 3, 4][..]), (false, 5, &[][..])]);
        assert!(records[0].unix_ms > 1_600_000_000_000);
        assert_eq!(bytes.len(), 8 + 21 + 4 + 21);

        bytes.truncate(bytes.len() - 1);
        assert_eq!(parse(&bytes).unwrap().len(), 1);
        assert_eq!(parse(b"vaegram\x02"), Err("not a vaegram recording"));
    }

    #[test]
    fn cuts_a_wrapped_password_change_and_its_answer() {
        let settings = types::account::PasswordInputSettings {
            new_algo: None,
            new_password_hash: Some(vec![9; 32]),
            hint: None,
            email: None,
            new_secure_settings: None,
        };
        let update = functions::account::UpdatePasswordSettings {
            password: base::InputCheckPasswordSRP::InputCheckPasswordEmpty,
            new_settings: base::account::PasswordInputSettings::PasswordInputSettings(Box::new(settings)),
        };
        let request = functions::InvokeWithTakeout { takeout_id: 7, query: init(update) }.to_bytes();
        let config = init(functions::help::GetConfig {}).to_bytes();
        let records = recorded("password", |r| {
            r.record(true, 4, &request).unwrap();
            r.record(false, 5, &rpc_result(4, &true.to_bytes())).unwrap();
            r.record(true, 8, &config).unwrap();
            r.record(false, 9, &rpc_result(8, &true.to_bytes())).unwrap();
        });
        let bodies: Vec<&[u8]> = records.iter().map(|r| r.body.as_slice()).collect();
        let answer = rpc_result(4, &[]);
        assert_eq!(bodies, [&request[..4], &answer[..], &config[..], &rpc_result(8, &true.to_bytes())[..]]);
    }

    #[test]
    fn cuts_exported_authorizations_login_tokens_and_salts() {
        let exported = types::auth::ExportedAuthorization { id: 1, bytes: vec![7; 256] };
        let exported = base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(exported));
        let token = types::auth::LoginToken { expires: 1, token: vec![7; 32] };
        let token = base::auth::LoginToken::LoginToken(Box::new(token));
        let import = functions::auth::ImportLoginToken { token: vec![7; 32] }.to_bytes();
        let session = |server_salt| {
            let s = types::NewSessionCreated { first_msg_id: 4, unique_id: 1, server_salt };
            base::NewSession::NewSessionCreated(Box::new(s)).to_bytes()
        };
        let records = recorded("tokens", |r| {
            r.record(false, 1, &rpc_result(100, &exported.to_bytes())).unwrap();
            r.record(false, 5, &rpc_result(104, &token.to_bytes())).unwrap();
            r.record(true, 8, &import).unwrap();
            r.record(false, 9, &session(77)).unwrap();
        });
        let bodies: Vec<&[u8]> = records.iter().map(|r| r.body.as_slice()).collect();
        let (export, login) = (rpc_result(100, &[]), rpc_result(104, &[]));
        assert_eq!(bodies, [&export[..], &login[..], &import[..4], &session(0)[..]]);
        assert_eq!(import[..4], functions::auth::ImportLoginToken::CONSTRUCTOR_ID.to_le_bytes());
    }

    #[test]
    fn cuts_bot_tokens_srp_proofs_and_payment_and_push_secrets() {
        let export = functions::bots::ExportBotToken { bot: base::InputUser::InputUserSelf, revoke: false }.to_bytes();
        let token = types::bots::ExportedBotToken { token: "4242:AA-secret".into() };
        let token = base::bots::ExportedBotToken::ExportedBotToken(Box::new(token));
        let mut srp = Vec::new();
        0xd27ff082u32.serialize(&mut srp);
        7i64.serialize(&mut srp);
        vec![0x3cu8; 256].serialize(&mut srp);
        vec![0xc3u8; 32].serialize(&mut srp);
        let answer = functions::messages::GetBotCallbackAnswer {
            game: false,
            peer: base::InputPeer::InputPeerSelf,
            msg_id: 1,
            data: None,
            password: Some(base::InputCheckPasswordSRP::from_bytes(&srp).unwrap()),
        }
        .to_bytes();
        // inputSavedStarGiftUser#69279795 msg_id:1
        let gift = base::InputSavedStarGift::from_bytes(&[0x95, 0x97, 0x27, 0x69, 1, 0, 0, 0]).unwrap();
        let withdraw =
            functions::payments::WithdrawStarGift { keep_on_profile: false, stargift: gift, tmp_password: vec![9; 32] };
        let device = functions::account::RegisterDevice {
            no_muted: false,
            token_type: 2,
            token: "push".into(),
            app_sandbox: false,
            secret: vec![9; 32],
            other_uids: Vec::new(),
        };
        let (withdraw, device) = (withdraw.to_bytes(), device.to_bytes());
        let records = recorded("bot-tokens", |r| {
            r.record(true, 100, &export).unwrap();
            r.record(false, 1, &rpc_result(100, &token.to_bytes())).unwrap();
            for (msg_id, body) in [(104, &answer), (108, &withdraw), (112, &device)] {
                r.record(true, msg_id, body).unwrap();
            }
        });
        let bodies: Vec<&[u8]> = records.iter().map(|r| r.body.as_slice()).collect();
        let cut = rpc_result(100, &[]);
        assert_eq!(bodies, [&export[..4], &cut[..], &answer[..4], &withdraw[..4], &device[..4]]);
    }

    #[test]
    fn keeps_errors_and_forgets_requests_that_timed_out() {
        let import = functions::auth::ImportLoginToken { token: vec![7; 32] }.to_bytes();
        let error = types::RpcError { error_code: 400, error_message: b"AUTH_TOKEN_EXPIRED".to_vec() };
        let error = rpc_result(4, &base::RpcError::RpcError(Box::new(error)).to_bytes());
        let (old, new) = (1 << 32, 700 << 32);
        let records = recorded("errors", |r| {
            r.record(true, 4, &import).unwrap();
            r.record(false, 5, &error).unwrap();
            r.record(true, old, &import).unwrap();
            r.record(true, new, &import).unwrap();
            r.record(false, 9, &rpc_result(old, &true.to_bytes())).unwrap();
            r.record(false, 13, &rpc_result(new, &true.to_bytes())).unwrap();
        });
        let bodies: Vec<&[u8]> = records.iter().map(|r| r.body.as_slice()).collect();
        let (late, answer) = (rpc_result(old, &true.to_bytes()), rpc_result(new, &[]));
        assert_eq!(bodies, [&import[..4], &error[..], &import[..4], &import[..4], &late[..], &answer[..]]);
    }

    fn gzip(body: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(body).unwrap();
        let mut out = gzip_packed::ID.to_le_bytes().to_vec();
        gz.finish().unwrap().serialize(&mut out);
        out
    }

    fn config() -> base::Config {
        base::Config::Config(Box::new(types::Config {
            default_p2p_contacts: false,
            preload_featured_stickers: false,
            revoke_pm_inbox: false,
            blocked_mode: false,
            force_try_ipv6: false,
            date: 1_700_000_000,
            expires: 1_700_003_600,
            test_mode: false,
            this_dc: 2,
            dc_options: Vec::new(),
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
            revoke_time_limit: i32::MAX,
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
            suggested_lang_code: None,
            lang_pack_version: None,
            base_lang_pack_version: None,
            reactions_default: None,
            autologin_token: Some("autologin-secret".into()),
        }))
    }

    fn container(messages: &[(i64, &[u8])]) -> Vec<u8> {
        let mut out = [msg_container::ID.to_le_bytes(), (messages.len() as u32).to_le_bytes()].concat();
        for &(msg_id, body) in messages {
            out.extend_from_slice(&msg_id.to_le_bytes());
            out.extend_from_slice(&1i32.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(body);
        }
        out
    }

    #[cfg(feature = "json")]
    #[test]
    fn a_dump_decodes_calls_and_names_what_it_cannot() {
        let json = |body: Vec<u8>| Record { outgoing: true, msg_id: 4, unix_ms: 0, body }.to_json();
        let init = crate::session::init_connection(1, &Default::default(), functions::help::GetConfig {});
        let init = json(init.to_bytes());
        for part in [r#""_":"invokeWithLayer""#, r#""query":{"_":"initConnection""#, r#""query":{"_":"help.getConfig"}"#] {
            assert!(init.contains(part), "{part} missing in {init}");
        }
        let users = json(functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] }.to_bytes());
        assert!(users.ends_with(r#""body":{"_":"users.getUsers","id":[{"_":"inputUserSelf"}]}}"#), "{users}");
        let vector = json(rpc_result(8, &Vec::<i64>::new().to_bytes()));
        assert!(vector.ends_with(r#""result":{"cut":"vector"}}}"#), "{vector}");
    }

    #[cfg(feature = "json")]
    #[test]
    fn a_dump_stops_at_nested_gzip_past_the_depth_limit() {
        let mut body = functions::updates::GetState {}.to_bytes();
        for _ in 0..=super::MAX_DEPTH + 1 {
            body = gzip(&body);
        }
        let json = Record { outgoing: false, msg_id: 1, unix_ms: 0, body }.to_json();
        assert!(json.ends_with(r#""body":{"cut":"too deep"}}"#), "{json}");
    }

    #[test]
    fn autologin_token_is_cut_in_every_wrapping() {
        let answer = rpc_result(8, &config().to_bytes());
        let inner_gzip = rpc_result(8, &gzip(&config().to_bytes()));
        let shapes = [
            answer.clone(),
            inner_gzip.clone(),
            gzip(&answer),
            gzip(&inner_gzip),
            container(&[(5, &b"\x01\x02\x03\x04"[..]), (9, &answer)]),
            gzip(&container(&[(9, &gzip(&answer))])),
        ];
        let records = recorded("autologin", |r| {
            for (i, body) in shapes.iter().enumerate() {
                r.record(false, 1 + 4 * i as i64, body).unwrap();
            }
        });
        let token = |body: &[u8]| body.windows(16).any(|w| w == b"autologin-secret");
        let unpacked = |body: &[u8]| match gzip_packed::unpack(body) {
            Ok(plain) => plain,
            Err(_) => body.to_vec(),
        };
        for record in &records {
            assert!(!token(&unpacked(&record.body)), "{:02x?}", &record.body[..16]);
        }
        let mut clean = config();
        let base::Config::Config(c) = &mut clean;
        c.autologin_token = None;
        let clean = rpc_result(8, &clean.to_bytes());
        assert_eq!([&records[0].body, &records[1].body, &records[2].body, &records[3].body], [&clean; 4]);
        assert_eq!(records[4].body, container(&[(5, &b"\x01\x02\x03\x04"[..]), (9, &clean)]));
        assert_eq!(records[5].body, container(&[(9, &clean)]));
    }

    #[test]
    fn a_cut_answer_inside_a_message_level_gzip_keeps_its_rpc_result_header() {
        let import = functions::auth::ImportLoginToken { token: vec![7; 32] }.to_bytes();
        let token = types::auth::LoginToken { expires: 1, token: vec![7; 32] };
        let token = base::auth::LoginToken::LoginToken(Box::new(token));
        let records = recorded("gzip-cut", |r| {
            r.record(true, 100, &import).unwrap();
            r.record(false, 1, &gzip(&rpc_result(100, &token.to_bytes()))).unwrap();
        });
        assert_eq!(records[1].body, rpc_result(100, &[]));
    }

    #[test]
    fn login_code_hashes_inside_updates_are_cut() {
        let sent = types::auth::SentCode {
            r#type: base::auth::SentCodeType::SentCodeTypeApp(types::auth::SentCodeTypeApp { length: 5 }),
            phone_code_hash: "code-hash-secret".into(),
            next_type: None,
            timeout: None,
        };
        let sent_code = base::auth::SentCode::SentCode(Box::new(sent));
        let update = base::Update::UpdateSentPhoneCode(Box::new(types::UpdateSentPhoneCode { sent_code }));
        let short = types::UpdateShort { update: update.clone(), date: 0 };
        let short = base::Updates::UpdateShort(Box::new(short)).to_bytes();
        let batch = |updates| types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
        let updates = base::Updates::Updates(Box::new(batch(vec![update.clone()]))).to_bytes();
        let state = types::updates::State { pts: 1, qts: 0, date: 0, seq: 0, unread_count: 0 };
        let difference = types::updates::Difference {
            new_messages: Vec::new(),
            new_encrypted_messages: Vec::new(),
            other_updates: vec![update],
            chats: Vec::new(),
            users: Vec::new(),
            state: base::updates::State::State(Box::new(state)),
        };
        let difference = rpc_result(9, &base::updates::Difference::Difference(Box::new(difference)).to_bytes());
        let empty = base::Updates::Updates(Box::new(batch(Vec::new()))).to_bytes();
        let records = recorded("sent-code", |r| {
            for body in [&short, &updates, &difference, &empty] {
                r.record(false, 9, body).unwrap();
            }
        });
        let kept = |r: &Record| r.body.windows(16).any(|w| w == b"code-hash-secret");
        assert!(!records.iter().any(kept), "a login code hash inside an update was recorded");
        assert_eq!(records[3].body, empty, "updates without credentials are kept whole");
    }

    #[test]
    fn a_failed_write_is_an_error() {
        let path = std::env::temp_dir().join(format!("vaegram-read-only-{}.rec", std::process::id()));
        std::fs::write(&path, b"").unwrap();
        let written = Recorder::read_only(&path).record(true, 4, &[1, 2, 3, 4]);
        std::fs::remove_file(&path).unwrap();
        assert!(written.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn recordings_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("vaegram-mode-{}.rec", std::process::id()));
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        drop(Recorder::create(&path).unwrap());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(mode & 0o777, 0o600);
    }
}
