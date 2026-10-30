use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccentColor {
    pub id: i32,
    pub is_hidden: bool,
    pub colors: Vec<i32>,
    pub dark_colors: Vec<i32>,
    pub channel_min_level: Option<i32>,
    pub group_min_level: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileAccentColor {
    pub id: i32,
    pub is_hidden: bool,
    pub palette_colors: Vec<i32>,
    pub background_colors: Vec<i32>,
    pub story_colors: Vec<i32>,
    pub dark_palette_colors: Vec<i32>,
    pub dark_background_colors: Vec<i32>,
    pub dark_story_colors: Vec<i32>,
    pub channel_min_level: Option<i32>,
    pub group_min_level: Option<i32>,
}

impl AccentColor {
    pub(crate) fn parse(raw: base::help::PeerColorOption) -> AccentColor {
        use base::help::PeerColorSet as S;
        let base::help::PeerColorOption::PeerColorOption(o) = raw;
        let colors = |set: Option<S>| match set {
            Some(S::PeerColorSet(s)) => s.colors,
            _ => Vec::new(),
        };
        AccentColor {
            id: o.color_id,
            is_hidden: o.hidden,
            colors: colors(o.colors),
            dark_colors: colors(o.dark_colors),
            channel_min_level: o.channel_min_level,
            group_min_level: o.group_min_level,
        }
    }
}

impl ProfileAccentColor {
    pub(crate) fn parse(raw: base::help::PeerColorOption) -> ProfileAccentColor {
        use base::help::PeerColorSet as S;
        let base::help::PeerColorOption::PeerColorOption(o) = raw;
        let colors = |set: Option<S>| match set {
            Some(S::PeerColorProfileSet(s)) => (s.palette_colors, s.bg_colors, s.story_colors),
            _ => Default::default(),
        };
        let (palette_colors, background_colors, story_colors) = colors(o.colors);
        let (dark_palette_colors, dark_background_colors, dark_story_colors) = colors(o.dark_colors);
        ProfileAccentColor {
            id: o.color_id,
            is_hidden: o.hidden,
            palette_colors,
            background_colors,
            story_colors,
            dark_palette_colors,
            dark_background_colors,
            dark_story_colors,
            channel_min_level: o.channel_min_level,
            group_min_level: o.group_min_level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccentColor, ProfileAccentColor};
    use crate::raw::{base, types};

    use base::help::PeerColorSet as S;

    fn option(colors: S, dark_colors: Option<S>) -> base::help::PeerColorOption {
        let o = types::help::PeerColorOption {
            hidden: false,
            color_id: 7,
            colors: Some(colors),
            dark_colors,
            channel_min_level: Some(2),
            group_min_level: None,
        };
        base::help::PeerColorOption::PeerColorOption(Box::new(o))
    }

    #[test]
    fn an_option_without_a_palette_parses_empty() {
        let raw = types::help::PeerColorOption {
            hidden: true,
            color_id: 3,
            colors: None,
            dark_colors: None,
            channel_min_level: None,
            group_min_level: None,
        };
        let raw = base::help::PeerColorOption::PeerColorOption(Box::new(raw));
        let color = AccentColor::parse(raw.clone());
        assert_eq!((color.is_hidden, color.colors, color.dark_colors), (true, Vec::new(), Vec::new()));
        let profile = ProfileAccentColor::parse(raw);
        assert_eq!((profile.palette_colors, profile.dark_story_colors), (Vec::new(), Vec::new()));
    }

    #[test]
    fn parses_both_palette_kinds() {
        let set = |colors: Vec<i32>| S::PeerColorSet(Box::new(types::help::PeerColorSet { colors }));
        let color = AccentColor::parse(option(set(vec![1, 2]), Some(set(vec![3]))));
        assert_eq!((color.id, color.colors, color.dark_colors), (7, vec![1, 2], vec![3]));
        assert_eq!((color.channel_min_level, color.group_min_level), (Some(2), None));

        let (palette_colors, bg_colors, story_colors) = (vec![1], vec![2, 3], vec![4]);
        let set = types::help::PeerColorProfileSet { palette_colors, bg_colors, story_colors };
        let p = ProfileAccentColor::parse(option(S::PeerColorProfileSet(Box::new(set)), None));
        assert_eq!((p.palette_colors, p.background_colors, p.story_colors), (vec![1], vec![2, 3], vec![4]));
        assert!(p.dark_palette_colors.is_empty());
    }
}
