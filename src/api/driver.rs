use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    DRIVER_MAX_CHANNELS,
    api::backend::{BackendEvent, BackendSignal, LedBackend},
    engine::{LedEngine, registration::ChannelState},
    host::DriverHostIngress,
    pack::{ActiveSpatialQuantizer, ActiveTemporalDither},
};

use super::{
    error_map::{
        map_driver_init_error, map_finalize_error, map_register_bind_error, map_runtime_commit_error,
        map_runtime_mark_written_error, map_runtime_service_error, map_runtime_write_pack_error,
        map_runtime_write_prepare_error,
    },
    types::{
        Channel, ConfiguredChannels, DriverInitError, FinalizeError, PreparedSetup, RegisterError,
        Rgb24, RuntimeError,
    },
};

static NEXT_DRIVER_ID: AtomicU32 = AtomicU32::new(1);

fn allocate_driver_id() -> u32 {
    let id = NEXT_DRIVER_ID.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
        NEXT_DRIVER_ID.store(1, Ordering::Relaxed);
        1
    } else {
        id
    }
}

pub struct Configuring;
pub struct Ready;

pub struct Driver<B, S = Configuring>
where
    B: LedBackend,
{
    pub(super) driver_id: u32,
    pub(super) engine: LedEngine<B>,
    pub(super) _state: PhantomData<S>,
}

pub struct ChannelWriter<'a, B>
where
    B: LedBackend,
{
    driver: &'a mut Driver<B, Ready>,
    channel_index: usize,
}

impl<B> Driver<B, Configuring>
where
    B: LedBackend,
{
    pub fn new(backend: B) -> Result<Self, DriverInitError> {
        let mut engine = LedEngine::new(backend);
        engine.init().map_err(map_driver_init_error)?;
        Ok(Self {
            driver_id: allocate_driver_id(),
            engine,
            _state: PhantomData,
        })
    }

    pub fn configure_prepared(
        &mut self,
        setup: &PreparedSetup,
    ) -> Result<ConfiguredChannels, RegisterError> {
        let mut handles = [None; DRIVER_MAX_CHANNELS];
        for (logical_channel, backend_channel, pixels, layout) in setup.iter() {
            let channel_index = logical_channel.as_index();
            let handle_index =
                u8::try_from(channel_index).map_err(|_| RegisterError::InvalidBinding)?;
            let channel = ChannelState::new(backend_channel, pixels as usize, layout);
            self.engine
                .register_channel(channel_index, channel)
                .map_err(map_register_bind_error)?;

            if channel_index >= handles.len() {
                return Err(RegisterError::InvalidBinding);
            }
            handles[channel_index] = Some(Channel::new(self.driver_id, handle_index));
        }

        Ok(ConfiguredChannels::from_entries(handles))
    }

    pub fn finalize(mut self) -> Result<Driver<B, Ready>, FinalizeError> {
        self.engine
            .finalize_configuration()
            .map_err(map_finalize_error)?;

        Ok(Driver::<B, Ready> {
            driver_id: self.driver_id,
            engine: self.engine,
            _state: PhantomData,
        })
    }
}

impl<B> Driver<B, Ready>
where
    B: LedBackend,
{
    pub fn channel<'a>(
        &'a mut self,
        channel: Channel,
    ) -> Result<ChannelWriter<'a, B>, RuntimeError> {
        if channel.owner() != self.driver_id {
            return Err(RuntimeError::InvalidChannel);
        }
        let channel_index = channel.as_index();
        Ok(ChannelWriter {
            driver: self,
            channel_index,
        })
    }

    pub fn commit(&mut self) -> Result<(), RuntimeError> {
        self.engine.submit_dirty().map_err(map_runtime_commit_error)
    }

    pub fn service(&mut self) -> Result<(), RuntimeError> {
        self.engine.service().map_err(map_runtime_service_error)
    }
}

impl<'a, B> ChannelWriter<'a, B>
where
    B: LedBackend,
{
    pub fn write_rgb24(&mut self, pixels: &[Rgb24]) -> Result<(), RuntimeError> {
        let plan = self
            .driver
            .engine
            .prepare_channel_write(self.channel_index)
            .map_err(map_runtime_write_prepare_error)?;

        LedEngine::<B>::write_slice_to_plan::<Rgb24, ActiveTemporalDither, ActiveSpatialQuantizer>(
            pixels,
            plan.frame_phase,
            plan,
        )
        .map_err(map_runtime_write_pack_error)?;

        self.driver
            .engine
            .mark_channel_written(self.channel_index)
            .map_err(map_runtime_mark_written_error)
    }
}

impl<B> DriverHostIngress for Driver<B, Ready>
where
    B: LedBackend,
{
    fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.engine.on_backend_signal(signal);
    }

    fn on_backend_event(&mut self, event: BackendEvent) {
        self.engine.on_backend_event(event);
    }
}
