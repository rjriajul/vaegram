use crate::enums::MaskPointType;
use crate::raw::{base, types};

/// Where a mask sticker sits on a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskPosition {
    /// The part of the face the mask is placed relative to.
    pub point: MaskPointType,
    /// Shift to the right, in mask widths scaled to the face.
    pub x_shift: f64,
    /// Shift down, in mask heights scaled to the face.
    pub y_shift: f64,
    /// Mask scaling coefficient. `2.0` makes the mask twice as big.
    pub scale: f64,
}

// Telegram never sends NaN coordinates, so equality is total and stickers and gifts can stay `Eq`.
impl Eq for MaskPosition {}

impl MaskPosition {
    /// Places a mask relative to `point`, shifted by `x_shift` mask widths right and `y_shift` mask heights down,
    /// and scaled by `scale`.
    pub fn new(point: MaskPointType, x_shift: f64, y_shift: f64, scale: f64) -> Self {
        MaskPosition { point, x_shift, y_shift, scale }
    }

    pub(crate) fn parse(coords: &base::MaskCoords) -> Option<MaskPosition> {
        let base::MaskCoords::MaskCoords(c) = coords;
        let point = match c.n {
            0 => MaskPointType::Forehead,
            1 => MaskPointType::Eyes,
            2 => MaskPointType::Mouth,
            3 => MaskPointType::Chin,
            _ => return None,
        };
        Some(MaskPosition { point, x_shift: c.x, y_shift: c.y, scale: c.zoom })
    }

    pub(crate) fn write(&self) -> base::MaskCoords {
        let coords = types::MaskCoords { n: self.point as i32, x: self.x_shift, y: self.y_shift, zoom: self.scale };
        base::MaskCoords::MaskCoords(Box::new(coords))
    }
}

#[cfg(test)]
mod tests {
    use super::MaskPosition;
    use crate::enums::MaskPointType;
    use crate::raw::{base, types};

    #[test]
    fn maps_the_face_point_both_ways() {
        let chin = MaskPosition { point: MaskPointType::Chin, x_shift: -1.0, y_shift: 0.5, scale: 2.0 };
        let coords = chin.write();
        let base::MaskCoords::MaskCoords(raw) = &coords;
        assert_eq!(**raw, types::MaskCoords { n: 3, x: -1.0, y: 0.5, zoom: 2.0 });
        assert_eq!(MaskPosition::parse(&coords), Some(chin));

        let unknown = base::MaskCoords::MaskCoords(Box::new(types::MaskCoords { n: 4, x: 0.0, y: 0.0, zoom: 1.0 }));
        assert_eq!(MaskPosition::parse(&unknown), None);
    }

    #[test]
    fn new_writes_its_values() {
        let eyes = MaskPosition::new(MaskPointType::Eyes, 0.5, -0.25, 1.5);
        let base::MaskCoords::MaskCoords(raw) = eyes.write();
        assert_eq!(*raw, types::MaskCoords { n: 1, x: 0.5, y: -0.25, zoom: 1.5 });
    }
}
