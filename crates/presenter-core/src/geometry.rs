use crate::{Error, Layout, View};

/// A rectangle in top-left display coordinates, unless documented as normalized.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }

    pub fn contains(self, other: Self) -> bool {
        let epsilon = 0.01;
        other.x >= self.x - epsilon
            && other.y >= self.y - epsilon
            && other.x + other.width <= self.x + self.width + epsilon
            && other.y + other.height <= self.y + self.height + epsilon
    }

    pub(crate) fn normalized_in(self, region: Self) -> Self {
        Self {
            x: (self.x - region.x) / region.width,
            y: (self.y - region.y) / region.height,
            width: self.width / region.width,
            height: self.height / region.height,
        }
    }
}

/// Effective CropBox/MediaBox and page rotation, independent of the renderer.
#[derive(Clone, Debug)]
pub struct PageInfo {
    /// Width of the visible page after rotation, in PDF points.
    pub width: f64,
    /// Height of the visible page after rotation, in PDF points.
    pub height: f64,
    /// Clockwise PDF rotation, one of 0, 90, 180, 270.
    pub rotation: u16,
    pub(crate) bounds: [f64; 4],
    pub(crate) animations: Vec<crate::Overlay>,
}

impl PageInfo {
    pub(crate) fn new(bounds: [f64; 4], rotation: i64) -> Result<Self, Error> {
        let [left, bottom, right, top] = bounds;
        let w = right - left;
        let h = top - bottom;
        if !bounds.iter().all(|value| value.is_finite())
            || w < 1.0
            || h < 1.0
            || w > 200_000.0
            || h > 200_000.0
        {
            return Err(Error::new("Invalid or excessively large PDF page bounds"));
        }
        if i32::try_from(rotation).is_err() {
            return Err(Error::new(
                "PDF page rotation exceeds the supported integer range",
            ));
        }
        let rotation = rotation.rem_euclid(360);
        if rotation % 90 != 0 {
            return Err(Error::new(
                "PDF page rotation must be a multiple of 90 degrees",
            ));
        }
        let (width, height) = if rotation == 90 || rotation == 270 {
            (h, w)
        } else {
            (w, h)
        };
        Ok(Self {
            width,
            height,
            rotation: rotation as u16,
            bounds,
            animations: Vec::new(),
        })
    }

    /// Converts a PDF bottom-left point to a visible page top-left point.
    pub fn pdf_point_to_display(&self, x: f64, y: f64) -> (f64, f64) {
        let u = x - self.bounds[0];
        let v = y - self.bounds[1];
        let w = self.bounds[2] - self.bounds[0];
        let h = self.bounds[3] - self.bounds[1];
        match self.rotation {
            90 => (v, u),
            180 => (w - u, v),
            270 => (h - v, w - u),
            _ => (u, h - v),
        }
    }

    /// Converts an axis-aligned PDF rectangle into rotated display coordinates.
    pub fn pdf_rect_to_display(&self, bounds: [f64; 4]) -> Result<Rect, Error> {
        if !bounds.iter().all(|value| value.is_finite()) {
            return Err(Error::new(
                "Animation rectangle contains a non-finite coordinate",
            ));
        }
        let a = self.pdf_point_to_display(bounds[0], bounds[1]);
        let b = self.pdf_point_to_display(bounds[2], bounds[3]);
        let rectangle = Rect {
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            width: (a.0 - b.0).abs(),
            height: (a.1 - b.1).abs(),
        };
        if rectangle.width <= 0.0 || rectangle.height <= 0.0 {
            return Err(Error::new("Animation rectangle is empty"));
        }
        Ok(rectangle)
    }

    pub fn region(&self, layout: Layout, view: View) -> Result<Rect, Error> {
        let width = match layout {
            Layout::Normal if view == View::Notes => {
                return Err(Error::new("Normal layout has no speaker notes region"));
            }
            Layout::Normal => self.width,
            Layout::NotesRight => self.width / 2.0,
        };
        Ok(Rect {
            x: if view == View::Notes { width } else { 0.0 },
            y: 0.0,
            width,
            height: self.height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_origins_and_rotations() {
        let bounds = [10.0, 20.0, 210.0, 120.0];
        let examples = [
            (0, (30.0, 70.0), (200.0, 100.0)),
            (90, (30.0, 30.0), (100.0, 200.0)),
            (180, (170.0, 30.0), (200.0, 100.0)),
            (270, (70.0, 170.0), (100.0, 200.0)),
        ];
        for (rotation, expected_point, expected_size) in examples {
            let page = PageInfo::new(bounds, rotation).unwrap();
            assert_eq!(page.pdf_point_to_display(40.0, 50.0), expected_point);
            assert_eq!((page.width, page.height), expected_size);
            let rect = page.pdf_rect_to_display(bounds).unwrap();
            assert_eq!(
                rect,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: page.width,
                    height: page.height
                }
            );
        }
    }

    #[test]
    fn rotated_notes_split_uses_visible_page() {
        let page = PageInfo::new([10.0, 20.0, 210.0, 120.0], 90).unwrap();
        let audience = page.region(Layout::NotesRight, View::Audience).unwrap();
        let notes = page.region(Layout::NotesRight, View::Notes).unwrap();
        assert_eq!(
            audience,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 200.0
            }
        );
        assert_eq!(
            notes,
            Rect {
                x: 50.0,
                ..audience
            }
        );
        assert!(page.region(Layout::Normal, View::Notes).is_err());
    }

    #[test]
    fn bounds_must_be_finite_positive_and_reasonable() {
        assert!(PageInfo::new([0.0, 0.0, f64::NAN, 10.0], 0).is_err());
        assert!(PageInfo::new([0.0, 0.0, 1.0, 1.0], 45).is_err());
        assert!(PageInfo::new([0.0, 0.0, 0.0, 10.0], 0).is_err());
        assert_eq!(
            PageInfo::new([0.0, 0.0, 20.0, 10.0], -90).unwrap().rotation,
            270
        );
    }
}
