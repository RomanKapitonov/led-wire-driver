pub(in crate::driver) mod channel_state;
mod error;
pub(in crate::driver) mod mask;
mod prepared_write;
pub(in crate::driver) mod registration;
pub(in crate::driver) mod runtime;

use channel_state::ChannelState;
pub(in crate::driver) use error::{BackendContractViolation, EngineError};
use mask::ChannelMask;
use prepared_write::PreparedWrite;
use registration::RegistrationTable;
use runtime::ReadyState;

use crate::{
    DRIVER_MAX_CHANNELS,
    backend::{AcquireWrite, BackendWriteLease, LedBackend},
};

const _: () = assert!(DRIVER_MAX_CHANNELS <= ChannelMask::CAPACITY_BITS);

pub(in crate::driver) struct LedEngine<B: LedBackend> {
    pub(in crate::driver) backend: B,
    pub(in crate::driver) max_channels: usize,
    pub(in crate::driver) channels: RegistrationTable,
    pub(in crate::driver) ready: ReadyState,
}

impl<B: LedBackend> LedEngine<B> {
    pub(in crate::driver) fn new(
        backend: B,
        max_channels: usize,
        channels: RegistrationTable,
    ) -> Self {
        Self {
            backend,
            max_channels,
            channels,
            ready: ReadyState::new(),
        }
    }

    pub(in crate::driver) fn acquire_prepared_write(
        &mut self,
        channel_index: usize,
    ) -> Result<PreparedWrite<'_, B>, EngineError> {
        self.surface_latched_violation()?;
        let channel = self.channels.record(channel_index)?;
        let (backend_channel, wire_byte_count, layout, frame_phase) = (
            channel.backend_channel,
            channel.wire_byte_count,
            channel.layout,
            channel.frame_phase,
        );

        let lease = self
            .backend
            .acquire_write_target(backend_channel)
            .map_err(EngineError::Backend)?;

        let mut lease = match lease {
            AcquireWrite::Ready(l) => l,
            AcquireWrite::Busy => return Err(EngineError::WriteBusy),
        };

        if lease.channel() != backend_channel {
            return Err(EngineError::BackendContractViolation(
                BackendContractViolation::WrongChannelReturned,
            ));
        }
        if lease.bytes_mut().len() != wire_byte_count {
            return Err(EngineError::BackendContractViolation(
                BackendContractViolation::WrongTargetLength,
            ));
        }

        Ok(PreparedWrite {
            layout,
            frame_phase,
            lease,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DRIVER_MAX_CHANNELS,
        backend::{BackendCapabilities, BackendWriteLease},
        model::{BackendChannelId, PixelLayout, Rgb48},
        test_support::FakeBackend,
    };

    fn make_engine(pixel_count: u16) -> LedEngine<FakeBackend> {
        use crate::backend::{BackendChannelSpec, LedBackend as _};
        let (mut backend, _handle) = FakeBackend::new(BackendCapabilities {
            max_channels: DRIVER_MAX_CHANNELS,
            max_bytes_per_channel: None,
        });
        backend.init().unwrap();
        backend
            .configure_channels(&[BackendChannelSpec {
                channel: BackendChannelId::new(0),
                pixels: pixel_count,
                layout: PixelLayout::Grb,
            }])
            .unwrap();
        let mut records = [const { None }; DRIVER_MAX_CHANNELS];
        records[0] = Some(channel_state::ChannelState::new(
            BackendChannelId::new(0),
            pixel_count,
            PixelLayout::Grb,
        ));
        let table = RegistrationTable::new(records, 1);
        LedEngine::new(backend, DRIVER_MAX_CHANNELS, table)
    }

    #[test]
    fn write_and_publish_marks_dirty() {
        let mut engine = make_engine(1);
        let colors = [Rgb48 {
            r: 65535,
            g: 0,
            b: 0,
        }];
        {
            let mut pw = engine.acquire_prepared_write(0).unwrap();
            pw.pack_rgb48_active(&colors).unwrap();
            pw.publish().unwrap();
        }
        engine.mark_channel_published(0).unwrap();
        assert!(!engine.ready.dirty_mask.is_empty());
    }

    #[test]
    fn submit_dirty_promotes_to_pending() {
        let mut engine = make_engine(1);
        let colors = [Rgb48 {
            r: 0,
            g: 0,
            b: 65535,
        }];
        {
            let mut pw = engine.acquire_prepared_write(0).unwrap();
            pw.pack_rgb48_active(&colors).unwrap();
            pw.publish().unwrap();
        }
        engine.mark_channel_published(0).unwrap();
        engine.submit_dirty().unwrap();
        assert!(engine.ready.dirty_mask.is_empty());
        assert!(!engine.ready.pending_mask.is_empty());
    }

    #[test]
    fn service_starts_transfer_when_pending() {
        let mut engine = make_engine(2);
        let colors = [Rgb48 {
            r: 100,
            g: 200,
            b: 0,
        }; 2];
        {
            let mut pw = engine.acquire_prepared_write(0).unwrap();
            pw.pack_rgb48_active(&colors).unwrap();
            pw.publish().unwrap();
        }
        engine.mark_channel_published(0).unwrap();
        engine.submit_dirty().unwrap();
        engine.service().unwrap();
        assert!(matches!(
            engine.ready.transfer,
            runtime::TransferState::InFlight { .. }
        ));
    }
}
