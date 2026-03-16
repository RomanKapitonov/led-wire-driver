//! Runtime progression for the engine state machine.
//!
//! Snapshot transfer invariants:
//! - transfer enters `InFlight` only after `StartTransfer::Started`
//! - busy start keeps pending channels intact for retry
//! - completion meaning is backend-owned; engine only reacts to
//!   `BackendEvent::TransferComplete`

use super::{
    EngineError, LedEngine,
    types::{ChannelMask, WireSpan, WireTarget, WritePlan},
};
use crate::DRIVER_MAX_CHANNELS;
use crate::api::backend::{
    AcquireWrite, BackendError, BackendEvent, BackendSignal, LedBackend, StartTransfer,
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
        submitted_mask: ChannelMask,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct ReadyState {
    pub transfer: TransferState,
    pub prepared_tokens: [Option<u16>; DRIVER_MAX_CHANNELS],
    pub dirty_mask: ChannelMask,
    pub pending_mask: ChannelMask,
}

impl ReadyState {
    pub const fn new() -> Self {
        Self {
            transfer: TransferState::Idle,
            prepared_tokens: [None; DRIVER_MAX_CHANNELS],
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
            _ => Err(EngineError::Backend(BackendError::InvalidBinding)),
        }
    }

    pub fn ready_mut(&mut self) -> Result<&mut ReadyState, EngineError> {
        match &mut self.lifecycle {
            EngineLifecycle::Ready(state) => Ok(state),
            _ => Err(EngineError::Backend(BackendError::InvalidBinding)),
        }
    }
}

impl<B> LedEngine<B>
where
    B: LedBackend,
{
    pub fn prepare_channel_write(
        &mut self,
        channel_index: usize,
    ) -> Result<WritePlan, EngineError> {
        let channel = self.channels.record(channel_index, self.max_channels())?;
        let grant = self
            .backend
            .acquire_write_target(channel.backend_channel().as_u8())
            .map_err(EngineError::Backend)?;

        let grant = match grant {
            AcquireWrite::Ready(grant) => grant,
            AcquireWrite::Busy => return Err(EngineError::WriteBusy),
        };

        if grant.channel != channel.backend_channel().as_u8() {
            return Err(EngineError::Backend(BackendError::InvalidBinding));
        }
        let expected_wire_bytes = (channel.len_pixels() as u32)
            .checked_mul(3)
            .ok_or(EngineError::Backend(BackendError::InvalidBinding))?;
        if grant.len != expected_wire_bytes {
            return Err(EngineError::Backend(BackendError::InvalidBinding));
        }

        let span = WireSpan {
            addr: grant.ptr as usize,
            size_bytes: grant.len,
        };
        let target = WireTarget::from_span(span)?;

        let ready = self.state.ready_mut()?;
        if channel_index >= ready.prepared_tokens.len() {
            return Err(EngineError::ChannelOutOfRange);
        }
        ready.prepared_tokens[channel_index] = Some(grant.token);

        Ok(WritePlan {
            layout: channel.layout(),
            frame_phase: channel.frame_phase(),
            target,
        })
    }

    pub fn mark_channel_written(&mut self, channel_index: usize) -> Result<(), EngineError> {
        self.channels.record(channel_index, self.max_channels())?;

        let token = {
            let ready = self.state.ready()?;
            if channel_index >= ready.prepared_tokens.len() {
                return Err(EngineError::ChannelOutOfRange);
            }
            ready.prepared_tokens[channel_index]
                .ok_or(EngineError::Backend(BackendError::InvalidBinding))?
        };

        self.backend
            .publish_write(token)
            .map_err(EngineError::Backend)?;

        let max_channels = self.max_channels();
        let ready = self.state.ready_mut()?;
        ready.prepared_tokens[channel_index] = None;
        ready.dirty_mask =
            self.channels
                .mark_written(channel_index, max_channels, ready.dirty_mask)?;
        Ok(())
    }

    pub fn submit_dirty(&mut self) -> Result<(), EngineError> {
        let max_channels = self.max_channels();
        let dirty = self.state.ready()?.dirty_mask;
        if dirty.is_empty() {
            return Ok(());
        }

        {
            let ready = self.state.ready_mut()?;
            ready.pending_mask = ChannelMask::from_bits(ready.pending_mask.bits() | dirty.bits());
            ready.dirty_mask = ChannelMask::ZERO;
        }

        for channel_index in 0..max_channels {
            let is_dirty = (dirty.bits() & (1u32 << channel_index)) != 0;
            if is_dirty {
                self.channels
                    .advance_phase_if_dirty(channel_index, max_channels, true)?;
            }
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
                submitted_mask: _,
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
                let ready = match self.state.ready_mut() {
                    Ok(ready) => ready,
                    Err(err) => {
                        debug_assert!(false, "submit start observed with non-ready engine state");
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
            StartTransfer::Busy => {}
        }
        Ok(())
    }
}
