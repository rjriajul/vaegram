use super::{ClientEvent, Pending, Runner, constructor_id};
use crate::crypto::mtproto;
use crate::errors::Result;
use crate::raw::core::msg_container;
use crate::raw::{Serializable, types};

// The spec allows 1024 messages per container; these limits stay below it. A body over the size cap
// goes alone, so big uploads don't hold up small requests.
// https://core.telegram.org/mtproto/service_messages#containers
pub(super) const MAX_CONTAINER_MSGS: usize = 1000;
pub(super) const MAX_CONTAINER_BYTES: usize = 1 << 15;
pub(super) const MAX_IDS: usize = 8192;
// msg_id, seqno and length in front of each message in a container.
const HEADER: usize = 16;

fn put_header(buf: &mut Vec<u8>, msg_id: i64, seq_no: i32, len: usize) {
    buf.extend_from_slice(&msg_id.to_le_bytes());
    buf.extend_from_slice(&seq_no.to_le_bytes());
    buf.extend_from_slice(&(len as u32).to_le_bytes());
}

impl Runner {
    pub(super) async fn flush(&mut self) -> Result<()> {
        if let Some(packet) = self.bind_packet() {
            self.writer.send(&packet).await?;
        }
        while let Some(packet) = self.pack() {
            self.writer.send(&packet).await?;
        }
        Ok(())
    }

    pub(super) fn encrypt(&self, msg_id: i64, seq_no: i32, body: &[u8]) -> Vec<u8> {
        if let Some(r) = &self.options.record {
            let written = if constructor_id(body) == Some(msg_container::ID) {
                let messages = msg_container::parse(body).unwrap_or_default();
                messages.iter().try_for_each(|m| r.record(true, m.msg_id, m.body))
            } else {
                r.record(true, msg_id, body)
            };
            if let Err(error) = written {
                self.events.emit(ClientEvent::RecordingFailed { error });
            }
        }
        mtproto::pack(&self.auth_key, &self.auth_key_id, &self.session_id, self.salt(), msg_id, seq_no, body)
    }

    // Requests move to `pending` before the write, so a failed write re-sends them after the reconnect.
    fn pack(&mut self) -> Option<Vec<u8>> {
        let mut msgs: Vec<(Vec<u8>, bool, Option<Pending>)> = Vec::new();
        let mut size = 0;
        if !self.acks.is_empty() {
            let n = self.acks.len().min(MAX_IDS);
            let mut body = Vec::new();
            types::MsgsAck { msg_ids: self.acks.drain(..n).collect() }.serialize(&mut body);
            size += HEADER + body.len();
            msgs.push((body, false, None));
        }
        loop {
            let len = match (self.service.front(), self.queue.front()) {
                (Some((body, _)), _) => body.len(),
                // No API request goes out under a temp key that isn't bound yet: the bind goes alone, service
                // messages (acks, pings, state requests) carry no API call.
                (None, Some(p)) if !self.unbound() => p.body.len(),
                _ => break,
            };
            if msgs.len() == MAX_CONTAINER_MSGS || (!msgs.is_empty() && size + HEADER + len > MAX_CONTAINER_BYTES) {
                break;
            }
            size += HEADER + len;
            msgs.push(match self.service.pop_front() {
                Some((body, content_related)) => (body, content_related, None),
                None => {
                    let mut p = self.queue.pop_front()?;
                    (std::mem::take(&mut p.body), true, Some(p))
                }
            });
        }

        if msgs.len() <= 1 {
            let (body, content_related, pending) = msgs.pop()?;
            let (msg_id, seq_no) = (self.msg_ids.next(), self.seq.next(content_related));
            let packet = self.encrypt(msg_id, seq_no, &body);
            self.sent(msg_id, msg_id, body, pending);
            return Some(packet);
        }
        // Inner msg_ids must be lower than the container's.
        let mut msg_id = self.msg_ids.next_n(msgs.len() + 1);
        let container = msg_id + 4 * msgs.len() as i64;
        let mut buf = std::mem::take(&mut self.buf);
        buf.clear();
        buf.extend_from_slice(&msg_container::ID.to_le_bytes());
        buf.extend_from_slice(&(msgs.len() as u32).to_le_bytes());
        for (body, content_related, pending) in msgs {
            put_header(&mut buf, msg_id, self.seq.next(content_related), body.len());
            buf.extend_from_slice(&body);
            self.sent(msg_id, container, body, pending);
            msg_id += 4;
        }
        let seq_no = self.seq.next(false);
        let packet = self.encrypt(container, seq_no, &buf);
        self.buf = buf;
        Some(packet)
    }

    fn sent(&mut self, msg_id: i64, container: i64, body: Vec<u8>, pending: Option<Pending>) {
        if let Some(p) = pending {
            self.pending.insert(msg_id, Pending { body, container, acked: false, ..p });
        }
    }
}
