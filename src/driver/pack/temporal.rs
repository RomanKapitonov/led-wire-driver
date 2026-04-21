use crate::model::FrameEpoch;

#[cfg_attr(feature = "pack-td-none", allow(dead_code))]
pub trait TemporalDither {
    fn offset(&self, frame: FrameEpoch) -> i16;
}

#[cfg(feature = "pack-td-none")]
#[derive(Default)]
pub struct NoTemporalDither;

#[cfg(feature = "pack-td-none")]
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
