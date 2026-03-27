use heapless::Vec;

use crate::{
    DRIVER_MAX_CHANNELS,
    model::{BackendChannelId, ChannelId, PixelLayout},
};

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
        let mut seen_backend = [false; BackendChannelId::CARDINALITY];

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
