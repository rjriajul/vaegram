use crate::enums::BaseTheme;
use crate::raw::{base, types};
use crate::types::ChatBackground;

// Only themes made from a file depend on the format; settings-based cloud themes look the same everywhere.
pub(crate) const THEME_FORMAT: &str = "android";

/// A cloud theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Telegram's id of the theme.
    pub id: i64,
    /// Short name used in the theme's `t.me/addtheme/<slug>` link.
    pub slug: String,
    /// Display title of the theme.
    pub title: String,
    /// Emoji the theme is tied to. `None` if it has none.
    pub emoji: Option<String>,
    /// `true` if this account created the theme.
    pub is_creator: bool,
    /// `true` if the theme is a default one.
    pub is_default: bool,
    /// `true` if the theme is meant for a single chat.
    pub is_for_chat: bool,
    /// Number of installs. 0 if Telegram sends no count.
    pub installs_count: i32,
    /// One entry per base theme the theme supports.
    pub settings: Vec<ThemeSettings>,
    pub(crate) access_hash: i64,
}

/// Colors and wallpaper of a theme on top of a base theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSettings {
    /// The built-in theme these settings modify.
    pub base_theme: BaseTheme,
    /// Accent color of the interface.
    pub accent_color: i32,
    /// Accent color of outgoing messages. `None` uses `accent_color`.
    pub outbox_accent_color: Option<i32>,
    /// Colors of the message bubbles. Several colors make a gradient.
    pub message_colors: Vec<i32>,
    /// `true` if the gradient in `message_colors` is animated.
    pub message_colors_animated: bool,
    /// Chat wallpaper. `None` if the theme sets none.
    pub wallpaper: Option<ChatBackground>,
}

impl Theme {
    pub(crate) fn parse(raw: base::Theme) -> Theme {
        let base::Theme::Theme(raw) = raw;
        let raw = *raw;
        Theme {
            id: raw.id,
            slug: raw.slug,
            title: raw.title,
            emoji: raw.emoticon,
            is_creator: raw.creator,
            is_default: raw.default,
            is_for_chat: raw.for_chat,
            installs_count: raw.installs_count.unwrap_or(0),
            settings: raw.settings.into_iter().flatten().map(ThemeSettings::parse).collect(),
            access_hash: raw.access_hash,
        }
    }

    pub(crate) fn write(&self) -> base::InputTheme {
        base::InputTheme::InputTheme(Box::new(types::InputTheme { id: self.id, access_hash: self.access_hash }))
    }
}

impl ThemeSettings {
    /// Creates settings over `base_theme` with the given accent color and no other options.
    pub fn new(base_theme: BaseTheme, accent_color: i32) -> Self {
        ThemeSettings {
            base_theme,
            accent_color,
            outbox_accent_color: None,
            message_colors: Vec::new(),
            message_colors_animated: false,
            wallpaper: None,
        }
    }

    /// Gives outgoing messages their own accent color instead of `accent_color`.
    pub fn outbox_accent_color(mut self, color: i32) -> Self {
        self.outbox_accent_color = Some(color);
        self
    }

    /// Colors the message bubbles. More than one color makes a gradient.
    pub fn message_colors(mut self, colors: impl IntoIterator<Item = i32>) -> Self {
        self.message_colors = colors.into_iter().collect();
        self
    }

    /// Sets whether the message bubble gradient is animated; `true` animates it.
    pub fn message_colors_animated(mut self, message_colors_animated: bool) -> Self {
        self.message_colors_animated = message_colors_animated;
        self
    }

    /// Adds a chat wallpaper to the theme.
    pub fn wallpaper(mut self, wallpaper: ChatBackground) -> Self {
        self.wallpaper = Some(wallpaper);
        self
    }

    pub(crate) fn parse(raw: base::ThemeSettings) -> ThemeSettings {
        let base::ThemeSettings::ThemeSettings(s) = raw;
        ThemeSettings {
            base_theme: BaseTheme::parse(&s.base_theme),
            accent_color: s.accent_color,
            outbox_accent_color: s.outbox_accent_color,
            message_colors: s.message_colors.unwrap_or_default(),
            message_colors_animated: s.message_colors_animated,
            wallpaper: s.wallpaper.map(ChatBackground::parse),
        }
    }

