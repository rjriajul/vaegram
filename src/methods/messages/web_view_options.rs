use crate::raw::base;

pub(crate) struct WebViewOptions {
    pub(crate) start_param: Option<String>,
    pub(crate) theme_params: Option<base::DataJSON>,
    pub(crate) platform: String,
    pub(crate) compact: bool,
    pub(crate) fullscreen: bool,
}

impl Default for WebViewOptions {
    fn default() -> Self {
        WebViewOptions {
            start_param: None,
            theme_params: None,
            platform: "unknown".into(),
            compact: false,
            fullscreen: false,
        }
    }
}

macro_rules! web_view_builder {
    ($name:ident) => {
        impl $name<'_> {
            pub fn start_param(mut self, param: impl Into<String>) -> Self {
                self.options.start_param = Some(param.into());
                self
            }

            pub fn theme_params(mut self, json: impl Into<String>) -> Self {
                let json = $crate::raw::types::DataJSON { data: json.into() };
                self.options.theme_params = Some($crate::raw::base::DataJSON::DataJSON(Box::new(json)));
                self
            }

            pub fn platform(mut self, platform: impl Into<String>) -> Self {
                self.options.platform = platform.into();
                self
            }

            pub fn compact(mut self) -> Self {
                self.options.compact = true;
                self
            }

            pub fn fullscreen(mut self) -> Self {
                self.options.fullscreen = true;
                self
            }
        }
    };
}

pub(crate) use web_view_builder;
