use std::sync::Arc;

use crate::raw::types;
use crate::types::Json;

/// Telegram's app configuration: limits and feature switches by name, such as `reactions_user_max`. The names and
/// values are Telegram's and change without notice, so they stay JSON.
///
/// [`Client::get_app_config`](crate::Client::get_app_config) keeps the last one it got, and only asks Telegram
/// whether it changed since. Clones share the JSON.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AppConfig {
    json: Arc<Json>,
    pub(crate) hash: i32,
}

impl AppConfig {
    pub(crate) fn parse(raw: types::help::AppConfig) -> AppConfig {
        AppConfig { json: Arc::new(Json::parse(raw.config)), hash: raw.hash }
    }

    /// An empty configuration, for a reply that has none.
    pub(crate) fn empty() -> AppConfig {
        AppConfig { json: Arc::new(Json::Object(Vec::new())), hash: 0 }
    }

    /// Returns the value named `key`. `None` if the configuration has no such field.
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.json.get(key)
    }

    /// Returns the whole configuration, a JSON object.
    pub fn json(&self) -> &Json {
        &self.json
    }
}
