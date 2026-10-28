use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq)]
pub enum InputRichMessage {
    Html { html: String, rtl: bool, no_autolink: bool },
    Markdown { markdown: String, rtl: bool, no_autolink: bool },
}

impl InputRichMessage {
    pub fn html(html: impl Into<String>) -> Self {
        InputRichMessage::Html { html: html.into(), rtl: false, no_autolink: false }
    }

    pub fn markdown(markdown: impl Into<String>) -> Self {
        InputRichMessage::Markdown { markdown: markdown.into(), rtl: false, no_autolink: false }
    }

    pub fn rtl(mut self) -> Self {
        match &mut self {
            InputRichMessage::Html { rtl, .. } | InputRichMessage::Markdown { rtl, .. } => *rtl = true,
        }
        self
    }

    pub fn no_autolink(mut self) -> Self {
        match &mut self {
            InputRichMessage::Html { no_autolink, .. } | InputRichMessage::Markdown { no_autolink, .. } => {
                *no_autolink = true
            }
        }
        self
    }

    pub(crate) fn write(self) -> base::InputRichMessage {
        match self {
            InputRichMessage::Html { html, rtl, no_autolink } => {
                let html = types::InputRichMessageHTML { rtl, noautolink: no_autolink, html, files: None };
                base::InputRichMessage::InputRichMessageHTML(Box::new(html))
            }
            InputRichMessage::Markdown { markdown, rtl, no_autolink } => {
                let markdown = types::InputRichMessageMarkdown { rtl, noautolink: no_autolink, markdown, files: None };
                base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown))
            }
        }
    }
}
