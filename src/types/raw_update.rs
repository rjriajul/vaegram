use std::fmt;
use std::sync::Arc;

use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, User};
use crate::utils::get_peer;

/// An update as Telegram sent it, together with the users and chats that came in the same batch. Updates that
/// arrive alone (`updateShort`) carry none.
///
/// ```
/// use vaegram::types::{Chat, RawUpdate, User};
///
/// fn route(update: &RawUpdate) -> (u32, Vec<i64>, Vec<i64>, Option<User>, Option<Chat>) {
///     let users: Vec<i64> = update.user_ids().collect();
///     let user = users.first().and_then(|&id| update.user(id));
///     let chat = update.chat_ids().next().and_then(|id| update.chat(id));
///     (update.constructor_id(), users, update.chat_ids().collect(), user, chat)
/// }
/// ```
#[cfg_attr(
    not(feature = "unstable-raw"),
    doc = r#"
The TL objects themselves are not part of the default API:

```compile_fail
fn raw(update: &vaegram::types::RawUpdate) {
    let _ = update.raw();
}
```

```compile_fail
fn raw(update: vaegram::types::RawUpdate) {
    let _ = update.update;
}
```
"#
)]
#[derive(Clone)]
pub struct RawUpdate {
    pub(crate) update: base::Update,
    pub(crate) entities: Arc<Entities>,
}

impl RawUpdate {
    /// The TL constructor id of the update, e.g. `0x1f2b0afd` for `updateNewMessage`.
    pub fn constructor_id(&self) -> u32 {
        self.update.constructor_id()
    }

    /// Ascending.
    pub fn user_ids(&self) -> impl Iterator<Item = i64> + use<> {
        sorted(self.entities.users.keys())
    }

    /// Bot API style ids, ascending.
    pub fn chat_ids(&self) -> impl Iterator<Item = i64> + use<> {
        sorted(self.entities.chats.keys())
    }

    /// `None` when the user didn't come with this update.
    pub fn user(&self, id: i64) -> Option<User> {
        self.entities.users.contains_key(&id).then(|| User::parse(id, &self.entities))
    }

    /// `None` when the chat didn't come with this update.
    pub fn chat(&self, id: i64) -> Option<Chat> {
        self.entities.chats.contains_key(&id).then(|| Chat::parse(&get_peer(id), &self.entities, false))
    }

    /// Appends `{"update":{..},"users":[..],"chats":[..]}` in the format of [`crate::raw::ToJson`], with users and
    /// chats in id order.
    #[cfg(feature = "json")]
    pub fn to_json(&self, out: &mut String) {
        use crate::raw::ToJson;

        fn list<'a, T: ToJson + 'a>(out: &mut String, items: impl Iterator<Item = &'a T>) {
            out.push('[');
            for (i, item) in items.enumerate() {
                if i > 0 {
                    out.push(',');
                }
                item.to_json(out);
            }
            out.push(']');
        }
        out.push_str(r#"{"update":"#);
        self.update.to_json(out);
        out.push_str(r#","users":"#);
        list(out, self.user_ids().map(|id| &self.entities.users[&id]));
        out.push_str(r#","chats":"#);
        list(out, self.chat_ids().map(|id| &self.entities.chats[&id]));
        out.push('}');
    }
}

/// Experimental: these may change or go away in any release.
#[cfg(feature = "unstable-raw")]
impl RawUpdate {
    pub fn raw(&self) -> &base::Update {
        &self.update
    }

    /// In id order.
    pub fn raw_users(&self) -> impl Iterator<Item = &base::User> {
        self.user_ids().map(|id| &self.entities.users[&id])
    }

    /// In id order.
    pub fn raw_chats(&self) -> impl Iterator<Item = &base::Chat> {
        self.chat_ids().map(|id| &self.entities.chats[&id])
    }
}

impl fmt::Debug for RawUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawUpdate")
            .field("constructor_id", &format_args!("{:#010x}", self.constructor_id()))
            .field("user_ids", &self.user_ids().collect::<Vec<_>>())
            .field("chat_ids", &self.chat_ids().collect::<Vec<_>>())
            .finish()
    }
}

fn sorted<'a>(ids: impl Iterator<Item = &'a i64>) -> std::vec::IntoIter<i64> {
    let mut ids: Vec<i64> = ids.copied().collect();
    ids.sort_unstable();
    ids.into_iter()
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use std::sync::Arc;

    use super::RawUpdate;
    use crate::raw::{base, types};

    // Part of the frozen JSON shape (crate::raw::core::json).
    #[test]
    fn json_envelope_is_stable() {
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        let update = RawUpdate { update: base::Update::UpdateUserStatus(Box::new(status)), entities: Arc::default() };
        let mut json = String::new();
        update.to_json(&mut json);
        let expected =
            r#"{"update":{"_":"updateUserStatus","user_id":7,"status":{"_":"userStatusEmpty"}},"users":[],"chats":[]}"#;
        assert_eq!(json, expected);
    }
}
