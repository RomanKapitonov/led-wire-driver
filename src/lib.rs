#![no_std]

#[cfg(test)]
extern crate std;

pub const DRIVER_MAX_CHANNELS: usize = 8;

pub(crate) mod model;
mod driver;
pub mod backend;
pub mod setup;
pub mod error;

#[cfg(test)]
pub(crate) mod test_support;

pub use backend::{
    AcquireWrite, BackendCapabilities, BackendChannelSpec, BackendError, BackendEvent,
    BackendSignal, BackendWriteLease, LedBackend, StartTransfer,
};

pub use model::{BackendChannelId, FrameEpoch, PixelLayout, Rgb48};

pub use setup::ChannelSetup;

pub use error::{ConfigureError, ServiceError, WriteError};

pub use driver::{ChannelHandle, ChannelHandles, ChannelWriter, ConfiguringDriver, Driver};

pub use driver::pack::{pack_rgb48_active, PackError};
