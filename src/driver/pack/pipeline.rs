use super::{spatial::SpatialQuantizer, temporal::TemporalDither};
use crate::model::{FrameEpoch, PixelLayout, Rgb48};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    SourceLengthMismatch {
        source_pixels: usize,
        target_pixels: usize,
    },
}

/// Returns the write index for (r, g, b) channels in the wire triplet.
const fn layout_map(layout: PixelLayout) -> [usize; 3] {
    match layout {
        PixelLayout::Grb => [1, 0, 2],
        PixelLayout::Rgb => [0, 1, 2],
        PixelLayout::Bgr => [2, 1, 0],
        PixelLayout::Rbg => [0, 2, 1],
        PixelLayout::Gbr => [1, 2, 0],
        PixelLayout::Brg => [2, 0, 1],
    }
}

fn apply_temporal(value: u16, t_nudge: i16) -> u16 {
    value.saturating_add_signed(t_nudge)
}

fn pack_kernel<TD, SQ, const R_INDEX: usize, const G_INDEX: usize, const B_INDEX: usize>(
    source: &[Rgb48],
    target_bytes: &mut [u8],
    frame: FrameEpoch,
) where
    TD: TemporalDither + Default,
    SQ: SpatialQuantizer + Default,
{
    let t_nudge = TD::default().offset(frame);
    let mut spatial_r = SQ::default();
    let mut spatial_g = SQ::default();
    let mut spatial_b = SQ::default();

    for (index, (color, chunk)) in source
        .iter()
        .zip(target_bytes.chunks_exact_mut(3))
        .enumerate()
    {
        if (color.r | color.g | color.b) == 0 {
            chunk.fill(0);
            continue;
        }

        let r = spatial_r.quantize(apply_temporal(color.r, t_nudge), index);
        let g = spatial_g.quantize(apply_temporal(color.g, t_nudge), index);
        let b = spatial_b.quantize(apply_temporal(color.b, t_nudge), index);
        chunk[R_INDEX] = r;
        chunk[G_INDEX] = g;
        chunk[B_INDEX] = b;
    }
}

