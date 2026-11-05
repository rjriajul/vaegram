use crate::raw::{base, types};

/// One model, symbol or backdrop to filter resale listings by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpgradedGiftAttributeId {
    /// The sticker id of a model.
    Model(i64),
    /// The sticker id of a symbol.
    Symbol(i64),
    /// The id of a backdrop.
    Backdrop(i32),
}

impl UpgradedGiftAttributeId {
    pub(crate) fn write(self) -> base::StarGiftAttributeId {
        match self {
            UpgradedGiftAttributeId::Model(document_id) => {
                base::StarGiftAttributeId::StarGiftAttributeIdModel(types::StarGiftAttributeIdModel { document_id })
            }
            UpgradedGiftAttributeId::Symbol(document_id) => {
                base::StarGiftAttributeId::StarGiftAttributeIdPattern(types::StarGiftAttributeIdPattern { document_id })
            }
            UpgradedGiftAttributeId::Backdrop(backdrop_id) => {
                base::StarGiftAttributeId::StarGiftAttributeIdBackdrop(types::StarGiftAttributeIdBackdrop {
                    backdrop_id,
                })
            }
        }
    }
}
