use super::EngineError;
use crate::{
    backend::{BackendWriteLease, LedBackend},
    driver::pack::{PackError, pack_rgb48_active},
    model::{FrameEpoch, PixelLayout, Rgb48},
};

pub(in crate::driver) struct PreparedWrite<'a, B>
where
    B: LedBackend + 'a,
{
    pub(in crate::driver) layout: PixelLayout,
    pub(in crate::driver) frame_phase: FrameEpoch,
    pub(in crate::driver) lease: B::WriteLease<'a>,
}

impl<B: LedBackend> PreparedWrite<'_, B> {
    pub(in crate::driver) fn pack_rgb48_active(
        &mut self,
        source: &[Rgb48],
    ) -> Result<(), EngineError> {
        let target = self.lease.bytes_mut();
        pack_rgb48_active(source, target, self.layout, self.frame_phase).map_err(|err| match err {
            PackError::SourceLengthMismatch {
                source_pixels,
                target_pixels,
            } => EngineError::SourceLengthMismatch {
                expected_pixels: target_pixels,
                actual_pixels: source_pixels,
            },
        })
    }

    pub(in crate::driver) fn publish(&mut self) -> Result<(), EngineError> {
        self.lease.publish().map_err(EngineError::Backend)
    }
}
