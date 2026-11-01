use crate::raw::base;

/// One of the usernames of a user, bot or channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username {
    pub username: String,
    /// The one username that can be changed; the others are collectible.
    pub editable: bool,
    /// Shown on the profile and resolvable.
    pub active: bool,
}

impl Username {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse(raw: &base::Username) -> Username {
        let base::Username::Username(u) = raw;
        Username { username: u.username.clone(), editable: u.editable, active: u.active }
    }
}

#[cfg(test)]
mod tests {
    use super::Username;
    use crate::raw::{base, types};

    #[test]
    fn parses_editable_and_active() {
        let raw = |editable, active| {
            let u = types::Username { editable, active, deletable: false, expired: false, username: "neko".into() };
            Username::parse(&base::Username::Username(u.into()))
        };
        assert_eq!(raw(true, false), Username { username: "neko".into(), editable: true, active: false });
        assert_eq!(raw(false, true), Username { username: "neko".into(), editable: false, active: true });
    }
}
