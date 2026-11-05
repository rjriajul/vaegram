use crate::raw::{base, types};

/// A chat wallpaper: a picture, a pattern over colors, or a plain color fill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatBackground {
    /// Telegram's id of the wallpaper.
    pub id: i64,
    /// Slug used in wallpaper links. Empty for wallpapers without a file.
    pub slug: String,
    /// `true` if the current account created the wallpaper.
    pub is_creator: bool,
    /// `true` if the wallpaper is one of Telegram's defaults.
    pub is_default: bool,
    /// `true` if the file is a pattern drawn over the background colors.
    pub is_pattern: bool,
    /// `true` if the wallpaper is meant for dark themes.
    pub is_dark: bool,
    /// `true` if the picture is blurred.
    pub is_blurred: bool,
    /// `true` if the picture moves with the device (parallax).
    pub is_moving: bool,
    /// Fill color as RGB, or the first color of a gradient.
    pub background_color: Option<i32>,
    /// Second color of a gradient, as RGB. `None` for a single-color fill.
    pub second_background_color: Option<i32>,
    /// Third color of a gradient, as RGB.
    pub third_background_color: Option<i32>,
    /// Fourth color of a gradient, as RGB.
    pub fourth_background_color: Option<i32>,
    /// How strongly the pattern shows over the colors.
    pub intensity: Option<i32>,
    /// Gradient angle in degrees. Telegram sends and reads it only together with
    /// [`second_background_color`](Self::second_background_color).
    pub rotation_angle: Option<i32>,
    /// Emoji the wallpaper is tied to.
    pub emoji: Option<String>,
    pub(crate) access_hash: Option<i64>,
}

impl ChatBackground {
    pub(crate) fn parse(raw: base::WallPaper) -> ChatBackground {
        let (id, access_hash, slug, is_creator, is_default, is_pattern, is_dark, settings) = match raw {
            base::WallPaper::WallPaper(w) => {
                let w = *w;
                (w.id, Some(w.access_hash), w.slug, w.creator, w.default, w.pattern, w.dark, w.settings)
            }
            base::WallPaper::WallPaperNoFile(w) => {
                let w = *w;
                (w.id, None, String::new(), false, w.default, false, w.dark, w.settings)
            }
        };
        let mut s = settings.map(|base::WallPaperSettings::WallPaperSettings(s)| *s);
        ChatBackground {
            id,
            slug,
            is_creator,
            is_default,
            is_pattern,
            is_dark,
            is_blurred: s.as_ref().is_some_and(|s| s.blur),
            is_moving: s.as_ref().is_some_and(|s| s.motion),
            background_color: s.as_ref().and_then(|s| s.background_color),
            second_background_color: s.as_ref().and_then(|s| s.second_background_color),
            third_background_color: s.as_ref().and_then(|s| s.third_background_color),
            fourth_background_color: s.as_ref().and_then(|s| s.fourth_background_color),
            intensity: s.as_ref().and_then(|s| s.intensity),
            rotation_angle: s.as_ref().and_then(|s| s.rotation),
            emoji: s.as_mut().and_then(|s| s.emoticon.take()),
            access_hash,
        }
    }

    pub(crate) fn write(&self) -> base::InputWallPaper {
        use base::InputWallPaper as I;
        match self.access_hash {
            Some(access_hash) => I::InputWallPaper(Box::new(types::InputWallPaper { id: self.id, access_hash })),
            None if !self.slug.is_empty() => {
                I::InputWallPaperSlug(Box::new(types::InputWallPaperSlug { slug: self.slug.clone() }))
            }
            None => I::InputWallPaperNoFile(types::InputWallPaperNoFile { id: self.id }),
        }
    }

