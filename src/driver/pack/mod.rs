mod pipeline;
mod spatial;
mod temporal;

pub(super) use pipeline::PackError;

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
pub(super) type ActiveTemporalDither = temporal::NoTemporalDither;
#[cfg(all(feature = "pack-td-bayer", not(feature = "pack-td-none")))]
pub(super) type ActiveTemporalDither = temporal::TemporalBayerDither;

#[cfg(all(feature = "pack-sq-none", not(feature = "pack-sq-bayer")))]
pub(super) type ActiveSpatialQuantizer = spatial::NoSpatialQuantizer;
#[cfg(all(feature = "pack-sq-bayer", not(feature = "pack-sq-none")))]
pub(super) type ActiveSpatialQuantizer = spatial::SpatialBayerQuantizer;

pub(super) fn pack_rgb48_active(
    source: &[Rgb48],
    target: &mut [u8],
    layout: PixelLayout,
    frame: FrameEpoch,
) -> Result<(), PackError> {
    pipeline::pack_into_bytes::<ActiveTemporalDither, ActiveSpatialQuantizer>(
        source, target, layout, frame,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FrameEpoch, PixelLayout, Rgb48};

    #[test]
    fn pack_single_pixel_grb_layout() {
        let source = [Rgb48 { r: 0xFFFF, g: 0x0000, b: 0x0000 }];
        let mut target = [0u8; 3];
        pack_rgb48_active(&source, &mut target, PixelLayout::Grb, FrameEpoch::ZERO).unwrap();
        assert_eq!(target[0], 0x00);
        assert_eq!(target[1], 0xFF);
        assert_eq!(target[2], 0x00);
    }

    #[test]
    fn pack_length_mismatch_returns_error() {
        let source = [Rgb48 { r: 0, g: 0, b: 0 }];
        let mut target = [0u8; 6];
        assert!(
            pack_rgb48_active(&source, &mut target, PixelLayout::Rgb, FrameEpoch::ZERO).is_err()
        );
    }
}
