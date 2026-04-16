//! Runtime progression for the engine state machine.
//!
//! Snapshot transfer invariants:
//! - transfer enters `InFlight` only after `StartTransfer::Started`
//! - busy start keeps pending channels intact for retry
//! - channel phase advances only for accepted submission batches
//! - completion meaning is backend-owned; engine only reacts to
//!   `BackendEvent::TransferComplete`
//! - invalid ingress timing is tracked explicitly instead of being
//!   debug-only release behavior
//! - backend contract mismatches are reported distinctly from transport faults
//!
//! State machine shape:
//! - published writes first mark channels dirty
//! - `submit_dirty()` promotes `dirty_mask -> pending_mask` without touching
//!   transfer ownership or frame phase
//! - `service()` is responsible for trying transport submission from idle
//! - frame phase advances only when backend transport accepts the exact pending
//!   batch via `StartTransfer::Started`
//! - completion later returns that accepted batch from `InFlight` to `Idle`

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
pub(crate) enum TransferState {
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
pub(crate) struct ReadyState {
    /// Transfer ownership currently held by backend transport, if any.
    pub transfer: TransferState,
    /// Channels published since the last commit promotion.
    pub dirty_mask: ChannelMask,
    /// Channels ready to retry until transport accepts them.
    pub pending_mask: ChannelMask,
    /// Latched ingress contract violation to surface on the next runtime call.
    pub ingress_violation: bool,
}

impl ReadyState {
    pub const fn new() -> Self {
        Self {
            transfer: TransferState::Idle,
            dirty_mask: ChannelMask::ZERO,
            pending_mask: ChannelMask::ZERO,
            ingress_violation: false,
        }
    }

    fn take_ingress_violation(&mut self) -> Option<super::BackendContractViolation> {
        if self.ingress_violation {
            self.ingress_violation = false;
            Some(super::BackendContractViolation::TransferCompleteWhileIdle)
        } else {
            None
        }
    }
}


impl<B> LedEngine<B>
where
    B: LedBackend,
{
    /// Surfaces one latched ingress violation on a normal runtime boundary.
    ///
    /// Ingress hooks stay infallible so firmware/IRQ glue can forward backend
    /// signals/events directly. Contract violations are therefore retained in
    /// ready-state and surfaced once on the next runtime operation.
    fn surface_latched_ingress_violation(&mut self) -> Result<(), EngineError> {
        let ready = match self.ready_mut() {
            Ok(ready) => ready,
            Err(_) => return Ok(()),
        };

        if let Some(violation) = ready.take_ingress_violation() {
            return Err(EngineError::BackendContractViolation(violation));
        }

        Ok(())
    }

    pub fn acquire_prepared_write(
        &mut self,
        channel_index: usize,
    ) -> Result<PreparedWrite<'_, B>, EngineError> {
        self.surface_latched_ingress_violation()?;
        let channel = self.channels.record(channel_index, self.max_channels())?;
        let lease = self
            .backend
            .acquire_write_target(channel.backend_channel)
            .map_err(EngineError::Backend)?;

        let mut lease = match lease {
            AcquireWrite::Ready(lease) => lease,
            AcquireWrite::Busy => return Err(EngineError::WriteBusy),
        };

        if lease.channel() != channel.backend_channel {
            return Err(EngineError::BackendContractViolation(
                super::BackendContractViolation::WrongChannelReturned,
            ));
        }
        let expected_wire_bytes = (channel.len_pixels as u32)
            .checked_mul(3)
            .ok_or(EngineError::ConfigurationLimitExceeded)?;
        if lease.bytes_mut().len() != expected_wire_bytes as usize {
            return Err(EngineError::BackendContractViolation(
                super::BackendContractViolation::WrongTargetLength,
            ));
        }

        Ok(PreparedWrite {
            layout: channel.layout,
            frame_phase: channel.frame_phase,
            lease,
        })
    }

    pub fn mark_channel_published(&mut self, channel_index: usize) -> Result<(), EngineError> {
        self.surface_latched_ingress_violation()?;
        self.channels.record(channel_index, self.max_channels())?;

        let max_channels = self.max_channels();
        let current_dirty = self.ready()?.dirty_mask;
        let new_dirty = self
            .channels
            .mark_written(channel_index, max_channels, current_dirty)?;
        self.ready_mut()?.dirty_mask = new_dirty;
        Ok(())
    }

    pub fn submit_dirty(&mut self) -> Result<(), EngineError> {
        self.surface_latched_ingress_violation()?;
        let dirty = self.ready()?.dirty_mask;
        if dirty.is_empty() {
            return Ok(());
        }

        {
            let ready = self.ready_mut()?;
            ready.pending_mask = ChannelMask::from_bits(ready.pending_mask.bits() | dirty.bits());
            ready.dirty_mask = ChannelMask::ZERO;
        }
        Ok(())
    }

    pub fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.backend.on_signal(signal);
    }

    /// Records backend event ingress against the current ready-state snapshot.
    ///
    /// `TransferComplete` is strict:
    /// - while `InFlight`, it marks completion pending for later `service()`
    /// - while idle, it latches a backend-contract violation for one-shot
    ///   surfacing on the next runtime call
    pub fn on_backend_event(&mut self, event: BackendEvent) {
        self.backend.on_event(event);
        if let Ok(ready) = self.ready_mut() {
            match event {
                BackendEvent::TransferComplete => {
                    if let TransferState::InFlight {
                        dma_complete_pending,
                        ..
                    } = &mut ready.transfer
                    {
                        *dma_complete_pending = true;
                    } else {
                        ready.ingress_violation = true;
                    }
                }
            }
        }
    }

    pub fn service(&mut self) -> Result<(), EngineError> {
        self.surface_latched_ingress_violation()?;
        // Phase 1: clear any completed in-flight transfer before initiating new work.
        self.complete_in_flight_if_ready()?;
        self.try_start_pending_submit()
    }

    /// Clears a completed in-flight transfer so the engine returns to idle.
    ///
    /// Returns `Ok(())` immediately if the engine is not in the ready state.
    /// Does nothing if no transfer is pending completion.
    fn complete_in_flight_if_ready(&mut self) -> Result<(), EngineError> {
        let ready = match self.ready_mut() {
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
        Ok(())
    }

    /// Starts pending submission only from idle and only for the exact
    /// currently pending batch.
    ///
    /// Accepted-submit semantics live here:
    /// - `Busy` leaves the pending batch intact
    /// - `Started` advances phase for the submitted batch, then transfers that
    ///   exact batch into `InFlight`
    fn try_start_pending_submit(&mut self) -> Result<(), EngineError> {
        let max_channels = self.max_channels();
        let ready = match self.ready() {
            Ok(ready) => ready,
            Err(err) => {
                debug_assert!(
                    self.is_ready(),
                    "try_start_pending_submit reached with non-ready engine state"
                );
                return Err(err);
            }
        };

        if !matches!(ready.transfer, TransferState::Idle) {
            return Ok(());
        }

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
                    let ready = match self.ready_mut() {
                        Ok(ready) => ready,
                        Err(err) => {
                            debug_assert!(
                                self.is_ready(),
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
