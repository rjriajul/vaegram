use std::fmt;
use std::str::FromStr;

use crate::errors::{Error, Result};
use crate::raw::{Serializable, base, types};
use crate::utils::{base64url_decode, base64url_encode};

/// Prints and parses as the Bot API `inline_message_id` string.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineMessageId(pub(crate) base::InputBotInlineMessageID);

impl InlineMessageId {
    pub fn dc_id(&self) -> i32 {
        match &self.0 {
            base::InputBotInlineMessageID::InputBotInlineMessageID(m) => m.dc_id,
            base::InputBotInlineMessageID::InputBotInlineMessageID64(m) => m.dc_id,
        }
    }
}

impl fmt::Display for InlineMessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The Bot API string is the bare TL body, so the constructor id is dropped and the length tells them apart.
        f.write_str(&base64url_encode(&self.0.to_bytes()[4..]))
    }
}

impl FromStr for InlineMessageId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let bytes = base64url_decode(s).ok_or(Error::InvalidArgument("inline message id isn't base64url"))?;
        let buf = &mut &bytes[..];
        let damaged = |_| Error::InvalidArgument("inline message id is damaged");
        let id = match bytes.len() {
            20 => base::InputBotInlineMessageID::InputBotInlineMessageID(Box::new(
                types::InputBotInlineMessageID::deserialize_bare(buf).map_err(damaged)?,
            )),
            24 => base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(
                types::InputBotInlineMessageID64::deserialize_bare(buf).map_err(damaged)?,
            )),
            _ => return Err(Error::InvalidArgument("inline message id has the wrong length")),
        };
        Ok(InlineMessageId(id))
    }
}

#[cfg(test)]
mod tests {
    use super::InlineMessageId;
    use crate::raw::{base, types};

    #[test]
    fn matches_bot_api_strings() {
        let id64 = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 123456789, id: 42, access_hash: -987654321 };
        let id64 = InlineMessageId(base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id64)));
        assert_eq!(id64.to_string(), "AgAAABXNWwcAAAAAKgAAAE-XIcX_____");
        assert_eq!("AgAAABXNWwcAAAAAKgAAAE-XIcX_____".parse::<InlineMessageId>().unwrap(), id64);

        let id = types::InputBotInlineMessageID { dc_id: 4, id: -5, access_hash: 7 };
        let id = InlineMessageId(base::InputBotInlineMessageID::InputBotInlineMessageID(Box::new(id)));
        assert_eq!(id.to_string(), "BAAAAPv_________BwAAAAAAAAA");
        assert_eq!("BAAAAPv_________BwAAAAAAAAA".parse::<InlineMessageId>().unwrap(), id);
        assert_eq!(id.dc_id(), 4);

        assert!("BAAA".parse::<InlineMessageId>().is_err());
        assert!("not base64!".parse::<InlineMessageId>().is_err());
    }
}
