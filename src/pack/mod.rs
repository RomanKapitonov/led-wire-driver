//! Packing boundary for `driver`.
//!
//! Responsibilities:
//! - convert color-domain values into wire-order RGB byte triplets;
//! - apply temporal/spatial quantization policies;
//! - remain a pure data-plane module with no transfer/state policy.
//!
//! Internal shape:
//! - `pipeline`: packing kernel plus trivial wire/layout glue
//! - `spatial`: spatial quantization policy implementations
//! - `temporal`: temporal dithering policy implementations
//!
//! Feature selection is exclusive per axis:
//! - exactly one temporal policy feature must be enabled,
//! - exactly one spatial policy feature must be enabled,
//! - `--all-features` is intentionally unsupported and should fail with the
//!   compile-time guards below.
//! - revisit additive feature support only if external library/tooling
//!   requirements or new policy axes make it materially valuable.

mod pipeline;
mod spatial;
mod temporal;

pub(crate) use pipeline::PackError;

use crate::model::{FrameEpoch, PixelLayout, Rgb48};

#[cfg(all(feature = "pack-td-none", feature = "pack-td-bayer"))]
compile_error!("Select exactly one temporal dithering policy feature.");

#[cfg(not(any(feature = "pack-td-none", feature = "pack-td-bayer")))]
compile_error!("Select one temporal dithering policy feature.");

#[cfg(all(feature = "pack-sq-none", feature = "pack-sq-bayer"))]
compile_error!("Select exactly one spatial quantization policy feature.");

#[cfg(not(any(feature = "pack-sq-none", feature = "pack-sq-bayer")))]
compile_error!("Select one spatial quantization policy feature.");

#[cfg(all(feature = "pack-td-none", not(feature = "pack-td-bayer")))]
pub(crate) type ActiveTemporalDither = temporal::NoTemporalDither;

#[cfg(all(feature = "pack-td-bayer", not(feature = "pack-td-none")))]
pub(crate) type ActiveTemporalDither = temporal::TemporalBayerDither;

#[cfg(any(
    all(feature = "pack-td-none", feature = "pack-td-bayer"),
    not(any(feature = "pack-td-none", feature = "pack-td-bayer"))
))]
#[derive(Default)]
pub(crate) struct InvalidTemporalDither;

#[cfg(any(
    all(feature = "pack-td-none", feature = "pack-td-bayer"),
    not(any(feature = "pack-td-none", feature = "pack-td-bayer"))
))]
impl temporal::TemporalDither for InvalidTemporalDither {
    fn offset(&self, _frame: FrameEpoch) -> i16 {
        0
    }
}

#[cfg(any(
    all(feature = "pack-td-none", feature = "pack-td-bayer"),
    not(any(feature = "pack-td-none", feature = "pack-td-bayer"))
))]
pub(crate) type ActiveTemporalDither = InvalidTemporalDither;

#[cfg(all(feature = "pack-sq-none", not(feature = "pack-sq-bayer")))]
pub(crate) type ActiveSpatialQuantizer = spatial::NoSpatialQuantizer;

#[cfg(all(feature = "pack-sq-bayer", not(feature = "pack-sq-none")))]
pub(crate) type ActiveSpatialQuantizer = spatial::SpatialBayerQuantizer;

#[cfg(any(
    all(feature = "pack-sq-none", feature = "pack-sq-bayer"),
    not(any(feature = "pack-sq-none", feature = "pack-sq-bayer"))
))]
#[derive(Default)]
pub(crate) struct InvalidSpatialQuantizer;

#[cfg(any(
    all(feature = "pack-sq-none", feature = "pack-sq-bayer"),
    not(any(feature = "pack-sq-none", feature = "pack-sq-bayer"))
))]
impl spatial::SpatialQuantizer for InvalidSpatialQuantizer {
    fn quantize(&mut self, value: u16, _index: usize) -> u8 {
        (value >> 8) as u8
    }
}

#[cfg(any(
    all(feature = "pack-sq-none", feature = "pack-sq-bayer"),
    not(any(feature = "pack-sq-none", feature = "pack-sq-bayer"))
))]
pub(crate) type ActiveSpatialQuantizer = InvalidSpatialQuantizer;

pub(crate) fn pack_rgb48_active(
    source: &[Rgb48],
    target: &mut [u8],
    layout: PixelLayout,
    frame: FrameEpoch,
) -> Result<(), PackError> {
    pipeline::pack_into_bytes::<ActiveTemporalDither, ActiveSpatialQuantizer>(
        source, target, layout, frame,
    )
}
