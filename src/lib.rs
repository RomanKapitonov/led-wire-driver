#![no_std]

#[cfg(test)]
extern crate std;

/// Compile-time driver logical-channel storage capacity.
pub const DRIVER_MAX_CHANNELS: usize = 8;

pub(crate) mod model;
mod driver;
pub mod backend;
pub mod setup;
pub mod error;

#[cfg(test)]
pub(crate) mod test_support;

// Kept until effects-mcu callsites migrate in Task 10
pub(crate) mod engine;
pub(crate) mod pack;
pub mod api;

// ── Crate-level re-exports — callers use `led_wire_driver::X`, not submodules ──

pub use backend::{
    AcquireWrite, BackendCapabilities, BackendChannelSpec, BackendError, BackendEvent,
    BackendSignal, BackendWriteLease, LedBackend, StartTransfer,
};

pub use model::{BackendChannelId, FrameEpoch, PixelLayout, Rgb48};

pub use setup::ChannelSetup;

pub use error::{ConfigureError, ServiceError, WriteError};

pub use driver::{ChannelHandle, ChannelHandles, ChannelWriter, ConfiguringDriver, Driver};
