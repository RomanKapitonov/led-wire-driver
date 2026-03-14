#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameEpoch(u32);

impl FrameEpoch {
    pub const ZERO: Self = Self(0);

    #[cfg(feature = "pack-td-bayer")]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn wrapping_add(self, rhs: u32) -> Self {
        Self(self.0.wrapping_add(rhs))
    }
}
