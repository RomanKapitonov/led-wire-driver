use crate::DRIVER_MAX_CHANNELS;
use heapless::Vec;

pub use crate::model::{BackendChannelId, ChannelId, PixelLayout, Rgb24};

/// Structural setup data that has already been validated by the preparation
/// boundary (bootstrap/system) and is ready for driver configuration.
#[derive(Debug)]
struct PreparedChannelBinding {
    logical_channel: ChannelId,
    backend_channel: BackendChannelId,
    pixels: u16,
    layout: PixelLayout,
}

impl PreparedChannelBinding {
    const fn new(
        logical_channel: ChannelId,
        backend_channel: BackendChannelId,
        pixels: u16,
        layout: PixelLayout,
    ) -> Self {
        Self {
            logical_channel,
            backend_channel,
            pixels,
            layout,
        }
    }
}

type PreparedBindings = Vec<PreparedChannelBinding, { DRIVER_MAX_CHANNELS }>;

/// Opaque prepared setup capability consumed by driver configuration.
///
/// Construction is restricted to the bootstrap preparation boundary.
#[derive(Debug)]
pub struct PreparedSetup {
    bindings: PreparedBindings,
}

impl PreparedSetup {
    pub fn new() -> Self {
        Self {
            bindings: PreparedBindings::new(),
        }
    }

    pub fn push_binding(
        &mut self,
        logical_channel: ChannelId,
        backend_channel: BackendChannelId,
        pixels: u16,
        layout: PixelLayout,
    ) -> Result<(), ()> {
        self.bindings
            .push(PreparedChannelBinding::new(
                logical_channel,
                backend_channel,
                pixels,
                layout,
            ))
            .map_err(|_| ())
    }

    pub(crate) fn iter(
        &self,
    ) -> impl Iterator<Item = (ChannelId, BackendChannelId, u16, PixelLayout)> + '_ {
        self.bindings
            .iter()
            .map(|binding| {
                (
                    binding.logical_channel,
                    binding.backend_channel,
                    binding.pixels,
                    binding.layout,
                )
            })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    driver_id: u32,
    index: u8,
}

impl Channel {
    pub(super) const fn new(driver_id: u32, index: u8) -> Self {
        Self { driver_id, index }
    }

    pub(super) const fn owner(self) -> u32 {
        self.driver_id
    }

    pub(crate) const fn as_index(self) -> usize {
        self.index as usize
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ConfiguredChannels {
    entries: [Option<Channel>; DRIVER_MAX_CHANNELS],
}

impl ConfiguredChannels {
    pub(crate) const fn from_entries(entries: [Option<Channel>; DRIVER_MAX_CHANNELS]) -> Self {
        Self { entries }
    }

    pub fn get(&self, id: ChannelId) -> Option<Channel> {
        self.entries.get(id.as_index()).copied().flatten()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    InvalidBinding,
    DuplicateChannel,
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriverInitError {
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FinalizeError {
    InvalidBinding,
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    Busy,
    InvalidChannel,
    LengthMismatch,
    Backend,
}

