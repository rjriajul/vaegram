/// The part of a face a mask sticker is placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MaskPointType {
    /// The forehead.
    Forehead = 0,
    /// The eyes.
    Eyes = 1,
    /// The mouth.
    Mouth = 2,
    /// The chin.
    Chin = 3,
}
