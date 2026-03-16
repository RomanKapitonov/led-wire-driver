//! Authoritative driver engine state machine.
//!
//! Internal engine layout:
//! - [`registration`]: registered-channel model and semantic channel metadata
//! - [`runtime`]: write/submit progression and transfer progression
//! - [`write`]: pack/write into resolved wire targets
//! - [`error`]: internal operational error vocabulary
//!
//! The engine is intentionally the only owner of driver runtime state.
//! Registration validity is intentionally split:
//! - structural channel/pixel/layout validity happens in the bootstrap
//!   preparation boundary
//! - lifecycle call-order validity is enforced in `driver::api` typestate
//! - backend resource/wire target validity is enforced by backend
//!
//! Engine invariants:
//! - `InFlight` is entered only after backend returns `StartTransfer::Started`
//! - busy start does not clear pending submissions
//! - driver runtime never owns transport slot/buffer strategy
//! - channel phase advances only for committed dirty channels

mod error;
pub(crate) mod registration;
mod runtime;
pub(crate) mod types;
mod write;

pub use error::EngineError;

use self::{
    registration::{ChannelState, RegistrationTable},
    runtime::{EngineLifecycle, EngineState, ReadyState},
    types::ChannelMask,
};
use crate::DRIVER_MAX_CHANNELS;
use crate::api::backend::{BackendChannelSpec, BackendError, LedBackend};

pub struct LedEngine<B>
where
    B: LedBackend,
{
    backend: B,
    max_channels: usize,
    state: EngineState,
    channels: RegistrationTable,
}

impl<B> LedEngine<B>
where
    B: LedBackend,
{
    /// Creates a new engine instance.
    ///
    /// The current internal channel mask uses a `u32`, so the engine supports
    /// at most 32 channels until that representation changes.
    pub fn new(backend: B) -> Self {
        assert!(
            DRIVER_MAX_CHANNELS <= ChannelMask::CAPACITY_BITS,
            "LedEngine supports at most {} channels because ChannelMask is u32",
            ChannelMask::CAPACITY_BITS
        );
        let max_channels = backend.capabilities().max_channels.min(DRIVER_MAX_CHANNELS);
        Self {
            backend,
            max_channels,
            state: EngineState::new(),
            channels: RegistrationTable::new(),
        }
    }

    pub fn init(&mut self) -> Result<(), EngineError> {
        if matches!(self.state.lifecycle, EngineLifecycle::Uninitialized) {
            self.backend.init().map_err(EngineError::Backend)?;
            self.state.lifecycle = EngineLifecycle::Registering;
        }
        Ok(())
    }

    pub fn max_channels(&self) -> usize {
        self.max_channels
    }

    pub fn register_channel(
        &mut self,
        channel_index: usize,
        channel: ChannelState,
    ) -> Result<(), EngineError> {
        if !self.state.is_registering() {
            debug_assert!(
                false,
                "register_channel called outside registration phase; typestate should prevent this"
            );
            return Err(EngineError::Backend(BackendError::InvalidBinding));
        }
        let spec = BackendChannelSpec {
            channel: channel.backend_channel().as_u8(),
            pixels: u16::try_from(channel.len_pixels())
                .map_err(|_| EngineError::ChannelOutOfRange)?,
            layout: channel.layout(),
        };
        self.backend
            .register_channel(spec)
            .map_err(EngineError::Backend)?;
        self.channels
            .register(self.max_channels(), channel_index, channel)
    }

    pub fn finalize_configuration(&mut self) -> Result<(), EngineError> {
        if self.state.is_ready() {
            return Ok(());
        }

        self.backend
            .finalize_channels()
            .map_err(EngineError::Backend)?;
        self.state.lifecycle = EngineLifecycle::Ready(ReadyState::new());
        Ok(())
    }
}
