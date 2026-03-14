use super::{
    convert::{IsOff, ToWire},
    error::PackError,
    layout::layout_map,
    spatial::SpatialQuantizer,
    temporal::TemporalDither,
};
use crate::{engine::types::FrameEpoch, model::PixelLayout};

pub fn pack_into_bytes<TD, SQ, C>(
    source: &[C],
    target_bytes: &mut [u8],
    layout: PixelLayout,
    frame: FrameEpoch,
) -> Result<(), PackError>
where
    C: ToWire + IsOff + Copy,
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

    let temporal = TD::default();
    let t_nudge = temporal.offset(frame);
    let map = layout_map(layout);

    let mut spatial_r = SQ::default();
    let mut spatial_g = SQ::default();
    let mut spatial_b = SQ::default();

    for (index, (color, chunk)) in source
        .iter()
        .zip(target_bytes.chunks_exact_mut(3))
        .enumerate()
    {
        if color.is_off() {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
            continue;
        }

        let raw = color.to_wire();
        let r = spatial_r.quantize(raw[0].saturating_add_signed(t_nudge), index);
        let g = spatial_g.quantize(raw[1].saturating_add_signed(t_nudge), index);
        let b = spatial_b.quantize(raw[2].saturating_add_signed(t_nudge), index);
        chunk[map[0]] = r;
        chunk[map[1]] = g;
        chunk[map[2]] = b;
    }

    Ok(())
}
