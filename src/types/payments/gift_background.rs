use crate::raw::base;

/// The colors a regular gift is drawn on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GiftBackground {
    /// Color at the center, as an RGB integer.
    pub center_color: i32,
    /// Color at the edges, as an RGB integer.
    pub edge_color: i32,
    /// Color of text drawn over the background, as an RGB integer.
    pub text_color: i32,
}

impl GiftBackground {
    pub(crate) fn parse(raw: &base::StarGiftBackground) -> GiftBackground {
        let base::StarGiftBackground::StarGiftBackground(b) = raw;
        GiftBackground { center_color: b.center_color, edge_color: b.edge_color, text_color: b.text_color }
    }
}
