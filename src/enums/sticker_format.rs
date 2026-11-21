/// How a sticker is stored and animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum StickerFormat {
    /// A WebP image.
    Static,
    /// A Lottie animation (TGS).
    Animated,
    /// A WebM video.
    Video,
}
