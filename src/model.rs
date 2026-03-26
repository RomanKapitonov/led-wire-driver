#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PixelLayout {
    Grb,
    Rgb,
    Bgr,
    Rbg,
    Gbr,
    Brg,
}

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
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn as_index(self) -> usize {
        self.0 as usize
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BackendChannelId(u8);

impl BackendChannelId {
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }

    pub const fn as_index(self) -> usize {
        self.0 as usize
    }
}
