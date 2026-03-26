//! Runtime progression for the engine state machine.
//!
//! Snapshot transfer invariants:
//! - transfer enters `InFlight` only after `StartTransfer::Started`
//! - busy start keeps pending channels intact for retry
//! - channel phase advances only for accepted submission batches
//! - completion meaning is backend-owned; engine only reacts to
//!   `BackendEvent::TransferComplete`
//! - backend contract mismatches are reported distinctly from transport faults

use super::{EngineError, LedEngine, mask::ChannelMask, prepared_write::PreparedWrite};
use crate::api::backend::{
    AcquireWrite, BackendEvent, BackendSignal, BackendWriteLease, LedBackend, StartTransfer,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum EngineLifecycle {
    Uninitialized,
    Registering,
    Ready(ReadyState),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum TransferState {
    /// No transfer currently accepted by backend transport.
    Idle,
    /// Backend accepted transfer start and completion is pending.
    InFlight {
        dma_complete_pending: bool,
        /// Exact accepted channel batch currently owned by transport.
        submitted_mask: ChannelMask,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct ReadyState {
    pub transfer: TransferState,
    pub dirty_mask: ChannelMask,
    pub pending_mask: ChannelMask,
}

impl ReadyState {
    pub const fn new() -> Self {
        Self {
            transfer: TransferState::Idle,
            dirty_mask: ChannelMask::ZERO,
            pending_mask: ChannelMask::ZERO,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct EngineState {
    pub lifecycle: EngineLifecycle,
}

impl EngineState {
    pub const fn new() -> Self {
        Self {
            lifecycle: EngineLifecycle::Uninitialized,
        }
    }

    pub const fn is_registering(&self) -> bool {
        matches!(self.lifecycle, EngineLifecycle::Registering)
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self.lifecycle, EngineLifecycle::Ready(_))
    }

    pub fn ready(&self) -> Result<&ReadyState, EngineError> {
        match &self.lifecycle {
            EngineLifecycle::Ready(state) => Ok(state),
            _ => Err(EngineError::InvalidState(
                super::EngineStateExpectation::MustBeReady,
            )),
        }
    }

    pub fn ready_mut(&mut self) -> Result<&mut ReadyState, EngineError> {
        match &mut self.lifecycle {
            EngineLifecycle::Ready(state) => Ok(state),
            _ => Err(EngineError::InvalidState(
                super::EngineStateExpectation::MustBeReady,
            )),
        }
    }
}

impl<B> LedEngine<B>
where
    B: LedBackend,
{
    pub fn acquire_prepared_write(
        &mut self,
        channel_index: usize,
    ) -> Result<PreparedWrite<'_, B>, EngineError> {
        let channel = self.channels.record(channel_index, self.max_channels())?;
        let lease = self
            .backend
            .acquire_write_target(channel.backend_channel())
            .map_err(EngineError::Backend)?;

        let mut lease = match lease {
            AcquireWrite::Ready(lease) => lease,
            AcquireWrite::Busy => return Err(EngineError::WriteBusy),
        };

        if lease.channel() != channel.backend_channel() {
            return Err(EngineError::BackendContractViolation(
                super::BackendContractViolation::WrongChannelReturned,
            ));
        }
        let expected_wire_bytes = (channel.len_pixels() as u32)
            .checked_mul(3)
            .ok_or(EngineError::ConfigurationLimitExceeded)?;
        if lease.bytes_mut().len() != expected_wire_bytes as usize {
            return Err(EngineError::BackendContractViolation(
                super::BackendContractViolation::WrongTargetLength,
            ));
        }

        Ok(PreparedWrite {
            layout: channel.layout(),
            frame_phase: channel.frame_phase(),
            lease,
        })
    }

    pub fn mark_channel_published(&mut self, channel_index: usize) -> Result<(), EngineError> {
        self.channels.record(channel_index, self.max_channels())?;

        let max_channels = self.max_channels();
        let ready = self.state.ready_mut()?;
        ready.dirty_mask =
            self.channels
                .mark_written(channel_index, max_channels, ready.dirty_mask)?;
        Ok(())
    }

    pub fn submit_dirty(&mut self) -> Result<(), EngineError> {
        let dirty = self.state.ready()?.dirty_mask;
        if dirty.is_empty() {
            return Ok(());
        }

        {
            let ready = self.state.ready_mut()?;
            ready.pending_mask = ChannelMask::from_bits(ready.pending_mask.bits() | dirty.bits());
            ready.dirty_mask = ChannelMask::ZERO;
        }
        Ok(())
    }

    pub fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.backend.on_signal(signal);
    }

    pub fn on_backend_event(&mut self, event: BackendEvent) {
        self.backend.on_event(event);
        if let Ok(ready) = self.state.ready_mut() {
            match event {
                BackendEvent::TransferComplete => {
                    if let TransferState::InFlight {
                        dma_complete_pending,
                        ..
                    } = &mut ready.transfer
                    {
                        *dma_complete_pending = true;
                    } else {
                        debug_assert!(
                            false,
                            "received transfer-complete event while no transfer is in flight"
                        );
                    }
                }
            }
        }
    }

    pub fn service(&mut self) -> Result<(), EngineError> {
        {
            let ready = match self.state.ready_mut() {
                Ok(ready) => ready,
                Err(_) => return Ok(()),
            };
            if let TransferState::InFlight {
                dma_complete_pending: true,
                submitted_mask: _submitted_mask,
            } = ready.transfer
            {
                ready.transfer = TransferState::Idle;
            }
        }

        {
            let ready = match self.state.ready() {
                Ok(ready) => ready,
                Err(err) => {
                    debug_assert!(
                        false,
                        "service transfer check reached with non-ready engine state"
                    );
                    return Err(err);
                }
            };
            if !matches!(ready.transfer, TransferState::Idle) {
                return Ok(());
            }
        }

        self.try_start_pending_submit()
    }

    fn try_start_pending_submit(&mut self) -> Result<(), EngineError> {
        let max_channels = self.max_channels();
        let ready = match self.state.ready() {
            Ok(ready) => ready,
            Err(err) => {
                debug_assert!(
                    false,
                    "try_start_pending_submit reached with non-ready engine state"
                );
                return Err(err);
            }
        };

        if ready.pending_mask.is_empty() {
            return Ok(());
        }

        let pending_mask = ChannelMask::from_bits(
            ready.pending_mask.bits() & self.channels.registered_channel_mask(max_channels)?.bits(),
        );
        if pending_mask.is_empty() {
            return Ok(());
        }

        match self
            .backend
            .submit_channels(pending_mask.bits())
            .map_err(EngineError::Backend)?
        {
            StartTransfer::Started => {
                {
                    let ready = match self.state.ready_mut() {
                        Ok(ready) => ready,
                        Err(err) => {
                            debug_assert!(
                                false,
                                "submit start observed with non-ready engine state"
                            );
                            return Err(err);
                        }
                    };
                    ready.transfer = TransferState::InFlight {
                        dma_complete_pending: false,
                        submitted_mask: pending_mask,
                    };
                    ready.pending_mask =
                        ChannelMask::from_bits(ready.pending_mask.bits() & !pending_mask.bits());
                }

                for channel_index in 0..max_channels {
                    let is_submitted = (pending_mask.bits() & (1u32 << channel_index)) != 0;
                    if is_submitted {
                        self.channels.advance_phase(channel_index, max_channels)?;
                    }
                }
            }
            StartTransfer::Busy => {}
        }
        Ok(())
    }
}
