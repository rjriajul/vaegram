use crate::raw::base;

/// The colors a regular gift is drawn on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GiftBackground {
    pub center_color: i32,
    pub edge_color: i32,
    pub text_color: i32,
}

impl GiftBackground {
    pub(crate) fn parse(raw: &base::StarGiftBackground) -> GiftBackground {
        let base::StarGiftBackground::StarGiftBackground(b) = raw;
        GiftBackground { center_color: b.center_color, edge_color: b.edge_color, text_color: b.text_color }
    }
}
