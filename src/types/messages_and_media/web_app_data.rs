/// Data a Web App sent back to the bot. Only the bot receives `data`; the user's copy carries the button text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAppData {
    pub button_text: String,
    pub data: Option<String>,
}