    pub(crate) fn write(&self) -> base::InputThemeSettings {
        let wallpaper = self.wallpaper.as_ref();
        base::InputThemeSettings::InputThemeSettings(Box::new(types::InputThemeSettings {
            message_colors_animated: self.message_colors_animated,
            base_theme: self.base_theme.write(),
            accent_color: self.accent_color,
            outbox_accent_color: self.outbox_accent_color,
            message_colors: (!self.message_colors.is_empty()).then(|| self.message_colors.clone()),
            wallpaper: wallpaper.map(ChatBackground::write),
            wallpaper_settings: wallpaper.map(ChatBackground::write_settings),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, ThemeSettings};
    use crate::enums::BaseTheme;
    use crate::raw::{base, types};
    use crate::types::ChatBackground;

    #[test]
    fn parses_themes() {
        let settings = types::ThemeSettings {
            message_colors_animated: true,
            base_theme: base::BaseTheme::BaseThemeNight,
            accent_color: 0x3e88f7,
            outbox_accent_color: None,
            message_colors: Some(vec![1, 2]),
            wallpaper: None,
        };
        let raw = base::Theme::Theme(Box::new(types::Theme {
            creator: true,
            default: false,
            for_chat: true,
            id: 5,
            access_hash: 6,
            slug: "night".into(),
            title: "Night".into(),
            document: None,
            settings: Some(vec![base::ThemeSettings::ThemeSettings(Box::new(settings))]),
            emoticon: Some("🌚".into()),
            installs_count: None,
        }));
        let theme = Theme::parse(raw);
        assert_eq!((theme.id, theme.title.as_str(), theme.emoji.as_deref()), (5, "Night", Some("🌚")));
        assert_eq!((theme.is_creator, theme.is_for_chat, theme.installs_count), (true, true, 0));
        let night = ThemeSettings::new(BaseTheme::Night, 0x3e88f7).message_colors([1, 2]).message_colors_animated(true);
        assert_eq!(theme.settings, [night]);
        let input = types::InputTheme { id: 5, access_hash: 6 };
        assert_eq!(theme.write(), base::InputTheme::InputTheme(Box::new(input)));
    }

    #[test]
    fn a_theme_without_settings_or_installs_parses_empty() {
        let raw = types::Theme {
            creator: false,
            default: true,
            for_chat: false,
            id: 1,
            access_hash: 2,
            slug: "s".into(),
            title: "T".into(),
            document: None,
            settings: None,
            emoticon: None,
            installs_count: None,
        };
        let theme = Theme::parse(base::Theme::Theme(Box::new(raw)));
        assert_eq!((theme.settings.len(), theme.emoji, theme.installs_count, theme.is_default), (0, None, 0, true));
    }

    #[test]
    fn message_colors_animated_setter_round_trips_the_field() {
        let animated = ThemeSettings::new(BaseTheme::Day, 1).message_colors_animated(true);
        assert!(animated.message_colors_animated);
        assert_eq!(animated.message_colors_animated(false), ThemeSettings::new(BaseTheme::Day, 1));
    }

    #[test]
    fn theme_settings_send_wallpaper_and_its_settings_together() {
        let plain = ThemeSettings::new(BaseTheme::Day, 1);
        let base::InputThemeSettings::InputThemeSettings(s) = plain.write();
        assert_eq!((s.message_colors, s.wallpaper, s.wallpaper_settings), (None, None, None));

        let fill = ChatBackground { background_color: Some(0xffffff), ..Default::default() };
        let base::InputThemeSettings::InputThemeSettings(s) = plain.wallpaper(fill.clone()).write();
        assert_eq!((s.wallpaper, s.wallpaper_settings), (Some(fill.write()), Some(fill.write_settings())));
    }
}
