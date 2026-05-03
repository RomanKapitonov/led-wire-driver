mod engine;
pub mod pack;

use core::sync::atomic::{AtomicU32, Ordering};

use engine::{
    EngineError, LedEngine, channel_state::ChannelState, registration::RegistrationTable,
};

use crate::{
    DRIVER_MAX_CHANNELS,
    backend::{BackendChannelSpec, BackendEvent, BackendSignal, LedBackend},
    error::{ConfigureError, ServiceError, WriteError},
    model::{BackendChannelId, Rgb48},
    setup::ChannelSetup,
};

// ── DriverId ──────────────────────────────────────────────────────────────────

static NEXT_DRIVER_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct DriverId(u32);

impl DriverId {
    fn allocate() -> Self {
        let mut current = NEXT_DRIVER_ID.load(Ordering::Relaxed);
        loop {
            let raw = if current == 0 { 1 } else { current };
            let next = raw.checked_add(1).unwrap_or(1);
            match NEXT_DRIVER_ID.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Self(raw),
                Err(observed) => {
                    core::hint::spin_loop();
                    current = observed;
                }
            }
        }
    }
}

// ── ChannelHandle + ChannelHandles ────────────────────────────────────────────

#[derive(Copy, Clone, Debug)]
pub struct ChannelHandle {
    driver_id: DriverId,
    channel_index: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct ChannelHandles {
    handles: [Option<ChannelHandle>; DRIVER_MAX_CHANNELS],
    count: usize,
}

impl ChannelHandles {
    fn new(handles: [Option<ChannelHandle>; DRIVER_MAX_CHANNELS], count: usize) -> Self {
        Self { handles, count }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, slot: usize) -> Option<&ChannelHandle> {
        self.handles.get(slot).and_then(|h| h.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChannelHandle> {
        self.handles.iter().flatten()
    }
}

// ── ConfiguringDriver ─────────────────────────────────────────────────────────

pub struct ConfiguringDriver;

impl ConfiguringDriver {
    pub fn new() -> Self {
        Self
    }

    pub fn configure<B: LedBackend>(
        self,
        channels: &[ChannelSetup],
        mut backend: B,
    ) -> Result<(Driver<B>, ChannelHandles), ConfigureError> {
        if channels.is_empty() {
            return Err(ConfigureError::NoChannels);
        }

        backend.init().map_err(ConfigureError::Backend)?;
        let caps = backend.capabilities();
        let max_channels = caps.max_channels.min(DRIVER_MAX_CHANNELS);

        if channels.len() > max_channels {
            return Err(ConfigureError::TooManyChannels);
        }

        let mut seen = [false; BackendChannelId::CARDINALITY];
        for setup in channels {
            let idx = setup.backend_channel.as_index();
            if seen[idx] {
                return Err(ConfigureError::DuplicateBackendChannel(
                    setup.backend_channel,
                ));
            }
            seen[idx] = true;

            let wire_bytes = (setup.pixel_count as u32)
                .checked_mul(3)
                .ok_or(ConfigureError::ChannelTooLarge)?;
            if let Some(max) = caps.max_bytes_per_channel {
                if wire_bytes > max {
                    return Err(ConfigureError::ChannelTooLarge);
                }
            }
        }

        let mut specs_buf = heapless::Vec::<BackendChannelSpec, DRIVER_MAX_CHANNELS>::new();
        for setup in channels {
            specs_buf
                .push(BackendChannelSpec {
                    channel: setup.backend_channel,
                    pixels: setup.pixel_count,
                    layout: setup.layout,
                })
                .ok();
        }
        backend
            .configure_channels(specs_buf.as_slice())
            .map_err(ConfigureError::Backend)?;

        let driver_id = DriverId::allocate();
        let mut records: [Option<ChannelState>; DRIVER_MAX_CHANNELS] =
            [const { None }; DRIVER_MAX_CHANNELS];
        let mut handle_arr: [Option<ChannelHandle>; DRIVER_MAX_CHANNELS] =
            [const { None }; DRIVER_MAX_CHANNELS];

        for (slot, setup) in channels.iter().enumerate() {
            records[slot] = Some(ChannelState::new(
                setup.backend_channel,
                setup.pixel_count,
                setup.layout,
            ));
            handle_arr[slot] = Some(ChannelHandle {
                driver_id,
                channel_index: slot,
            });
        }

        let count = channels.len();
        let table = RegistrationTable::new(records, count);
        let engine = LedEngine::new(backend, max_channels, table);
        Ok((
            Driver { engine, driver_id },
            ChannelHandles::new(handle_arr, count),
        ))
    }
}

impl Default for ConfiguringDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Driver<B> ─────────────────────────────────────────────────────────────────

pub struct Driver<B: LedBackend> {
    engine: LedEngine<B>,
    driver_id: DriverId,
}

pub struct ChannelWriter<'d, B: LedBackend> {
    engine: &'d mut LedEngine<B>,
    channel_index: usize,
}

impl<B: LedBackend> Driver<B> {
    pub fn channel<'d>(
        &'d mut self,
        handle: &ChannelHandle,
    ) -> Result<ChannelWriter<'d, B>, WriteError> {
        if handle.driver_id != self.driver_id {
            return Err(WriteError::InvalidChannel);
        }
        Ok(ChannelWriter {
            engine: &mut self.engine,
            channel_index: handle.channel_index,
        })
    }

    pub fn commit(&mut self) -> Result<(), ServiceError> {
        self.engine.submit_dirty().map_err(map_service_err)
    }

    pub fn service(&mut self) -> Result<(), ServiceError> {
        self.engine.service().map_err(map_service_err)
    }

