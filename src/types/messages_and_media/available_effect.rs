use crate::raw::base;

/// A message effect that can be attached when sending a message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AvailableEffect {
    /// Effect id to pass when sending.
    pub id: i64,
    /// Emoji that represents the effect.
    pub emoji: String,
    /// `true` if only Premium users can send the effect.
    pub is_premium: bool,
    /// Id of the document used as the effect's static icon.
    pub static_icon_id: Option<i64>,
    /// Id of the effect's sticker.
    pub effect_sticker_id: i64,
    /// Id of the document that holds the effect's full animation.
    pub effect_animation_id: Option<i64>,
}

impl AvailableEffect {
    pub(crate) fn parse(raw: base::AvailableEffect) -> AvailableEffect {
        let base::AvailableEffect::AvailableEffect(e) = raw;
        AvailableEffect {
            id: e.id,
            emoji: e.emoticon,
            is_premium: e.premium_required,
            static_icon_id: e.static_icon_id,
            effect_sticker_id: e.effect_sticker_id,
            effect_animation_id: e.effect_animation_id,
        }
    }
}
