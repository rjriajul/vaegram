use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuButton {
    /// Falls back to the button set for all users; has no effect when setting that one.
    Default,
    Commands,
    WebApp {
        text: String,
        url: String,
    },
}

impl MenuButton {
    pub(crate) fn write(self) -> base::BotMenuButton {
        match self {
            MenuButton::Default => base::BotMenuButton::BotMenuButtonDefault,
            MenuButton::Commands => base::BotMenuButton::BotMenuButtonCommands,
            MenuButton::WebApp { text, url } => {
                base::BotMenuButton::BotMenuButton(Box::new(types::BotMenuButton { text, url }))
            }
        }
    }

    pub(crate) fn parse(button: base::BotMenuButton) -> MenuButton {
        match button {
            base::BotMenuButton::BotMenuButtonDefault => MenuButton::Default,
            base::BotMenuButton::BotMenuButtonCommands => MenuButton::Commands,
            base::BotMenuButton::BotMenuButton(b) => MenuButton::WebApp { text: b.text, url: b.url },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MenuButton;

    #[test]
    fn round_trips_through_raw() {
        for button in [
            MenuButton::Default,
            MenuButton::Commands,
            MenuButton::WebApp { text: "Open".into(), url: "https://example.com".into() },
        ] {
            assert_eq!(MenuButton::parse(button.clone().write()), button);
        }
    }
}
