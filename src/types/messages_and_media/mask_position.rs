use crate::enums::MaskPointType;
use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskPosition {
    pub point: MaskPointType,
    /// Shift to the right, in mask widths scaled to the face.
    pub x_shift: f64,
    /// Shift down, in mask heights scaled to the face.
    pub y_shift: f64,
    pub scale: f64,
}

// Telegram never sends NaN coordinates, so equality is total and stickers and gifts can stay `Eq`.
impl Eq for MaskPosition {}

impl MaskPosition {
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
}
