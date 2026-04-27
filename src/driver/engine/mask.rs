#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) struct ChannelMask(u32);

impl ChannelMask {
    pub(in crate::driver) const EMPTY: Self = Self(0);
    pub(in crate::driver) const CAPACITY_BITS: usize = u32::BITS as usize;

    pub(in crate::driver) const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub(in crate::driver) const fn bits(self) -> u32 {
        self.0
    }

    /// Returns a mask with exactly one bit set at `index`.
    /// Caller must ensure `index < DRIVER_MAX_CHANNELS <= CAPACITY_BITS`.
    pub(in crate::driver) fn single(index: usize) -> Self {
        debug_assert!(index < Self::CAPACITY_BITS, "channel index exceeds mask capacity");
        Self(1u32 << index)
    }

    pub(in crate::driver) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(in crate::driver) fn contains(self, index: usize) -> bool {
        self.0 & (1u32 << index) != 0
    }

    pub(in crate::driver) const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(in crate::driver) const fn exclude(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_sets_correct_bit() {
        assert_eq!(ChannelMask::single(0).bits(), 1u32);
        assert_eq!(ChannelMask::single(3).bits(), 8u32);
        assert_eq!(ChannelMask::single(7).bits(), 128u32);
    }

    #[test]
    fn union_combines_bits() {
        assert_eq!(ChannelMask::single(0).union(ChannelMask::single(1)).bits(), 3u32);
    }

    #[test]
    fn contains_checks_bit() {
        let m = ChannelMask::single(5);
        assert!(m.contains(5));
        assert!(!m.contains(4));
    }

    #[test]
    fn exclude_clears_bits() {
        let m = ChannelMask::single(0).union(ChannelMask::single(1));
        assert_eq!(m.exclude(ChannelMask::single(0)).bits(), 2u32);
    }
}
