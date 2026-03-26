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
//!   first, with explicit `EngineError::InvalidState(...)` retained as an
//!   internal backstop for misused engine entry points
//! - backend resource/wire target validity is enforced by backend
//!
//! Engine invariants:
//! - `InFlight` is entered only after backend returns `StartTransfer::Started`
//! - busy start does not clear pending submissions
//! - driver runtime never owns transport slot/buffer strategy
//! - channel phase is intended to advance only for submission batches the
//!   backend actually accepts; runtime code is responsible for enforcing that
//!   accepted-submit rule

mod error;
mod mask;
mod prepared_write;
pub(crate) mod registration;
mod runtime;

pub use error::{BackendContractViolation, EngineError, EngineStateExpectation};

use self::{
    mask::ChannelMask,
    registration::{RegistrationPlan, RegistrationTable},
    runtime::{EngineLifecycle, EngineState, ReadyState},
};
use crate::{
    DRIVER_MAX_CHANNELS,
    api::{backend::LedBackend, types::PreparedSetup},
};

pub struct LedEngine<B>
where
    B: LedBackend,
{
    backend: B,
    max_channels: usize,
    max_bytes_per_channel: Option<u32>,
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
        let capabilities = backend.capabilities();
        let max_channels = capabilities.max_channels.min(DRIVER_MAX_CHANNELS);
        Self {
            backend,
            max_channels,
            max_bytes_per_channel: capabilities.max_bytes_per_channel,
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

    pub(crate) fn is_configuration_committed(&self) -> bool {
        self.state.is_ready()
    }

    pub(crate) fn build_registration_plan(
        &self,
        setup: &PreparedSetup,
        driver_id: u32,
    ) -> Result<RegistrationPlan, EngineError> {
        if !self.state.is_registering() {
            debug_assert!(
                false,
                "build_registration_plan called outside registration phase; typestate should prevent this"
            );
            return Err(EngineError::InvalidState(
                EngineStateExpectation::MustBeRegistering,
            ));
        }

        RegistrationPlan::from_prepared_setup(
            setup,
            driver_id,
            self.max_channels(),
            self.max_bytes_per_channel,
        )
    }

    pub(crate) fn apply_registration_plan(
        &mut self,
        plan: &RegistrationPlan,
    ) -> Result<(), EngineError> {
        if !self.state.is_registering() {
            debug_assert!(
                false,
                "apply_registration_plan called outside registration phase; typestate should prevent this"
            );
            return Err(EngineError::InvalidState(
                EngineStateExpectation::MustBeRegistering,
            ));
        }

        self.backend
            .configure_channels(plan.specs())
            .map_err(EngineError::Backend)?;

        self.channels.commit_plan(self.max_channels(), plan)?;

        Ok(())
    }

    pub(crate) fn enter_ready_state(&mut self) {
        if self.state.is_ready() {
            return;
        }

        self.state.lifecycle = EngineLifecycle::Ready(ReadyState::new());
    }
}
