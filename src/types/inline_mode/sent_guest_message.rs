use crate::raw::{Serializable, base};
use crate::storage::session_string::encode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentGuestMessage {
    pub inline_message_id: String,
}

impl SentGuestMessage {
    pub(crate) fn parse(id: &base::InputBotInlineMessageID) -> SentGuestMessage {
        // The bare TL fields, base64url without padding.
        let mut buf = Vec::new();
        id.serialize(&mut buf);
        SentGuestMessage { inline_message_id: encode(&buf[4..]) }
    }
}

#[cfg(test)]
mod tests {
    use super::SentGuestMessage;
    use crate::raw::{base, types};

    #[test]
    fn packs_inline_message_ids() {
        let id = types::InputBotInlineMessageID { dc_id: 2, id: 123456789, access_hash: -987654321 };
        let id = base::InputBotInlineMessageID::InputBotInlineMessageID(Box::new(id));
        assert_eq!(SentGuestMessage::parse(&id).inline_message_id, "AgAAABXNWwcAAAAAT5chxf____8");

        let id = types::InputBotInlineMessageID64 { dc_id: 4, owner_id: -1001234567890, id: 42, access_hash: 7 };
        let id = base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id));
        assert_eq!(SentGuestMessage::parse(&id).inline_message_id, "BAAAAC7txOEW____KgAAAAcAAAAAAAAA");
    }
}
