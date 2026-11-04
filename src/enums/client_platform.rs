/// The platform web apps are told the client runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClientPlatform {
    Android,
    Ios,
    Wp,
    Bb,
    Desktop,
    Web,
    Ubp,
    Other,
}

impl ClientPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientPlatform::Android => "android",
            ClientPlatform::Ios => "ios",
            ClientPlatform::Wp => "wp",
            ClientPlatform::Bb => "bb",
            ClientPlatform::Desktop => "desktop",
            ClientPlatform::Web => "web",
            ClientPlatform::Ubp => "ubp",
            ClientPlatform::Other => "other",
        }
    }
}

impl From<ClientPlatform> for String {
    fn from(platform: ClientPlatform) -> String {
        platform.as_str().into()
    }
}

#[cfg(test)]
mod tests {
    use super::ClientPlatform as P;

    #[test]
    fn platforms_write_their_strings() {
        let all = [P::Android, P::Ios, P::Wp, P::Bb, P::Desktop, P::Web, P::Ubp, P::Other];
        let written: Vec<String> = all.into_iter().map(String::from).collect();
        assert_eq!(written, ["android", "ios", "wp", "bb", "desktop", "web", "ubp", "other"]);
    }
}
