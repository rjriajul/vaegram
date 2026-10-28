use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Message;

const HISTORY_LIMIT: usize = 100;

pub(super) struct Pages {
    buffer: std::vec::IntoIter<base::Message>,
    entities: Entities,
    pub(super) remaining: usize,
    pub(super) offset_id: i32,
    done: bool,
}

impl Pages {
    pub(super) fn new() -> Pages {
        Pages {
            buffer: Vec::new().into_iter(),
            entities: Entities::default(),
            remaining: usize::MAX,
            offset_id: 0,
            done: false,
        }
    }

    pub(super) fn limit(&self) -> Option<i32> {
        (!self.done && self.remaining > 0).then(|| self.remaining.min(HISTORY_LIMIT) as i32)
    }

    pub(super) fn fill(&mut self, client: &Client, page: base::messages::Messages) {
        let (messages, entities) = Message::unpack(client, page);
        match messages.last().map(message_id) {
            // An offset that doesn't move down means Telegram has nothing older to give.
            Some(id) if self.offset_id == 0 || id < self.offset_id => self.offset_id = id,
            _ => {
                self.done = true;
                return;
            }
        }
        self.buffer = messages.into_iter();
        self.entities = entities;
    }

    pub(super) fn pop(&mut self, client: &Client) -> Option<Message> {
        while self.remaining > 0 {
            if let Some(message) = Message::from_raw(client, self.buffer.next()?, &self.entities) {
                self.remaining -= 1;
                return Some(message);
            }
        }
        None
    }
}

fn message_id(message: &base::Message) -> i32 {
    match message {
        base::Message::MessageEmpty(m) => m.id,
        base::Message::Message(m) => m.id,
        base::Message::MessageService(m) => m.id,
    }
}
