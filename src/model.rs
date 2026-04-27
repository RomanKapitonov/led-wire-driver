#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PixelLayout {
    Grb,
    Rgb,
    Bgr,
    Rbg,
    Gbr,
    Brg,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb48 {
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameEpoch(u32);

impl FrameEpoch {
    pub const ZERO: Self = Self(0);

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[cfg(feature = "pack-td-bayer")]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn wrapping_add(self, rhs: u32) -> Self {
        Self(self.0.wrapping_add(rhs))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelId(u8);

impl ChannelId {
    pub const CARDINALITY: usize = (u8::MAX as usize) + 1;

    /// Unchecked logical channel label.
    ///
    /// Validity is contextual: a `ChannelId` may still be absent from a given
    /// prepared setup or driver instance.
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    /// Converts a driver-visible logical channel index into a `ChannelId` when
    /// it fits in the identifier representation.
    pub const fn from_index(index: usize) -> Option<Self> {
        if index < Self::CARDINALITY {
            Some(Self(index as u8))
        } else {
            None
        }
    }

    pub const fn as_index(self) -> usize {
        self.0 as usize
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BackendChannelId(u8);

impl BackendChannelId {
    pub const CARDINALITY: usize = (u8::MAX as usize) + 1;

    /// Unchecked backend-owned channel label.
    ///
    /// Validity is contextual and depends on the concrete backend instance.
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn as_index(self) -> usize {
        self.0 as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_channel_id_roundtrip() {
        let id = BackendChannelId::new(42);
        assert_eq!(id.as_index(), 42);
    }

    #[test]
    fn frame_epoch_wrapping_add() {
        let epoch = FrameEpoch::from_raw(u32::MAX);
        assert_eq!(epoch.wrapping_add(1), FrameEpoch::from_raw(0));
    }
}
