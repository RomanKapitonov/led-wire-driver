use super::{
    convert::WireColor, error::PackError, layout::layout_map, spatial::SpatialQuantizer,
    temporal::TemporalDither,
};
use crate::{engine::types::FrameEpoch, model::PixelLayout};

#[cfg(feature = "pack-sq-none")]
#[inline(always)]
fn downscale_to_u8(value: u16) -> u8 {
    (value >> 8) as u8
}

#[cfg(feature = "pack-td-bayer")]
#[inline(always)]
fn temporal_offset<TD>(frame: FrameEpoch) -> i16
where
    TD: TemporalDither + Default,
{
    TD::default().offset(frame)
}

#[cfg(feature = "pack-td-none")]
#[inline(always)]
fn temporal_offset<TD>(_frame: FrameEpoch) -> i16
where
    TD: TemporalDither + Default,
{
    0
}

#[cfg(feature = "pack-td-bayer")]
#[inline(always)]
fn apply_temporal(value: u16, t_nudge: i16) -> u16 {
    value.saturating_add_signed(t_nudge)
}

#[cfg(feature = "pack-td-none")]
#[inline(always)]
fn apply_temporal(value: u16, _t_nudge: i16) -> u16 {
    value
}

#[cfg(feature = "pack-sq-bayer")]
#[inline(always)]
fn quantize_channel<SQ>(quantizer: &mut SQ, value: u16, index: usize) -> u8
where
    SQ: SpatialQuantizer,
{
    quantizer.quantize(value, index)
}

#[cfg(feature = "pack-sq-none")]
#[inline(always)]
fn quantize_channel<SQ>(_quantizer: &mut SQ, value: u16, _index: usize) -> u8
where
    SQ: SpatialQuantizer,
{
    downscale_to_u8(value)
}

#[inline(always)]
fn pack_kernel<TD, SQ, C, const R_INDEX: usize, const G_INDEX: usize, const B_INDEX: usize>(
    source: &[C],
    target_bytes: &mut [u8],
    frame: FrameEpoch,
) where
    C: WireColor + Copy,
    TD: TemporalDither + Default,
    SQ: SpatialQuantizer + Default,
{
    let t_nudge = temporal_offset::<TD>(frame);
    let mut spatial_r = SQ::default();
    let mut spatial_g = SQ::default();
    let mut spatial_b = SQ::default();

    for (index, (color, chunk)) in source
        .iter()
        .zip(target_bytes.chunks_exact_mut(3))
        .enumerate()
    {
        if color.is_off() {
            chunk.fill(0);
            continue;
        }

        let raw = color.to_wire();
        let r = quantize_channel(&mut spatial_r, apply_temporal(raw[0], t_nudge), index);
        let g = quantize_channel(&mut spatial_g, apply_temporal(raw[1], t_nudge), index);
        let b = quantize_channel(&mut spatial_b, apply_temporal(raw[2], t_nudge), index);
        chunk[R_INDEX] = r;
        chunk[G_INDEX] = g;
        chunk[B_INDEX] = b;
    }
}

pub fn pack_into_bytes<TD, SQ, C>(
    source: &[C],
    target_bytes: &mut [u8],
    layout: PixelLayout,
    frame: FrameEpoch,
) -> Result<(), PackError>
where
    C: WireColor + Copy,
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

    match layout_map(layout) {
        [0, 1, 2] => pack_kernel::<TD, SQ, C, 0, 1, 2>(source, target_bytes, frame),
        [0, 2, 1] => pack_kernel::<TD, SQ, C, 0, 2, 1>(source, target_bytes, frame),
        [1, 0, 2] => pack_kernel::<TD, SQ, C, 1, 0, 2>(source, target_bytes, frame),
        [1, 2, 0] => pack_kernel::<TD, SQ, C, 1, 2, 0>(source, target_bytes, frame),
        [2, 0, 1] => pack_kernel::<TD, SQ, C, 2, 0, 1>(source, target_bytes, frame),
        [2, 1, 0] => pack_kernel::<TD, SQ, C, 2, 1, 0>(source, target_bytes, frame),
        _ => unreachable!("layout_map always yields one of six channel permutations"),
    }

    Ok(())
}
