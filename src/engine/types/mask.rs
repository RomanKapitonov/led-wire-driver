#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelMask(u32);

impl ChannelMask {
    pub const ZERO: Self = Self(0);
    pub const CAPACITY_BITS: usize = u32::BITS as usize;

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}
