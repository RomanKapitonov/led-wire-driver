use super::EngineError;
use crate::{
    api::backend::{BackendWriteLease, LedBackend},
    model::{FrameEpoch, PixelLayout, Rgb48},
    pack::{
        ActiveSpatialQuantizer, ActiveTemporalDither, PackError, SpatialQuantizer, TemporalDither,
        WireColor, pack_into_bytes,
    },
};

/// Engine-owned owner for one acquired backend write lease.
///
/// This type is the concrete write object used by the runtime write path:
/// - it holds the live backend lease for one acquired write,
/// - it carries the metadata needed to pack into that lease,
/// - publish/abort ownership lives on the lease object rather than in parallel
///   engine bookkeeping.
pub(crate) struct PreparedWrite<'a, B>
where
    B: LedBackend + 'a,
{
    pub(crate) layout: PixelLayout,
    pub(crate) frame_phase: FrameEpoch,
    pub(crate) lease: B::WriteLease<'a>,
}

impl<B> PreparedWrite<'_, B>
where
    B: LedBackend,
{
    pub fn pack_rgb48_active(&mut self, source: &[Rgb48]) -> Result<(), EngineError> {
        self.pack_slice_with::<Rgb48, ActiveTemporalDither, ActiveSpatialQuantizer>(source)
    }

    fn pack_slice_with<Color, TD, SQ>(&mut self, source: &[Color]) -> Result<(), EngineError>
    where
        Color: WireColor + Copy,
        TD: TemporalDither + Default,
        SQ: SpatialQuantizer + Default,
    {
        let target = self.lease.bytes_mut();
        pack_into_bytes::<TD, SQ, Color>(source, target, self.layout, self.frame_phase).map_err(
            |err| match err {
                PackError::SourceLengthMismatch {
                    source_pixels,
                    target_pixels,
                } => EngineError::SourceLengthMismatch {
                    expected_pixels: target_pixels,
                    actual_pixels: source_pixels,
                },
            },
        )?;
        Ok(())
    }

    pub fn publish(&mut self) -> Result<(), EngineError> {
        self.lease.publish().map_err(EngineError::Backend)?;
        Ok(())
    }
}