    pub fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.engine.on_backend_signal(signal);
    }

    pub fn on_backend_event(&mut self, event: BackendEvent) {
        self.engine.on_backend_event(event);
    }
}

impl<'d, B: LedBackend> ChannelWriter<'d, B> {
    pub fn write_rgb48(&mut self, pixels: &[Rgb48]) -> Result<(), WriteError> {
        let mut pw = self
            .engine
            .acquire_prepared_write(self.channel_index)
            .map_err(map_write_err)?;
        pw.pack_rgb48_active(pixels).map_err(map_write_err)?;
        pw.publish().map_err(map_write_err)?;
        drop(pw);
        self.engine
            .mark_channel_published(self.channel_index)
            .map_err(map_write_err)
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────────

fn map_write_err(e: EngineError) -> WriteError {
    match e {
        EngineError::InvalidChannel => WriteError::InvalidChannel,
        EngineError::WriteBusy => WriteError::Busy,
        EngineError::Backend(e) => WriteError::Backend(e),
        EngineError::BackendContractViolation(_) => WriteError::BackendContract,
        EngineError::SourceLengthMismatch {
            expected_pixels,
            actual_pixels,
        } => WriteError::LengthMismatch {
            expected: expected_pixels,
            actual: actual_pixels,
        },
    }
}

fn map_service_err(e: EngineError) -> ServiceError {
    match e {
        EngineError::Backend(e) => ServiceError::Backend(e),
        _ => ServiceError::BackendContract,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DRIVER_MAX_CHANNELS,
        backend::{BackendCapabilities, BackendEvent, StartTransfer},
        error::{ConfigureError, WriteError},
        model::{BackendChannelId, PixelLayout, Rgb48},
        setup::ChannelSetup,
        test_support::FakeBackend,
    };

    fn caps() -> BackendCapabilities {
        BackendCapabilities {
            max_channels: DRIVER_MAX_CHANNELS,
            max_bytes_per_channel: None,
        }
    }

    fn ch(backend: u8, pixels: u16) -> ChannelSetup {
        ChannelSetup {
            backend_channel: BackendChannelId::new(backend),
            pixel_count: pixels,
            layout: PixelLayout::Grb,
        }
    }

    #[test]
    fn configure_succeeds() {
        let (backend, _) = FakeBackend::new(caps());
        let (_driver, handles) = ConfiguringDriver::new()
            .configure(&[ch(0, 10)], backend)
            .unwrap();
        assert_eq!(handles.len(), 1);
        assert!(handles.get(0).is_some());
        assert!(handles.get(1).is_none());
    }

    #[test]
    fn configure_empty_returns_error() {
        let (backend, _) = FakeBackend::new(caps());
        let err = ConfiguringDriver::new()
            .configure(&[], backend)
            .err()
            .unwrap();
        assert_eq!(err, ConfigureError::NoChannels);
    }

    #[test]
    fn configure_duplicate_backend_channel() {
        let (backend, _) = FakeBackend::new(caps());
        let err = ConfiguringDriver::new()
            .configure(&[ch(0, 5), ch(0, 5)], backend)
            .err()
            .unwrap();
        assert_eq!(
            err,
            ConfigureError::DuplicateBackendChannel(BackendChannelId::new(0))
        );
    }

    #[test]
    fn configure_too_many_channels() {
        let (backend, _) = FakeBackend::new(BackendCapabilities {
            max_channels: 2,
            max_bytes_per_channel: None,
        });
        let err = ConfiguringDriver::new()
            .configure(&[ch(0, 1), ch(1, 1), ch(2, 1)], backend)
            .err()
            .unwrap();
        assert_eq!(err, ConfigureError::TooManyChannels);
    }

    #[test]
    fn cross_driver_handle_rejected() {
        let (b1, _) = FakeBackend::new(caps());
        let (b2, _) = FakeBackend::new(caps());
        let (mut d1, h1) = ConfiguringDriver::new().configure(&[ch(0, 1)], b1).unwrap();
        let (mut d2, _) = ConfiguringDriver::new().configure(&[ch(0, 1)], b2).unwrap();
        let handle = h1.get(0).unwrap();
        assert!(d2.channel(handle).is_err());
        assert!(d1.channel(handle).is_ok());
    }

    #[test]
    fn full_write_commit_service_cycle() {
        let (backend, bh) = FakeBackend::new(caps());
        bh.script_submit_result(Ok(StartTransfer::Started));
        let (mut driver, handles) = ConfiguringDriver::new()
            .configure(&[ch(0, 2)], backend)
            .unwrap();
        let h = *handles.get(0).unwrap();
        let colors = [Rgb48 {
            r: 65535,
            g: 0,
            b: 0,
        }; 2];
        driver.channel(&h).unwrap().write_rgb48(&colors).unwrap();
        driver.commit().unwrap();
        driver.service().unwrap();
        assert_eq!(bh.log().submit_masks, [1u32]);
        driver.on_backend_event(BackendEvent::TransferComplete);
        driver.service().unwrap();
        assert_eq!(bh.log().submit_masks.len(), 1);
    }

    #[test]
    fn write_wrong_pixel_count_returns_length_mismatch() {
        let (backend, _) = FakeBackend::new(caps());
        let (mut driver, handles) = ConfiguringDriver::new()
            .configure(&[ch(0, 10)], backend)
            .unwrap();
        let h = *handles.get(0).unwrap();
        let err = driver
            .channel(&h)
            .unwrap()
            .write_rgb48(&[Rgb48 { r: 0, g: 0, b: 0 }; 5])
            .err()
            .unwrap();
        assert!(matches!(
            err,
            WriteError::LengthMismatch {
                expected: 10,
                actual: 5
            }
        ));
    }
}
