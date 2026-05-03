#![no_std]

#[cfg(test)]
extern crate std;

pub const DRIVER_MAX_CHANNELS: usize = 8;

pub mod backend;
mod driver;
pub mod error;
pub(crate) mod model;
pub mod setup;

#[cfg(test)]
pub(crate) mod test_support;

pub use backend::{
    AcquireWrite, BackendCapabilities, BackendChannelSpec, BackendError, BackendEvent,
    BackendSignal, BackendWriteLease, LedBackend, StartTransfer,
};
pub use driver::{
    ChannelHandle, ChannelHandles, ChannelWriter, ConfiguringDriver, Driver,
    pack::{PackError, pack_rgb48_active},
};
pub use error::{ConfigureError, ServiceError, WriteError};
pub use model::{BackendChannelId, FrameEpoch, PixelLayout, Rgb48};
pub use setup::ChannelSetup;
