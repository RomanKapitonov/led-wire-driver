use crate::model::PixelLayout;

/// Returns the write index for (r, g, b) channels in the wire triplet.
pub const fn layout_map(layout: PixelLayout) -> [usize; 3] {
    match layout {
        PixelLayout::Grb => [1, 0, 2],
        PixelLayout::Rgb => [0, 1, 2],
        PixelLayout::Bgr => [2, 1, 0],
        PixelLayout::Rbg => [0, 2, 1],
        PixelLayout::Gbr => [1, 2, 0],
        PixelLayout::Brg => [2, 0, 1],
    }
}
