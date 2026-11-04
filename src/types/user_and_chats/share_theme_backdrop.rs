use crate::raw::base;

/// A backdrop a chat's share theme can use; see [`Client::get_share_theme_backdrops`](crate::Client::get_share_theme_backdrops).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareThemeBackdrop {
    pub id: i32,
    pub name: String,
    pub center_color: i32,
    pub edge_color: i32,
    pub pattern_color: i32,
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
