use crate::Client;
use crate::raw::base;

pub(crate) struct WebViewOptions {
    pub(crate) start_param: Option<String>,
    pub(crate) theme_params: Option<base::DataJSON>,
    pub(crate) platform: String,
    pub(crate) compact: bool,
    pub(crate) fullscreen: bool,
}

impl WebViewOptions {
    pub(crate) fn new(client: &Client) -> Self {
        WebViewOptions {
            start_param: None,
            theme_params: None,
            platform: client.0.client_platform.clone(),
            compact: false,
            fullscreen: false,
        }
    }
}

macro_rules! web_view_builder {
    ($name:ident) => {
        impl $name<'_> {
            /// Value passed to the web app as its start parameter.
            pub fn start_param(mut self, param: impl Into<String>) -> Self {
                self.options.start_param = Some(param.into());
                self
            }

            /// Theme parameters for the web app, as a JSON object string.
            pub fn theme_params(mut self, json: impl Into<String>) -> Self {
                let json = $crate::raw::types::DataJSON { data: json.into() };
                self.options.theme_params = Some($crate::raw::base::DataJSON::DataJSON(Box::new(json)));
                self
            }

            /// Platform name reported to the web app. Defaults to the client's platform.
            pub fn platform(mut self, platform: impl Into<String>) -> Self {
                self.options.platform = platform.into();
                self
            }

            /// Opens the web app in compact mode.
            pub fn compact(mut self) -> Self {
                self.options.compact = true;
                self
            }

            /// Opens the web app in fullscreen mode.
            pub fn fullscreen(mut self) -> Self {
                self.options.fullscreen = true;
                self
            }
        }
    };
}

pub(crate) use web_view_builder;