pub fn pack_into_bytes<TD, SQ>(
    source: &[Rgb48],
    target_bytes: &mut [u8],
    layout: PixelLayout,
    frame: FrameEpoch,
) -> Result<(), PackError>
where
    TD: TemporalDither + Default,
    SQ: SpatialQuantizer + Default,
{
    debug_assert!(
        target_bytes.len().is_multiple_of(3),
        "wire target must be sized in RGB24 triplets"
    );
    let target_pixels = target_bytes.len() / 3;

    if source.len() != target_pixels {
        return Err(PackError::SourceLengthMismatch {
            source_pixels: source.len(),
            target_pixels,
        });
    }

    // Each arm destructures the layout permutation from layout_map() and passes
    // the three indices directly as const generics. Keep the match pattern and
    // the type parameters in sync: [r, g, b] => pack_kernel::<..., r, g, b>.
    match layout_map(layout) {
        [0, 1, 2] => pack_kernel::<TD, SQ, 0, 1, 2>(source, target_bytes, frame),
        [0, 2, 1] => pack_kernel::<TD, SQ, 0, 2, 1>(source, target_bytes, frame),
        [1, 0, 2] => pack_kernel::<TD, SQ, 1, 0, 2>(source, target_bytes, frame),
        [1, 2, 0] => pack_kernel::<TD, SQ, 1, 2, 0>(source, target_bytes, frame),
        [2, 0, 1] => pack_kernel::<TD, SQ, 2, 0, 1>(source, target_bytes, frame),
        [2, 1, 0] => pack_kernel::<TD, SQ, 2, 1, 0>(source, target_bytes, frame),
        _ => unreachable!("layout_map always yields one of six channel permutations"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{spatial::SpatialQuantizer, temporal::TemporalDither};

    #[derive(Default)]
    struct NoopTemporal;

    impl TemporalDither for NoopTemporal {
        fn offset(&self, _frame: FrameEpoch) -> i16 {
            0
        }
    }

    #[derive(Default)]
    struct NudgeTemporal;

    impl TemporalDither for NudgeTemporal {
        fn offset(&self, _frame: FrameEpoch) -> i16 {
            256
        }
    }

    #[derive(Default)]
    struct ShiftSpatial;

    impl SpatialQuantizer for ShiftSpatial {
        fn quantize(&mut self, value: u16, _index: usize) -> u8 {
            (value >> 8) as u8
        }
    }

    #[test]
    fn rejects_source_length_mismatch() {
        let source = [Rgb48 { r: 0, g: 0, b: 0 }];
        let mut target = [0u8; 6];

        let err = pack_into_bytes::<NoopTemporal, ShiftSpatial>(
            &source,
            &mut target,
            PixelLayout::Rgb,
            FrameEpoch::ZERO,
        )
        .expect_err("length mismatch should be rejected");

        assert_eq!(
            err,
            PackError::SourceLengthMismatch {
                source_pixels: 1,
                target_pixels: 2,
            }
        );
    }

    #[test]
    fn maps_layout_indices_correctly() {
        let source = [Rgb48 {
            r: 0x1100,
            g: 0x2200,
            b: 0x3300,
        }];

        let mut rgb = [0u8; 3];
        pack_into_bytes::<NoopTemporal, ShiftSpatial>(
            &source,
            &mut rgb,
            PixelLayout::Rgb,
            FrameEpoch::ZERO,
        )
        .expect("rgb layout should pack");
        assert_eq!(rgb, [0x11, 0x22, 0x33]);

        let mut grb = [0u8; 3];
        pack_into_bytes::<NoopTemporal, ShiftSpatial>(
            &source,
            &mut grb,
            PixelLayout::Grb,
            FrameEpoch::ZERO,
        )
        .expect("grb layout should pack");
        assert_eq!(grb, [0x22, 0x11, 0x33]);

        let mut brg = [0u8; 3];
        pack_into_bytes::<NoopTemporal, ShiftSpatial>(
            &source,
            &mut brg,
            PixelLayout::Brg,
            FrameEpoch::ZERO,
        )
        .expect("brg layout should pack");
        assert_eq!(brg, [0x22, 0x33, 0x11]);
    }

    #[test]
    fn off_pixels_zero_the_target_chunk() {
        let source = [
            Rgb48 { r: 0, g: 0, b: 0 },
            Rgb48 {
                r: 0x0100,
                g: 0x0200,
                b: 0x0300,
            },
        ];
        let mut target = [0xFFu8; 6];

        pack_into_bytes::<NoopTemporal, ShiftSpatial>(
            &source,
            &mut target,
            PixelLayout::Rgb,
            FrameEpoch::ZERO,
        )
        .expect("pack should succeed");

        assert_eq!(&target[..3], &[0, 0, 0]);
        assert_eq!(&target[3..], &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn temporal_and_spatial_policies_affect_output() {
        let source = [Rgb48 { r: 0, g: 0, b: 0 }];
        let mut target = [0xAAu8; 3];

        pack_into_bytes::<NudgeTemporal, ShiftSpatial>(
            &source,
            &mut target,
            PixelLayout::Rgb,
            FrameEpoch::ZERO,
        )
        .expect("pack should succeed");

        assert_eq!(target, [0, 0, 0]);

        let source = [Rgb48 {
            r: 0x0001,
            g: 0x0001,
            b: 0x0001,
        }];
        let mut target = [0u8; 3];

        pack_into_bytes::<NudgeTemporal, ShiftSpatial>(
            &source,
            &mut target,
            PixelLayout::Rgb,
            FrameEpoch::ZERO,
        )
        .expect("temporal nudge should affect quantized output");

        assert_eq!(target, [1, 1, 1]);
    }
}
