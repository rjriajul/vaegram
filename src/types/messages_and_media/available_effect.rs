use crate::raw::base;

/// A message effect that can be attached when sending a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableEffect {
    /// Effect id to pass when sending.
    pub id: i64,
    /// Emoji that represents the effect.
    pub emoji: String,
    /// `true` if only Premium users can send the effect.
    pub premium_required: bool,
    /// Id of the document used as the static icon. `None` if Telegram sent none.
    pub static_icon_id: Option<i64>,
    /// Id of the effect's sticker.
    pub effect_sticker_id: i64,
    /// Id of the document that holds the effect's animation. `None` if Telegram sent none.
    pub effect_animation_id: Option<i64>,
}

impl AvailableEffect {
    pub(crate) fn parse(raw: base::AvailableEffect) -> AvailableEffect {
        let base::AvailableEffect::AvailableEffect(e) = raw;
        AvailableEffect {
            id: e.id,
            emoji: e.emoticon,
            premium_required: e.premium_required,
            static_icon_id: e.static_icon_id,
            effect_sticker_id: e.effect_sticker_id,
            effect_animation_id: e.effect_animation_id,
        }
    }
}
