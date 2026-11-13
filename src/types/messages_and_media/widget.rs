use std::io::Read;

use crate::raw::types;

/// An HTML widget in a message: a self-contained page the app runs offline, with data only the bot can change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Widget {
    /// Id of the shared widget, to send it again with [`Client::send_shared_widget`](crate::Client::send_shared_widget).
    /// `None` unless the widget is shared.
    pub id: Option<i64>,
    /// Width in logical pixels. `None` when it fills the message bubble's width.
    pub width: Option<i32>,
    /// Height in logical pixels. `None` when it isn't fixed.
    pub height: Option<i32>,
    /// The widget's HTML.
    pub html: String,
    /// Data shown wherever the widget is: in the original, in forwards, and in every message of a shared widget.
    pub public_data: Option<String>,
    /// Hash to pass to [`Client::get_widget_public_data`](crate::Client::get_widget_public_data).
    pub public_data_hash: Option<i64>,
    /// Data that belongs to this message and its chat only. Forwards don't carry it.
    pub chat_data: Option<String>,
    /// Whether this message is a forwarded copy of the widget.
    pub is_copy: bool,
    /// Whether a tap in the widget can send a query to the bot.
    pub can_send_queries: bool,
    /// Whether the widget can also send queries to the bot without a tap.
    pub can_send_background_queries: bool,
}

impl Widget {
    pub(crate) fn parse(raw: &types::MessageMediaWidget) -> Option<Self> {
        const LIMIT: u64 = 1024 * 1024;
        let mut bytes = Vec::new();
        flate2::read::ZlibDecoder::new(raw.payload.as_slice()).take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() > LIMIT as usize {
            return None;
        }
        Some(Self {
            id: raw.widget_id,
            width: (raw.width != 0).then_some(raw.width),
            height: (raw.height != 0).then_some(raw.height),
            html: String::from_utf8(bytes).ok()?,
            public_data: raw.public_data.clone(),
            public_data_hash: raw.public_data_hash,
            chat_data: raw.chat_data.clone(),
            is_copy: raw.is_copy,
            can_send_queries: raw.can_query,
            can_send_background_queries: raw.background_query,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Widget;
    use crate::{
        Client,
        dispatcher::Entities,
        raw::{base, types},
        types::Media,
    };
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    fn raw(payload: Vec<u8>) -> types::MessageMediaWidget {
        types::MessageMediaWidget {
            is_copy: false,
            can_query: true,
            background_query: false,
            width: 0,
            height: 42,
            payload,
            public_data: None,
            public_data_hash: None,
            chat_data: None,
            widget_id: None,
        }
    }

    #[test]
    fn inflates_html_and_auto_size() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all("<b>hello</b>".as_bytes()).unwrap();
        let mut source = raw(encoder.finish().unwrap());
        source.public_data_hash = Some(123);
        let widget = Widget::parse(&source).unwrap();
        assert_eq!(widget.html, "<b>hello</b>");
        assert_eq!((widget.width, widget.height), (None, Some(42)));
        assert!(widget.can_send_queries);
        assert_eq!(widget.public_data_hash, Some(123));
    }

    #[test]
    fn broken_payload_is_unknown_media() {
        let media = base::MessageMedia::MessageMediaWidget(Box::new(raw(vec![1, 2, 3])));
        assert!(matches!(Media::parse(&Client::offline(), &media, &Entities::default()), Some(Media::Unknown { .. })));
    }

    #[test]
    fn rejects_oversized_or_non_utf8_html() {
        for input in [vec![b'a'; 1024 * 1024 + 1], vec![0xff]] {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&input).unwrap();
            assert!(Widget::parse(&raw(encoder.finish().unwrap())).is_none());
        }
    }
}
