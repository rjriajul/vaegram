use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::{ClickResult, Message, Text};

/// Which keyboard button to press: `0` (counting row by row), `(row, column)` or its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRef<'a> {
    Index(usize),
    Position(usize, usize),
    Text(&'a str),
}

impl From<usize> for ButtonRef<'_> {
    fn from(i: usize) -> Self {
        ButtonRef::Index(i)
    }
}

impl From<(usize, usize)> for ButtonRef<'_> {
    fn from((row, column): (usize, usize)) -> Self {
        ButtonRef::Position(row, column)
    }
}

impl<'a> From<&'a str> for ButtonRef<'a> {
    fn from(text: &'a str) -> Self {
        ButtonRef::Text(text)
    }
}

#[must_use = "a request does nothing until it is awaited"]
pub struct Click<'a> {
    message: &'a Message,
    button: ButtonRef<'a>,
    password: Option<String>,
}

impl Message {
    pub fn click<'a>(&'a self, button: impl Into<ButtonRef<'a>>) -> Click<'a> {
        Click { message: self, button: button.into(), password: None }
    }
}

impl Click<'_> {
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    async fn send(self) -> Result<ClickResult> {
        use base::{ButtonType as B, InlineButtonType as T};
        let (message, client) = (self.message, self.message.client());
        let not_found = Error::InvalidArgument("this message has no such button");
        let button = match &message.raw.reply_markup {
            Some(base::ReplyMarkup::ReplyInlineMarkup(k)) => {
                let rows =
                    k.rows.iter().map(|base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(r)| &r.buttons[..]);
                let base::KeyboardInlineButton::KeyboardInlineButton(b) =
                    pick(rows, self.button, |base::KeyboardInlineButton::KeyboardInlineButton(b)| &b.text)
                        .ok_or(not_found)?;
                &b.r#type
            }
            Some(base::ReplyMarkup::ReplyKeyboardMarkup(k)) => {
                let rows = k.rows.iter().map(|base::KeyboardButtonRow::KeyboardButtonRow(r)| &r.buttons[..]);
                let base::KeyboardButton::KeyboardButton(b) =
                    pick(rows, self.button, |base::KeyboardButton::KeyboardButton(b)| &b.text).ok_or(not_found)?;
                let refuse = |what| Err(Error::InvalidArgument(what));
                match &b.r#type {
                    B::ButtonTypeDefault => {}
                    B::ButtonTypeSimpleWebView(w) => return Ok(ClickResult::Url(w.url.clone())),
                    B::ButtonTypeRequestPhone => return refuse("this button shares a phone number: use send_contact"),
                    B::ButtonTypeRequestGeoLocation => {
                        return refuse("this button shares a location: use send_location");
                    }
                    B::ButtonTypeRequestPoll(_) => return refuse("this button asks for a poll: use send_poll"),
                    _ => return refuse("this button can't be pressed from a client"),
                }
                // The label is the bot's text, not markup of ours, so no parse mode may run over it.
                let text = Text::new().plain(b.text.as_str());
                return Ok(ClickResult::Sent(Box::new(client.send_message(message.chat.id, text).await?)));
            }
            _ => return Err(not_found),
        };
        let callback = match button {
            T::InlineButtonTypeCallback(c) => client.request_callback_answer(message.chat.id, message.id, &c.data[..]),
            T::InlineButtonTypeGame => client.request_callback_answer(message.chat.id, message.id, []).game(),
            T::InlineButtonTypeUrl(u) => return Ok(ClickResult::Url(u.url.clone())),
            T::InlineButtonTypeUrlAuth(u) => return Ok(ClickResult::Url(u.url.clone())),
            T::InlineButtonTypeWebView(u) => return Ok(ClickResult::Url(u.url.clone())),
            T::InlineButtonTypeSwitchInline(s) => {
                return Ok(ClickResult::SwitchInline { query: s.query.clone(), same_chat: s.same_peer });
            }
            T::InlineButtonTypeCopy(c) => return Ok(ClickResult::Copy(c.copy_text.clone())),
            T::InlineButtonTypeUserProfile(u) => return Ok(ClickResult::User(u.user_id)),
            _ => return Err(Error::InvalidArgument("this button can't be pressed from a client")),
        };
        let callback = match self.password {
            Some(password) => callback.password(password),
            None => callback,
        };
        Ok(ClickResult::Answer(callback.await?))
    }
}