    pub(crate) fn write_settings(&self) -> base::WallPaperSettings {
        // second_background_color and rotation share a flag bit: Telegram reads both or neither.
        let rotation = self.second_background_color.map(|_| self.rotation_angle.unwrap_or(0));
        base::WallPaperSettings::WallPaperSettings(Box::new(types::WallPaperSettings {
            blur: self.is_blurred,
            motion: self.is_moving,
            background_color: self.background_color,
            second_background_color: self.second_background_color,
            third_background_color: self.third_background_color,
            fourth_background_color: self.fourth_background_color,
            intensity: self.intensity,
            rotation,
            emoticon: self.emoji.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::ChatBackground;
    use crate::raw::{base, types};

    #[test]
    fn parses_wallpapers_and_writes_them_back() {
        let document = base::Document::DocumentEmpty(types::DocumentEmpty { id: 9 });
        let s = types::WallPaperSettings {
            blur: true,
            motion: false,
            background_color: Some(0xdbddbb),
            second_background_color: Some(0x6ba587),
            third_background_color: None,
            fourth_background_color: None,
            intensity: Some(50),
            rotation: Some(45),
            emoticon: Some("🏝".into()),
        };
        let raw = base::WallPaper::WallPaper(Box::new(types::WallPaper {
            id: 7,
            creator: false,
            default: true,
            pattern: true,
            dark: false,
            access_hash: 99,
            slug: "fqv01SQemVIBAAAApND8LDRUhRU".into(),
            document,
            settings: Some(base::WallPaperSettings::WallPaperSettings(Box::new(s.clone()))),
        }));
        let bg = ChatBackground::parse(raw);
        let flags = (bg.id, bg.slug.as_str(), bg.is_default, bg.is_pattern);
        assert_eq!(flags, (7, "fqv01SQemVIBAAAApND8LDRUhRU", true, true));
        let settings = (bg.is_blurred, bg.intensity, bg.rotation_angle, bg.emoji.as_deref());
        assert_eq!(settings, (true, Some(50), Some(45), Some("🏝")));
        let input = types::InputWallPaper { id: 7, access_hash: 99 };
        assert_eq!(bg.write(), base::InputWallPaper::InputWallPaper(Box::new(input)));
        assert_eq!(bg.write_settings(), base::WallPaperSettings::WallPaperSettings(Box::new(s)));

        let raw = base::WallPaper::WallPaperNoFile(Box::new(types::WallPaperNoFile {
            id: 3,
            default: false,
            dark: true,
            settings: None,
        }));
        let bg = ChatBackground::parse(raw);
        assert_eq!((bg.id, bg.is_dark, bg.background_color), (3, true, None));
        let input = base::InputWallPaper::InputWallPaperNoFile(types::InputWallPaperNoFile { id: 3 });
        assert_eq!(bg.write(), input);
    }

    #[test]
    fn hand_built_backgrounds_pick_their_input() {
        let fill = ChatBackground { background_color: Some(0xdbddbb), rotation_angle: Some(90), ..Default::default() };
        let input = base::InputWallPaper::InputWallPaperNoFile(types::InputWallPaperNoFile { id: 0 });
        assert_eq!(fill.write(), input);
        let base::WallPaperSettings::WallPaperSettings(s) = fill.write_settings();
        assert_eq!((s.background_color, s.second_background_color, s.rotation), (Some(0xdbddbb), None, None));

        let gradient = ChatBackground { second_background_color: Some(1), ..fill };
        let base::WallPaperSettings::WallPaperSettings(s) = gradient.write_settings();
        assert_eq!((s.second_background_color, s.rotation), (Some(1), Some(90)));
        let unrotated = ChatBackground { rotation_angle: None, ..gradient };
        let base::WallPaperSettings::WallPaperSettings(s) = unrotated.write_settings();
        assert_eq!((s.second_background_color, s.rotation), (Some(1), Some(0)));

        let linked = ChatBackground { slug: "abc".into(), ..Default::default() };
        let slug = types::InputWallPaperSlug { slug: "abc".into() };
        let input = base::InputWallPaper::InputWallPaperSlug(Box::new(slug));
        assert_eq!(linked.write(), input);
    }
}
