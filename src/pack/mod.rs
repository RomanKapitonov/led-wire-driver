//! Packing boundary for `driver`.
//!
//! Responsibilities:
//! - convert color-domain values into wire-order RGB byte triplets;
//! - apply temporal/spatial quantization policies;
//! - remain a pure data-plane module with no transfer/state policy.

mod convert;
mod error;
mod layout;
mod pipeline;
mod spatial;
mod temporal;

pub(crate) use convert::{IsOff, ToWire};
pub(crate) use error::PackError;
pub(crate) use pipeline::pack_into_bytes;
pub(crate) use spatial::SpatialQuantizer;
pub(crate) use temporal::TemporalDither;

#[cfg(all(feature = "pack-td-none", feature = "pack-td-bayer"))]
compile_error!("Select exactly one temporal dithering policy feature.");

#[cfg(not(any(feature = "pack-td-none", feature = "pack-td-bayer")))]
compile_error!("Select one temporal dithering policy feature.");

#[cfg(all(feature = "pack-sq-none", feature = "pack-sq-bayer"))]
compile_error!("Select exactly one spatial quantization policy feature.");

#[cfg(not(any(feature = "pack-sq-none", feature = "pack-sq-bayer")))]
compile_error!("Select one spatial quantization policy feature.");

#[cfg(feature = "pack-td-none")]
pub(crate) type ActiveTemporalDither = temporal::NoTemporalDither;

#[cfg(feature = "pack-td-bayer")]
pub(crate) type ActiveTemporalDither = temporal::TemporalBayerDither;

#[cfg(feature = "pack-sq-none")]
pub(crate) type ActiveSpatialQuantizer = spatial::NoSpatialQuantizer;

#[cfg(feature = "pack-sq-bayer")]
pub(crate) type ActiveSpatialQuantizer = spatial::SpatialBayerQuantizer;
