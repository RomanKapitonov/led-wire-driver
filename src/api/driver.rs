use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering},
};

use super::{
    Rgb48,
    channel::{Channel, ConfiguredChannels, DriverId},
    error_map::{
        map_driver_init_error, map_register_bind_error, map_runtime_commit_error,
        map_runtime_mark_published_error, map_runtime_service_error, map_runtime_write_pack_error,
        map_runtime_write_prepare_error, map_runtime_write_publish_error,
    },
    errors::{DriverInitError, RegisterError, RuntimeError},
    setup::PreparedSetup,
};
use crate::{
    api::backend::{BackendEvent, BackendSignal, LedBackend},
    engine::LedEngine,
};

// Driver construction is intended to remain safe under concurrent callers.
// `DriverId(0)` is reserved, so allocation starts at 1 and should continue to
// skip zero even after wrap-around.
static NEXT_DRIVER_ID: AtomicU32 = AtomicU32::new(1);

/// Allocates one non-zero driver owner token.
///
/// Contract:
/// - concurrent callers must still receive distinct `DriverId` values,
/// - `DriverId(0)` is reserved and must never be handed out,
/// - the allocator only provides uniqueness, so `Relaxed` ordering is
///   sufficient.
fn allocate_driver_id() -> DriverId {
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
            Ok(_) => return DriverId::new(raw),
            Err(observed) => current = observed,
        }
    }
}

pub struct Configuring;
pub struct Ready;

pub struct Driver<B, S = Configuring>
where
    B: LedBackend,
{
    pub(super) driver_id: DriverId,
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

    /// Atomically applies one validated setup to the backend and local
    /// registration table.
    ///
    /// Contract:
    /// - the setup must be non-empty,
    /// - configuration is single-shot,
    /// - on success the returned handles always correspond to committed state,
    /// - on failure no caller-visible handles are produced.
    pub fn configure_prepared(
        &mut self,
        setup: &PreparedSetup,
    ) -> Result<ConfiguredChannels, RegisterError> {
        if setup.is_empty() {
            return Err(RegisterError::EmptyConfiguration);
        }
        if self.engine.is_configuration_committed() {
            return Err(RegisterError::AlreadyConfigured);
        }

        self.engine
            .configure_prepared(setup, self.driver_id)
            .map_err(map_register_bind_error)
    }

    /// Completes the configuring typestate after successful configuration.
    ///
    /// `configure_prepared(...)` performs registration commit work. This method
    /// only transitions the public driver type into `Ready`.
    pub fn finalize(self) -> Driver<B, Ready> {
        debug_assert!(
            self.engine.is_configuration_committed(),
            "finalize called before a successful configure_prepared() commit"
        );
        Driver::<B, Ready> {
            driver_id: self.driver_id,
            engine: self.engine,
            _state: PhantomData,
        }
    }
}

impl<B> Driver<B, Ready>
where
    B: LedBackend,
{
    /// Resolves one configured channel handle for runtime writes.
    ///
    /// Handles are tied to the driver instance that produced them. A handle
    /// from another driver is rejected even if its logical channel index
    /// happens to match one in this driver.
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

    pub fn on_backend_signal(&mut self, signal: BackendSignal) {
        self.engine.on_backend_signal(signal);
    }

    pub fn on_backend_event(&mut self, event: BackendEvent) {
        self.engine.on_backend_event(event);
    }
}

impl<'a, B> ChannelWriter<'a, B>
where
    B: LedBackend,
{
    pub fn write_rgb48(&mut self, pixels: &[Rgb48]) -> Result<(), RuntimeError> {
        {
            let mut write = self
                .driver
                .engine
                .acquire_prepared_write(self.channel_index)
                .map_err(map_runtime_write_prepare_error)?;

            write
                .pack_rgb48_active(pixels)
                .map_err(map_runtime_write_pack_error)?;

            write.publish().map_err(map_runtime_write_publish_error)?;
        }

        self.driver
            .engine
            .mark_channel_published(self.channel_index)
            .map_err(map_runtime_mark_published_error)
    }
}
