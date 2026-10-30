use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::StoriesStealthMode;

#[must_use = "a request does nothing until it is awaited"]
pub struct EnableStealthMode<'a> {
    client: &'a Client,
    past: bool,
    future: bool,
}

impl Client {
    /// Premium only; the hidden periods come from the server's `stories_stealth_*_period` config.
    pub fn enable_stealth_mode(&self) -> EnableStealthMode<'_> {
        EnableStealthMode { client: self, past: false, future: false }
    }
}

impl EnableStealthMode<'_> {
    pub fn past(mut self) -> Self {
        self.past = true;
        self
    }

    pub fn future(mut self) -> Self {
        self.future = true;
        self
    }

    async fn send(self) -> Result<StoriesStealthMode> {
        let request = functions::stories::ActivateStealthMode { past: self.past, future: self.future };
        stealth_mode(self.client.invoke(&request).await?)
    }
}

into_future!(EnableStealthMode, StoriesStealthMode);

fn stealth_mode(updates: base::Updates) -> Result<StoriesStealthMode> {
    let updates = match updates {
        base::Updates::Updates(u) => u.updates,
        base::Updates::UpdatesCombined(u) => u.updates,
        base::Updates::UpdateShort(u) => vec![u.update],
        _ => Vec::new(),
    };
    updates
        .into_iter()
        .find_map(|u| match u {
            base::Update::UpdateStoriesStealthMode(u) => Some(StoriesStealthMode::parse(u.stealth_mode)),
            _ => None,
        })
        .ok_or(Error::Unexpected("Telegram sent no stealth mode back"))
}

#[cfg(test)]
mod tests {
    use super::stealth_mode;
    use crate::raw::{base, types};
    use crate::types::StoriesStealthMode;

    #[test]
    fn reads_stealth_mode_from_updates() {
        let mode = types::StoriesStealthMode { active_until_date: Some(100), cooldown_until_date: Some(200) };
        let update = types::UpdateStoriesStealthMode {
            stealth_mode: base::StoriesStealthMode::StoriesStealthMode(Box::new(mode)),
        };
        let updates = types::UpdateShort { update: base::Update::UpdateStoriesStealthMode(Box::new(update)), date: 0 };
        let mode = stealth_mode(base::Updates::UpdateShort(Box::new(updates))).unwrap();
        assert_eq!(mode, StoriesStealthMode { active_until_date: Some(100), cooldown_until_date: Some(200) });
        assert!(stealth_mode(base::Updates::UpdatesTooLong).is_err());
    }
}
