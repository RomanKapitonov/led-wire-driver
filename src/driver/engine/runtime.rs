use super::{EngineError, LedEngine, error::BackendContractViolation, mask::ChannelMask};
use crate::backend::{BackendEvent, BackendSignal, LedBackend, StartTransfer};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) enum TransferState {
    Idle,
    InFlight {
        dma_complete_pending: bool,
        submitted_mask: ChannelMask,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) struct ReadyState {
    pub(in crate::driver) transfer: TransferState,
    pub(in crate::driver) dirty_mask: ChannelMask,
    pub(in crate::driver) pending_mask: ChannelMask,
    pub(in crate::driver) ingress_violation: bool,
}

impl ReadyState {
    pub(in crate::driver) const fn new() -> Self {
        Self {
            transfer: TransferState::Idle,
            dirty_mask: ChannelMask::EMPTY,
            pending_mask: ChannelMask::EMPTY,
            ingress_violation: false,
        }
    }

    fn take_ingress_violation(&mut self) -> Option<BackendContractViolation> {
        if self.ingress_violation {
            self.ingress_violation = false;
            Some(BackendContractViolation::TransferCompleteWhileIdle)
        } else {
            None
        }
    }
}

impl<B: LedBackend> LedEngine<B> {
    pub(in crate::driver) fn surface_latched_violation(&mut self) -> Result<(), EngineError> {
        if let Some(violation) = self.ready.take_ingress_violation() {
            return Err(EngineError::BackendContractViolation(violation));
        }
        Ok(())
    }

    pub(in crate::driver) fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.backend.on_signal(signal);
    }

    pub(in crate::driver) fn on_backend_event(&mut self, event: BackendEvent) {
        self.backend.on_event(event);
        match event {
            BackendEvent::TransferComplete => match &mut self.ready.transfer {
                TransferState::InFlight {
                    dma_complete_pending,
                    ..
                } => {
                    *dma_complete_pending = true;
                }
                TransferState::Idle => {
                    self.ready.ingress_violation = true;
                }
            },
        }
    }

    pub(in crate::driver) fn mark_channel_published(
        &mut self,
        channel_index: usize,
    ) -> Result<(), EngineError> {
        self.surface_latched_violation()?;
        self.channels.record(channel_index)?;
        self.ready.dirty_mask = self
            .ready
            .dirty_mask
            .union(ChannelMask::single(channel_index));
        Ok(())
    }

    pub(in crate::driver) fn submit_dirty(&mut self) -> Result<(), EngineError> {
        self.surface_latched_violation()?;
        let dirty = self.ready.dirty_mask;
        if dirty.is_empty() {
            return Ok(());
        }
        self.ready.pending_mask = self.ready.pending_mask.union(dirty);
        self.ready.dirty_mask = ChannelMask::EMPTY;
        Ok(())
    }

    pub(in crate::driver) fn service(&mut self) -> Result<(), EngineError> {
        self.surface_latched_violation()?;
        self.complete_in_flight_if_ready();
        self.try_start_pending_submit()
    }

    fn complete_in_flight_if_ready(&mut self) {
        if let TransferState::InFlight {
            dma_complete_pending: true,
            ..
        } = self.ready.transfer
        {
            self.ready.transfer = TransferState::Idle;
        }
    }

    fn compute_pending_batch(&self) -> Option<ChannelMask> {
        if !matches!(self.ready.transfer, TransferState::Idle) {
            return None;
        }
        if self.ready.pending_mask.is_empty() {
            return None;
        }
        let valid = ChannelMask::from_bits(
            self.ready.pending_mask.bits() & self.channels.registered_mask().bits(),
        );
        if valid.is_empty() { None } else { Some(valid) }
    }

    fn on_submit_started(&mut self, submitted: ChannelMask) -> Result<(), EngineError> {
        self.ready.transfer = TransferState::InFlight {
            dma_complete_pending: false,
            submitted_mask: submitted,
        };
        self.ready.pending_mask = self.ready.pending_mask.exclude(submitted);
        for idx in 0..self.max_channels {
            if submitted.contains(idx) {
                self.channels.record_mut(idx)?.advance_phase();
            }
        }
        Ok(())
    }

    fn try_start_pending_submit(&mut self) -> Result<(), EngineError> {
        let Some(pending) = self.compute_pending_batch() else {
            return Ok(());
        };
        match self
            .backend
            .submit_channels(pending.bits())
            .map_err(EngineError::Backend)?
        {
            StartTransfer::Started => self.on_submit_started(pending),
            StartTransfer::Busy => Ok(()),
        }
    }
}
