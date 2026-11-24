use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use super::calls::RecordedCall;
use super::{State, TestError, auto_reply};
use crate::errors::{Error, Result, RpcError};
use crate::raw::Serializable;

/// How a [`Script`](super::Script) answers one request.
#[derive(Debug)]
#[non_exhaustive]
pub enum Reply {
    /// The default answer a [`Client::recording`](crate::Client::recording) client gives; an error for a request
    /// without one (see [the list](super#default-answers)).
    OkDefault,
    /// `true` or `false`, for the requests Telegram answers with a `Bool`, such as
    /// `messages.setBotCallbackAnswer`. Any other request fails to read it.
    Bool(bool),
    /// The message, sent or edited, with this id, for `messages.sendMessage`, `messages.sendMedia` and
    /// `messages.editMessage`; its chat and text come from the request.
    SentMessage {
        /// The id the message gets.
        id: i32,
    },
    /// The request fails with this error.
    Error(Error),
    /// These TL bytes, which the request's caller reads as its answer.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    Raw(Vec<u8>),
}

impl From<RpcError> for Reply {
    fn from(error: RpcError) -> Self {
        Reply::Error(Error::Rpc(error))
    }
}

impl From<Error> for Reply {
    fn from(error: Error) -> Self {
        Reply::Error(error)
    }
}

type Responder = Arc<dyn Fn(&RecordedCall) -> Reply + Send + Sync>;

/// A script's answers, by method.
pub(crate) struct Rules {
    pub(crate) auto_reply: bool,
    queued: HashMap<&'static str, VecDeque<Reply>>,
    responders: HashMap<&'static str, Responder>,
    #[cfg(feature = "unstable-raw")]
    raw: VecDeque<Result<Vec<u8>>>,
}

/// What answers the next request.
enum Next {
    Reply(Reply),
    Responder(Responder),
    Unscripted,
    #[cfg(feature = "unstable-raw")]
    Raw(Result<Vec<u8>>),
}

impl Rules {
    pub(crate) fn new(auto_reply: bool) -> Rules {
        Rules {
            auto_reply,
            queued: HashMap::new(),
            responders: HashMap::new(),
            #[cfg(feature = "unstable-raw")]
            raw: VecDeque::new(),
        }
    }

    pub(crate) fn queue(&mut self, method: &'static str, reply: Reply) {
        self.queued.entry(method).or_default().push_back(reply);
    }

    pub(crate) fn respond(&mut self, method: &'static str, responder: Responder) {
        self.responders.insert(method, responder);
    }

    #[cfg(feature = "unstable-raw")]
    pub(crate) fn push_raw(&mut self, reply: Result<Vec<u8>>) {
        self.raw.push_back(reply);
    }

    fn next(&mut self, method: &str) -> Next {
        #[cfg(feature = "unstable-raw")]
        {
            if let Some(raw) = self.raw.pop_front() {
                return Next::Raw(raw);
            }
        }
        if let Some(reply) = self.queued.get_mut(method).and_then(VecDeque::pop_front) {
            return Next::Reply(reply);
        }
        if let Some(responder) = self.responders.get(method) {
            return Next::Responder(responder.clone());
        }
        if self.auto_reply { Next::Reply(Reply::OkDefault) } else { Next::Unscripted }
    }
}

/// The answer to `call`, as the test client's rules give it.
pub(crate) fn answer(state: &State, call: &RecordedCall) -> Result<Vec<u8>> {
    // The lock is let go of before a responder runs, so one can script the next request.
    let next = state.rules().next(call.method);
    let reply = match next {
        Next::Reply(reply) => reply,
        // A panic would end the task that answers every request, so it comes back as this request's error.
        Next::Responder(responder) => match catch_unwind(AssertUnwindSafe(|| responder(call))) {
            Ok(reply) => reply,
            Err(panic) => {
                let message = match (panic.downcast_ref::<&str>(), panic.downcast_ref::<String>()) {
                    (Some(text), _) => (*text).to_owned(),
                    (_, Some(text)) => text.clone(),
                    _ => "a panic without a message".to_owned(),
                };
                return Err(TestError::ResponderPanicked { method: call.method, message }.into());
            }
        },
        Next::Unscripted => return Err(TestError::Unscripted { method: call.method }.into()),
        #[cfg(feature = "unstable-raw")]
        Next::Raw(raw) => return raw,
    };
    match reply {
        Reply::OkDefault => auto_reply::answer(state, call),
        Reply::Bool(value) => Ok(value.to_bytes()),
        Reply::SentMessage { id } => auto_reply::sent(state, call, id),
        Reply::Error(error) => Err(error),
        #[cfg(feature = "unstable-raw")]
        Reply::Raw(bytes) => Ok(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_replies_come_before_the_responder_and_the_default() {
        let mut rules = Rules::new(true);
        rules.queue("a", Reply::Bool(false));
        rules.respond("a", Arc::new(|_: &RecordedCall| Reply::Bool(true)));
        assert!(matches!(rules.next("a"), Next::Reply(Reply::Bool(false))));
        assert!(matches!(rules.next("a"), Next::Responder(_)));
        assert!(matches!(rules.next("b"), Next::Reply(Reply::OkDefault)));
        rules.auto_reply = false;
        assert!(matches!(rules.next("b"), Next::Unscripted));
    }
}
