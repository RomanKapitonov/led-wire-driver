use heapless::Vec;

use crate::DRIVER_MAX_CHANNELS;
pub use crate::model::{BackendChannelId, ChannelId, PixelLayout, Rgb48};

/// One validated structural mapping from a logical driver channel to one
/// backend-owned wire target.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PreparedBinding {
    pub logical_channel: ChannelId,
    pub backend_channel: BackendChannelId,
    pub pixels: u16,
    pub layout: PixelLayout,
}

impl PreparedBinding {
    pub const fn new(
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

type PreparedBindings = Vec<PreparedBinding, { DRIVER_MAX_CHANNELS }>;

#[derive(Debug)]
struct SetupDraft {
    bindings: PreparedBindings,
}

impl SetupDraft {
    const fn new() -> Self {
        Self {
            bindings: PreparedBindings::new(),
        }
    }

    fn push_binding(&mut self, binding: PreparedBinding) -> Result<(), SetupBuildError> {
        self.bindings
            .push(binding)
            .map_err(|_| SetupBuildError::CapacityExceeded)
    }

    fn validate(self) -> Result<PreparedSetup, SetupBuildError> {
        let mut seen_logical = [false; DRIVER_MAX_CHANNELS];
        let mut seen_backend = [false; (u8::MAX as usize) + 1];

        for binding in &self.bindings {
            let logical_index = binding.logical_channel.as_index();
            if logical_index >= DRIVER_MAX_CHANNELS {
                return Err(SetupBuildError::InvalidLogicalChannel);
            }
            if seen_logical[logical_index] {
                return Err(SetupBuildError::DuplicateLogicalChannel);
            }
            seen_logical[logical_index] = true;

            let backend_index = binding.backend_channel.as_index();
            if seen_backend[backend_index] {
                return Err(SetupBuildError::DuplicateBackendChannel);
            }
            seen_backend[backend_index] = true;

            if binding.pixels == 0 {
                return Err(SetupBuildError::InvalidPixelCount);
            }
            let _wire_bytes = u32::from(binding.pixels)
                .checked_mul(3)
                .ok_or(SetupBuildError::OverflowingWireSize)?;
        }

        Ok(PreparedSetup {
            bindings: self.bindings,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SetupBuildError {
    /// More bindings were provided than the driver can store.
    CapacityExceeded,
    /// A logical channel index exceeded the driver's public channel capacity.
    InvalidLogicalChannel,
    /// Two bindings targeted the same logical channel.
    DuplicateLogicalChannel,
    /// Two bindings targeted the same backend-owned channel identity.
    DuplicateBackendChannel,
    /// A channel declared zero pixels.
    InvalidPixelCount,
    /// `pixels * 3` overflowed while deriving wire bytes.
    OverflowingWireSize,
}

/// Immutable prepared setup capability consumed by driver configuration.
///
/// The public API does not expose mutable setup construction. Callers hand
/// prepared bindings into `from_bindings(...)`, which validates them before a
/// `PreparedSetup` is produced.
#[derive(Debug)]
pub struct PreparedSetup {
    bindings: PreparedBindings,
}

impl PreparedSetup {
    /// Validates a binding list and produces an immutable prepared setup.
    pub fn from_bindings<I>(bindings: I) -> Result<Self, SetupBuildError>
    where
        I: IntoIterator<Item = PreparedBinding>,
    {
        let mut draft = SetupDraft::new();
        for binding in bindings {
            draft.push_binding(binding)?;
        }
        draft.validate()
    }

    /// Returns the validated binding list as a stable read-only slice.
    pub fn bindings(&self) -> &[PreparedBinding] {
        self.bindings.as_slice()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PreparedBinding> + '_ {
        self.bindings().iter()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    driver_id: u32,
    index: u8,
}

impl Channel {
    pub(crate) const fn new(driver_id: u32, index: u8) -> Self {
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
    /// The setup or backend binding shape is invalid for this driver/backend.
    InvalidBinding,
    /// Two bindings resolved to the same logical channel during registration.
    DuplicateChannel,
    /// The caller attempted to configure with an empty prepared setup.
    EmptyConfiguration,
    /// The driver already committed one configuration; registration is
    /// single-shot.
    AlreadyConfigured,
    /// Backend configuration failed for a backend-owned reason.
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriverInitError {
    /// Backend initialization failed before the driver entered registration.
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// The backend cannot currently hand out a writable target.
    Busy,
    /// The supplied channel handle is invalid for this driver or channel.
    InvalidChannel,
    /// The supplied source slice length does not match the configured channel.
    LengthMismatch,
    /// The backend violated a runtime contract expected by the driver.
    BackendContract,
    /// A genuine backend-owned runtime failure occurred.
    Backend,
}
