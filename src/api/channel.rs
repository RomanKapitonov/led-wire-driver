use crate::{DRIVER_MAX_CHANNELS, model::ChannelId};

/// Internal driver-instance ownership tag for runtime channel handles.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct DriverId(u32);

impl DriverId {
    pub(crate) const fn new(raw: u32) -> Self {
        Self(raw)
    }
}

/// Opaque runtime channel handle returned from successful configuration.
///
/// Internally this handle binds a logical `ChannelId` to one concrete driver
/// instance via `DriverId`, so handles cannot be replayed across drivers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    driver_id: DriverId,
    id: ChannelId,
}

impl Channel {
    pub(crate) const fn new(driver_id: DriverId, id: ChannelId) -> Self {
        Self { driver_id, id }
    }

    /// Returns the internal driver-instance ownership tag carried by this
    /// handle. The public API uses it to reject handles from a different
    /// driver instance even when the logical channel number matches.
    pub(super) const fn owner(self) -> DriverId {
        self.driver_id
    }

    pub(crate) const fn as_index(self) -> usize {
        self.id.as_index()
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
