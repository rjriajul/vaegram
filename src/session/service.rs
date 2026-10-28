use super::{Runner, internals};
use crate::errors::Error;
use crate::raw::{Deserializable, Serializable, base, types};

impl Runner {
    pub(super) fn handle_bad_msg(&mut self, server_msg_id: i64, body: &[u8]) {
        match base::BadMsgNotification::from_bytes(body) {
            Ok(base::BadMsgNotification::BadServerSalt(b)) => {
                self.set_salt(b.new_server_salt);
                self.resend(b.bad_msg_id);
            }
            Ok(base::BadMsgNotification::BadMsgNotification(b)) => match b.error_code {
                // Our msg_id is too low or too high: the local clock is off. The server's
                // own msg_id carries its time, so take the offset from it and resend.
                16 | 17 => {
                    self.set_time_offset((server_msg_id >> 32) - internals::now());
                    self.clock_synced = true;
                    self.resend(b.bad_msg_id);
                }
                32 | 33 => {
                    self.seq.resync(b.error_code == 32);
                    self.resend(b.bad_msg_id);
                }
                code => {
                    // A refused msgs_state_req is asked again, like one refused for its salt.
                    if let Some(ids) = self.checking.remove(&b.bad_msg_id) {
                        self.recheck.extend(ids);
                    }
                    for (msg_id, p) in self.take_sent(b.bad_msg_id) {
                        self.finish(msg_id, p, Err(Error::BadMessage(code)));
                    }
                }
            },
            Err(_) => {}
        }
    }

    pub(super) fn handle_new_session(&mut self, body: &[u8]) {
        if let Ok(base::NewSession::NewSessionCreated(s)) = base::NewSession::from_bytes(body) {
            self.set_salt(s.server_salt);
            let lost: Vec<i64> = self.pending.keys().copied().filter(|&id| id < s.first_msg_id).collect();
            for id in lost {
                self.resend(id);
            }
            self.request_difference();
        }
    }

    pub(super) fn handle_msgs_ack(&mut self, body: &[u8]) {
        if let Ok(base::MsgsAck::MsgsAck(a)) = base::MsgsAck::from_bytes(body) {
            for id in a.msg_ids {
                if let Some(p) = self.pending.get_mut(&id) {
                    p.acked = true;
                }
            }
        }
    }

    pub(super) fn handle_state_req(&mut self, msg_id: i64, body: &[u8]) {
        if let Ok(base::MsgsStateReq::MsgsStateReq(r)) = base::MsgsStateReq::from_bytes(body) {
            self.state_info(msg_id, &r.msg_ids);
        }
    }

    pub(super) fn handle_state_info(&mut self, body: &[u8]) {
        let Ok(base::MsgsStateInfo::MsgsStateInfo(i)) = base::MsgsStateInfo::from_bytes(body) else { return };
        let Some(ids) = self.checking.remove(&i.req_msg_id) else { return };
        for (k, id) in ids.into_iter().enumerate() {
            match i.info.get(k) {
                Some(&status) if never_got(status) => self.resend(id),
                Some(_) => {
                    if let Some(p) = self.pending.get_mut(&id) {
                        p.acked = true;
                    }
                }
                // The answer must have a byte per id; for one it left out, asking again is safe, resending is not.
                None => self.recheck.push(id),
            }
        }
    }

    pub(super) fn handle_resend_req(&mut self, msg_id: i64, body: &[u8]) {
        if let Ok(base::MsgResendReq::MsgResendReq(r)) = base::MsgResendReq::from_bytes(body) {
            let mut all = true;
            for &id in &r.msg_ids {
                let sent = self.take_sent(id);
                all &= !sent.is_empty();
                self.requeue(sent);
            }
            // Some we no longer have: the spec says to answer as if it was a msgs_state_req.
            if !all {
                self.state_info(msg_id, &r.msg_ids);
            }
        }
    }

    pub(super) fn handle_all_info(&mut self, body: &[u8]) {
        if let Ok(base::MsgsAllInfo::MsgsAllInfo(i)) = base::MsgsAllInfo::from_bytes(body) {
            for (&id, &status) in i.msg_ids.iter().zip(&i.info) {
                if never_got(status) {
                    self.resend(id);
                }
            }
        }
    }

    // The server has an answer we may not have got: ack it if we did, ask for it again if not.
    pub(super) fn handle_detailed_info(&mut self, body: &[u8]) {
        match base::MsgDetailedInfo::from_bytes(body) {
            Ok(base::MsgDetailedInfo::MsgDetailedInfo(i)) => {
                if self.pending.contains_key(&i.msg_id) {
                    self.request_resend(i.answer_msg_id);
                } else {
                    self.acks.push(i.answer_msg_id);
                }
            }
            Ok(base::MsgDetailedInfo::MsgNewDetailedInfo(i)) => {
                if !self.acks.contains(&i.answer_msg_id) {
                    self.request_resend(i.answer_msg_id);
                }
            }
            Err(_) => {}
        }
    }

    fn request_resend(&mut self, msg_id: i64) {
        self.service.push_back((types::MsgResendReq { msg_ids: vec![msg_id] }.to_bytes(), true));
    }

    // https://core.telegram.org/mtproto/service_messages_about_messages#request-for-message-status-information
    // We don't keep a history of received msg_ids, so anything already acked reads as "unknown" (1).
    fn state_info(&mut self, req_msg_id: i64, msg_ids: &[i64]) {
        let info = msg_ids.iter().map(|id| if self.acks.contains(id) { 4 } else { 1 }).collect();
        self.service.push_back((types::MsgsStateInfo { req_msg_id, info }.to_bytes(), false));
    }
}

// Status 1-3: not received. +32/+64: the query already ran, +128: the server knows it got it; either way,
// re-sending could run it twice.
fn never_got(status: u8) -> bool {
    (1..=3).contains(&(status & 7)) && status & (32 | 64 | 128) == 0
}
