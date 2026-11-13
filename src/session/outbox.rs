use std::sync::Arc;

use super::{ClientEvent, Pending, Runner, constructor_id};
use crate::crypto::mtproto;
use crate::errors::Result;
use crate::raw::core::msg_container;
use crate::raw::{Identifiable, Serializable, functions, types};

// The spec allows 1024 messages per container; these limits stay below it. A body over the size cap
// goes alone, so big uploads don't hold up small requests.
// https://core.telegram.org/mtproto/service_messages#containers
pub(super) const MAX_CONTAINER_MSGS: usize = 1000;
pub(super) const MAX_CONTAINER_BYTES: usize = 1 << 15;
pub(super) const MAX_IDS: usize = 8192;
// msg_id, seqno and length in front of each message in a container.
const HEADER: usize = 16;
const GET_FILE: u32 = <functions::upload::GetFile as Identifiable>::CONSTRUCTOR_ID;
const TAKEOUT: u32 = <functions::InvokeWithTakeout<functions::Ping> as Identifiable>::CONSTRUCTOR_ID;

// Two upload.getFile in one container draw FLOOD_WAITs, as mtcute found, so each goes in a container of its own.
fn is_get_file(body: &[u8]) -> bool {
    match constructor_id(body) {
        Some(GET_FILE) => true,
        // Inside a takeout session: the constructor and the takeout id come first.
        Some(TAKEOUT) => constructor_id(body.get(12..).unwrap_or_default()) == Some(GET_FILE),
        _ => false,
    }
}

fn put_header(buf: &mut Vec<u8>, msg_id: i64, seq_no: i32, len: usize) {
    buf.extend_from_slice(&msg_id.to_le_bytes());
    buf.extend_from_slice(&seq_no.to_le_bytes());
    buf.extend_from_slice(&(len as u32).to_le_bytes());
}

impl Runner {
    pub(super) async fn flush(&mut self) -> Result<()> {
        if self.bind_packet() {
            self.writer.finish().await?;
        }
        while self.pack() {
            self.writer.finish().await?;
        }
        Ok(())
    }

    fn begin(&mut self) -> usize {
        self.writer.begin();
        let at = self.writer.buf.len();
        self.writer.buf.resize(at + mtproto::HEADER, 0);
        at
    }

    pub(super) fn seal(&mut self, msg_id: i64, seq_no: i32, body: &[u8]) {
        let at = self.begin();
        self.writer.buf.extend_from_slice(body);
        self.seal_at(at, msg_id, seq_no);
    }

    fn seal_at(&mut self, at: usize, msg_id: i64, seq_no: i32) {
        let salt = self.salt();
        let buf = &mut self.writer.buf;
        if let Some(r) = &self.options.record {
            let body = &buf[at + mtproto::HEADER..];
            let written = if constructor_id(body) == Some(msg_container::ID) {
                let messages = msg_container::parse(body).unwrap_or_default();
                messages.iter().try_for_each(|m| r.record(true, m.msg_id, m.body))
            } else {
                r.record(true, msg_id, body)
            };
            if let Err(error) = written {
                self.events.emit(ClientEvent::RecordingFailed { error: Arc::new(error) });
            }
        }
        mtproto::header(&mut buf[at..], salt, &self.session_id, msg_id, seq_no);
        mtproto::seal(buf, at, &self.auth_key, &self.auth_key_id);
    }

    // Requests move to `pending` before the write, so a failed write re-sends them after the reconnect.
    fn pack(&mut self) -> bool {
        let mut msgs: Vec<(Vec<u8>, bool, Option<Pending>)> = Vec::new();
        let mut size = 0;
        if !self.acks.is_empty() {
            let n = self.acks.len().min(MAX_IDS);
            let mut body = Vec::new();
            types::MsgsAck { msg_ids: self.acks.drain(..n).collect() }.serialize(&mut body);
            size += HEADER + body.len();
            msgs.push((body, false, None));
        }
        let mut get_file = false;
        loop {
            let (len, asks_file) = match (self.service.front(), self.queue.front()) {
                (Some((body, _)), _) => (body.len(), false),
                // No API request goes out under a temp key that isn't bound yet or is being replaced: the bind goes
                // alone, service messages (acks, pings, state requests) carry no API call.
                (None, Some(p)) if !self.temp_key_stale() => (p.body.len(), is_get_file(&p.body)),
                _ => break,
            };
            let full = msgs.len() == MAX_CONTAINER_MSGS || size + HEADER + len > MAX_CONTAINER_BYTES;
            if (!msgs.is_empty() && full) || (get_file && asks_file) {
                break;
            }
            get_file |= asks_file;
            size += HEADER + len;
            msgs.push(match self.service.pop_front() {
                Some((body, content_related)) => (body, content_related, None),
                None => {
                    let Some(mut p) = self.queue.pop_front() else { break };
                    (std::mem::take(&mut p.body), true, Some(p))
                }
            });
        }

        if msgs.len() <= 1 {
            let Some((body, content_related, pending)) = msgs.pop() else { return false };
            let (msg_id, seq_no) = (self.msg_ids.next(), self.seq.next(content_related));
            self.seal(msg_id, seq_no, &body);
            self.sent(msg_id, msg_id, body, pending);
            return true;
        }
        // Inner msg_ids must be lower than the container's.
        let mut msg_id = self.msg_ids.next_n(msgs.len() + 1);
        let container = msg_id + 4 * msgs.len() as i64;
        let at = self.begin();
        let mut buf = std::mem::take(&mut self.writer.buf);
        buf.extend_from_slice(&msg_container::ID.to_le_bytes());
        buf.extend_from_slice(&(msgs.len() as u32).to_le_bytes());
        for (body, content_related, pending) in msgs {
            put_header(&mut buf, msg_id, self.seq.next(content_related), body.len());
            buf.extend_from_slice(&body);
            self.sent(msg_id, container, body, pending);
            msg_id += 4;
        }
        self.writer.buf = buf;
        let seq_no = self.seq.next(false);
        self.seal_at(at, container, seq_no);
        true
    }

    fn sent(&mut self, msg_id: i64, container: i64, body: Vec<u8>, pending: Option<Pending>) {
        if let Some(p) = pending {
            self.pending.insert(msg_id, Pending { body, container, acked: false, ..p });
        }
    }
}
