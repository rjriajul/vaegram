use crate::raw::base;

/// A backdrop a chat's share theme can use.
///
/// Fetched with [`Client::get_share_theme_backdrops`](crate::Client::get_share_theme_backdrops).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareThemeBackdrop {
    /// Id of the backdrop.
    pub id: i32,
    /// Name of the backdrop.
    pub name: String,
    /// Color at the center of the gradient.
    pub center_color: i32,
    /// Color at the edge of the gradient.
    pub edge_color: i32,
    /// Color of the pattern drawn over the gradient.
    pub pattern_color: i32,
    /// Color of the text drawn on the backdrop.
    pub text_color: i32,
}

impl ShareThemeBackdrop {
    pub(crate) fn parse(raw: &base::ShareThemeBackdropOption) -> ShareThemeBackdrop {
        let base::ShareThemeBackdropOption::ShareThemeBackdropOption(o) = raw;
        ShareThemeBackdrop {
            id: o.backdrop_id,
            name: o.name.clone(),
            center_color: o.center_color,
            edge_color: o.edge_color,
            pattern_color: o.pattern_color,
            text_color: o.text_color,
        }
    }
}
