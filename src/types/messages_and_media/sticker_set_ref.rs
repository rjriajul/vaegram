use crate::raw::{base, types};

/// A sticker set to look up: by its short name, or by the id and access hash a [`Sticker`](crate::types::Sticker)
/// or [`StickerSet`](crate::types::StickerSet) carries.
///
/// Make one from a short name (`"cats_by_bot".into()`), with [`Sticker::set_ref`](crate::types::Sticker::set_ref)
/// or with [`StickerSet::set_ref`](crate::types::StickerSet::set_ref). A set's access hash belongs to the account
/// that was given it, so a reference from one client may be refused by another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StickerSetRef(Kind);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Kind {
    Id { id: i64, access_hash: i64 },
    Name(String),
}

impl StickerSetRef {
    pub(crate) fn from_id(id: i64, access_hash: i64) -> Self {
        StickerSetRef(Kind::Id { id, access_hash })
    }

    /// Telegram's id of the set, or `None` for a reference by short name.
    pub fn id(&self) -> Option<i64> {
        match self.0 {
            Kind::Id { id, .. } => Some(id),
            Kind::Name(_) => None,
        }
    }

    /// The set's short name, or `None` for a reference by id.
    pub fn name(&self) -> Option<&str> {
        match &self.0 {
            Kind::Id { .. } => None,
            Kind::Name(name) => Some(name),
        }
    }

    pub(crate) fn input(&self) -> base::InputStickerSet {
        match &self.0 {
            Kind::Id { id, access_hash } => {
                let id = types::InputStickerSetID { id: *id, access_hash: *access_hash };
                base::InputStickerSet::InputStickerSetID(Box::new(id))
            }
            Kind::Name(name) => crate::utils::input_sticker_set(name),
        }
    }
}

impl From<&str> for StickerSetRef {
    fn from(name: &str) -> Self {
        StickerSetRef(Kind::Name(name.to_owned()))
    }
}

impl From<String> for StickerSetRef {
    fn from(name: String) -> Self {
        StickerSetRef(Kind::Name(name))
    }
}

impl From<&String> for StickerSetRef {
    fn from(name: &String) -> Self {
        StickerSetRef(Kind::Name(name.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::StickerSetRef;
    use crate::raw::{base, types};

    #[test]
    fn names_the_set_by_id_or_short_name() {
        let by_id = StickerSetRef::from_id(10, 11);
        assert_eq!((by_id.id(), by_id.name()), (Some(10), None));
        let id = types::InputStickerSetID { id: 10, access_hash: 11 };
        assert_eq!(by_id.input(), base::InputStickerSet::InputStickerSetID(Box::new(id)));

        let by_name = StickerSetRef::from("cats_by_bot");
        assert_eq!((by_name.id(), by_name.name()), (None, Some("cats_by_bot")));
        let name = types::InputStickerSetShortName { short_name: "cats_by_bot".into() };
        assert_eq!(by_name.input(), base::InputStickerSet::InputStickerSetShortName(Box::new(name)));
        assert_eq!(by_name, StickerSetRef::from(String::from("cats_by_bot")));
    }
}
