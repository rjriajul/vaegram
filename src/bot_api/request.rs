use serde::Serialize;
use serde::de::DeserializeOwned;

/// A Bot API method: the parameters serialize to its JSON body, and `Response` is what `result` holds.
///
/// Every method in [`raw::methods`](super::raw::methods) implements it; send one with
/// [`BotApi::invoke`](super::BotApi::invoke).
pub trait Request: Serialize {
    /// The method's name in the URL, such as `sendMessage`.
    const NAME: &'static str;
    /// The type of the `result` field of a successful answer.
    type Response: DeserializeOwned;
}
