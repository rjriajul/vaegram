use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableEffect {
    pub id: i64,
    pub emoji: String,
    pub premium_required: bool,
    pub static_icon_id: Option<i64>,
    pub effect_sticker_id: i64,
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