impl<'a> IntoFuture for Click<'a> {
    type Output = Result<ClickResult>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

fn pick<'b, B>(
    mut rows: impl Iterator<Item = &'b [B]>,
    at: ButtonRef<'_>,
    text: impl Fn(&B) -> &String,
) -> Option<&'b B> {
    match at {
        ButtonRef::Index(i) => rows.flat_map(|r| r.iter()).nth(i),
        ButtonRef::Position(row, column) => rows.nth(row)?.get(column),
        ButtonRef::Text(t) => rows.flat_map(|r| r.iter()).find(|&b| text(b) == t),
    }
}

#[cfg(test)]
mod tests {
    use super::ButtonRef;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::ParseMode;
    use crate::errors::Error;
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::{
        ClickResult, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, Message, ReplyKeyboardMarkup,
    };
    use crate::utils::get_peer;

    const BOT: i64 = 42;

    fn message(client: &Client, markup: base::ReplyMarkup) -> Message {
        let peer = types::InputPeerUser { user_id: BOT, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(BOT, base::InputPeer::InputPeerUser(Box::new(peer)));
        let mut raw = Message::bare(5, get_peer(BOT), 0);
        raw.reply_markup = Some(markup);
        Message::parse(client, Box::new(raw), &Entities::default())
    }

    #[tokio::test]
    async fn inline_buttons_by_index_position_and_text() {
        let (client, mut rx) = Client::recording();
        let markup = InlineKeyboardMarkup::new([
            vec![InlineKeyboardButton::url("Site", "https://x.y"), InlineKeyboardButton::callback("Go", "data")],
            vec![InlineKeyboardButton::copy_text("Copy", "secret")],
        ]);
        let message = message(&client, markup.write());

        assert!(matches!(message.click(0).await, Ok(ClickResult::Url(u)) if u == "https://x.y"));
        assert!(matches!(message.click("Copy").await, Ok(ClickResult::Copy(c)) if c == "secret"));
        assert!(message.click(ButtonRef::Position(1, 1)).await.is_err());
        assert!(rx.try_recv().is_err());

        let _ = message.click((0, 1)).await;
        let expected = functions::messages::GetBotCallbackAnswer {
            game: false,
            peer: client.resolve_peer(BOT).await.unwrap(),
            msg_id: 5,
            data: Some(b"data".to_vec()),
            password: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn reply_buttons_send_their_text() {
        let (client, mut rx) = Client::recording();
        let markup = ReplyKeyboardMarkup::new([vec![KeyboardButton::new("Yes"), KeyboardButton::new("No")]]);
        let message = message(&client, markup.write());
        let _ = message.click("No").await;
        let body = rx.try_recv().unwrap();
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), functions::messages::SendMessage::CONSTRUCTOR_ID);
        assert!(body.windows(3).any(|w| w == b"\x02No"));
    }

    #[tokio::test]
    async fn reply_button_text_skips_the_parse_mode() {
        let (mut client, mut rx) = Client::recording();
        std::sync::Arc::get_mut(&mut client.0).unwrap().parse_mode = ParseMode::Html;
        let label = r#"<a href="https://x.example">Yes</a>"#;
        let message = message(&client, ReplyKeyboardMarkup::new([vec![KeyboardButton::new(label)]]).write());
        let _ = message.click(0).await;
        let body = rx.try_recv().unwrap();
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        let literal = [&[label.len() as u8], label.as_bytes()].concat();
        assert_eq!((body.windows(literal.len()).any(|w| w == literal), flags & 1 << 3), (true, 0));
    }

    #[tokio::test]
    async fn reply_buttons_that_share_data_are_not_sent_as_text() {
        let (client, mut rx) = Client::recording();
        let mut markup = ReplyKeyboardMarkup::new([vec![
            KeyboardButton::new("Phone").request_contact(),
            KeyboardButton::new("Here").request_location(),
            KeyboardButton::new("App"),
        ]])
        .write();
        if let base::ReplyMarkup::ReplyKeyboardMarkup(k) = &mut markup {
            let base::KeyboardButtonRow::KeyboardButtonRow(row) = &mut k.rows[0];
            let base::KeyboardButton::KeyboardButton(app) = &mut row.buttons[2];
            let web_view = types::ButtonTypeSimpleWebView { url: "https://app.example".into() };
            app.r#type = base::ButtonType::ButtonTypeSimpleWebView(Box::new(web_view));
        }
        let message = message(&client, markup);

        assert!(matches!(message.click("Phone").await, Err(Error::InvalidArgument(e)) if e.contains("send_contact")));
        assert!(matches!(message.click("Here").await, Err(Error::InvalidArgument(e)) if e.contains("send_location")));
        assert!(matches!(message.click("App").await, Ok(ClickResult::Url(u)) if u == "https://app.example"));
        assert!(rx.try_recv().is_err());
    }
}
