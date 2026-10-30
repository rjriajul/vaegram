use crate::errors::{Error, Result};
use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq)]
pub struct BotApp {
    pub id: i64,
    pub short_name: String,
    pub title: String,
    pub description: String,
    pub inactive: bool,
    pub request_write_access: bool,
    pub has_settings: bool,
}

impl BotApp {
    pub(crate) fn parse(raw: base::messages::BotApp) -> Result<BotApp> {
        let base::messages::BotApp::BotApp(r) = raw;
        let types::messages::BotApp { inactive, request_write_access, has_settings, app } = *r;
        let base::BotApp::BotApp(app) = app else {
            return Err(Error::Unexpected("messages.getBotApp returned botAppNotModified"));
        };
        Ok(BotApp {
            id: app.id,
            short_name: app.short_name.clone(),
            title: app.title.clone(),
            description: app.description.clone(),
            inactive,
            request_write_access,
            has_settings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BotApp;
    use crate::errors::Error;
    use crate::raw::{base, types};

    #[test]
    fn takes_flags_from_the_wrapper() {
        let photo = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 });
        let app = types::BotApp {
            id: 7,
            access_hash: 8,
            short_name: "game".into(),
            title: "Game".into(),
            description: "A game".into(),
            photo,
            document: None,
            hash: 9,
        };
        let raw = |app| {
            let r = types::messages::BotApp { inactive: true, request_write_access: false, has_settings: true, app };
            base::messages::BotApp::BotApp(Box::new(r))
        };
        let parsed = BotApp::parse(raw(base::BotApp::BotApp(Box::new(app)))).unwrap();
        assert_eq!((parsed.id, parsed.short_name.as_str(), parsed.title.as_str()), (7, "game", "Game"));
        assert_eq!((parsed.inactive, parsed.request_write_access, parsed.has_settings), (true, false, true));
        assert!(matches!(BotApp::parse(raw(base::BotApp::BotAppNotModified)), Err(Error::Unexpected(_))));
    }
}
