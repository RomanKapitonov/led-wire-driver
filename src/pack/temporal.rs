use crate::engine::types::FrameEpoch;

pub trait TemporalDither {
    fn offset(&self, frame: FrameEpoch) -> i16;
}

#[derive(Default)]
pub struct NoTemporalDither;

impl TemporalDither for NoTemporalDither {
    fn offset(&self, _frame: FrameEpoch) -> i16 {
        0
    }
}

#[cfg(feature = "pack-td-bayer")]
#[derive(Default)]
pub struct TemporalBayerDither;

#[cfg(feature = "pack-td-bayer")]
impl TemporalDither for TemporalBayerDither {
    fn offset(&self, frame: FrameEpoch) -> i16 {
        match frame.as_u32() % 4 {
            0 => -32,
            1 => 32,
            2 => -16,
            _ => 16,
        }
    }
}
