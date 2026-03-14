use crate::{
    api::backend::LedBackend,
    engine::types::FrameEpoch,
    pack::{IsOff, PackError, SpatialQuantizer, TemporalDither, ToWire, pack_into_bytes},
};

use super::{EngineError, LedEngine, types::WritePlan};

impl<B> LedEngine<B>
where
    B: LedBackend,
{
    pub fn write_slice_to_plan<Color, TD, SQ>(
        source: &[Color],
        frame_count: FrameEpoch,
        plan: WritePlan,
    ) -> Result<(), EngineError>
    where
        Color: ToWire + IsOff + Copy,
        TD: TemporalDither + Default,
        SQ: SpatialQuantizer + Default,
    {
        plan.target.with_mut_bytes(|target| {
            pack_into_bytes::<TD, SQ, Color>(source, target, plan.layout, frame_count)
                .map_err(|err| match err {
                    PackError::SourceLengthMismatch {
                        source_pixels,
                        target_pixels,
                    } => EngineError::SourceLengthMismatch {
                        expected_pixels: target_pixels,
                        actual_pixels: source_pixels,
                    },
                })
        })?;
        Ok(())
    }
}
