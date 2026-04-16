//! Engine-owned channel registration semantics and storage.
//!
//! Registration uses a staged-then-commit model:
//! - [`RegistrationPlan`] validates one prepared setup into backend specs,
//!   channel handles, and staged records without mutating engine state
//! - backend configuration is applied against that staged plan as one batch
//! - only after backend success does [`RegistrationTable`] commit the staged
//!   channel records locally
//!
//! This keeps configuration atomic from the driver's point of view while still
//! allowing registration planning to remain a separate internal concern.

use heapless::Vec;

use super::{EngineError, mask::ChannelMask};

const _: () = {
    assert!(
        DRIVER_MAX_CHANNELS <= u8::MAX as usize,
        "DRIVER_MAX_CHANNELS exceeds u8 capacity; update ChannelId storage before raising this limit"
    );
};
use crate::{
    DRIVER_MAX_CHANNELS,
    api::{
        backend::BackendChannelSpec,
        channel::{Channel, DriverId},
        setup::PreparedSetup,
    },
    model::{BackendChannelId, ChannelId, FrameEpoch, PixelLayout},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelState {
    pub len_pixels: usize,
    pub layout: PixelLayout,
    pub backend_channel: BackendChannelId,
    pub frame_phase: FrameEpoch,
}

impl ChannelState {
    pub const fn new(
        backend_channel: BackendChannelId,
        len_pixels: usize,
        layout: PixelLayout,
    ) -> Self {
        Self {
            len_pixels,
            backend_channel,
            layout,
            frame_phase: FrameEpoch::ZERO,
        }
    }

    pub fn advance_phase(&mut self) {
        self.frame_phase = self.frame_phase.wrapping_add(1);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegistrationPlan {
    handles: [Option<Channel>; DRIVER_MAX_CHANNELS],
    records: [Option<ChannelState>; DRIVER_MAX_CHANNELS],
    specs: Vec<BackendChannelSpec, { DRIVER_MAX_CHANNELS }>,
    order: Vec<usize, { DRIVER_MAX_CHANNELS }>,
}

impl RegistrationPlan {
    /// Builds one staged registration unit from a validated prepared setup.
    ///
    /// This stage performs structural limit checks and produces:
    /// - runtime channel handles for the owning driver instance
    /// - staged channel records for local commit
    /// - backend channel specs for batch backend configuration
    pub(crate) fn from_prepared_setup(
        setup: &PreparedSetup,
        driver_id: DriverId,
        max_channels: usize,
        max_bytes_per_channel: Option<u32>,
    ) -> Result<Self, EngineError> {
        let mut handles = [None; DRIVER_MAX_CHANNELS];
        let mut records = [None; DRIVER_MAX_CHANNELS];
        let mut specs = Vec::<BackendChannelSpec, { DRIVER_MAX_CHANNELS }>::new();
        let mut order = Vec::<usize, { DRIVER_MAX_CHANNELS }>::new();

        for binding in setup.bindings() {
            let channel_index = binding.logical_channel.as_index();
            if channel_index >= max_channels {
                return Err(EngineError::ChannelOutOfRange);
            }

            if records[channel_index].is_some() {
                return Err(EngineError::ChannelAlreadyRegistered);
            }

            let wire_bytes = u32::from(binding.pixels)
                .checked_mul(3)
                .ok_or(EngineError::ConfigurationLimitExceeded)?;
            if let Some(max_bytes) = max_bytes_per_channel
                && wire_bytes > max_bytes
            {
                return Err(EngineError::ConfigurationLimitExceeded);
            }

            let record = ChannelState::new(
                binding.backend_channel,
                binding.pixels as usize,
                binding.layout,
            );
            let spec = BackendChannelSpec {
                channel: record.backend_channel,
                pixels: u16::try_from(record.len_pixels)
                    .map_err(|_| EngineError::ChannelOutOfRange)?,
                layout: record.layout,
            };

            let logical_channel = ChannelId::from_index(channel_index)
                .expect("registration plan channel index must fit in ChannelId");
            handles[channel_index] = Some(Channel::new(driver_id, logical_channel));
            records[channel_index] = Some(record);
            specs
                .push(spec)
                .map_err(|_| EngineError::ConfigurationLimitExceeded)?;
            order
                .push(channel_index)
                .map_err(|_| EngineError::ConfigurationLimitExceeded)?;
        }

        Ok(Self {
            handles,
            records,
            specs,
            order,
        })
    }

    pub(crate) fn handles(&self) -> [Option<Channel>; DRIVER_MAX_CHANNELS] {
        self.handles
    }

    pub(crate) fn specs(&self) -> &[BackendChannelSpec] {
        self.specs.as_slice()
    }

    pub(crate) fn staged_records(&self) -> impl Iterator<Item = (usize, ChannelState)> + '_ {
        self.order.iter().map(|&channel_index| {
            let record = self.records[channel_index]
                .expect("registration plan order must only reference populated records");
            (channel_index, record)
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct RegistrationTable {
    pub(super) records: [Option<ChannelState>; DRIVER_MAX_CHANNELS],
}

impl RegistrationTable {
    pub const fn new() -> Self {
        Self {
            records: [const { None }; DRIVER_MAX_CHANNELS],
        }
    }

    pub fn register(
        &mut self,
        max_channels: usize,
        channel_index: usize,
        channel: ChannelState,
    ) -> Result<(), EngineError> {
        if channel_index >= max_channels {
            return Err(EngineError::ChannelOutOfRange);
        }
        if self.records[channel_index].is_some() {
            return Err(EngineError::ChannelAlreadyRegistered);
        }
        self.records[channel_index] = Some(channel);
        Ok(())
    }

    pub fn commit_plan(
        &mut self,
        max_channels: usize,
        plan: &RegistrationPlan,
    ) -> Result<(), EngineError> {
        // Local registration state is committed only after backend batch
        // configuration succeeds. This method applies the staged records from
        // that already-accepted plan.
        for (channel_index, channel) in plan.staged_records() {
            self.register(max_channels, channel_index, channel)?;
        }
        Ok(())
    }

    pub fn record(
        &self,
        channel_index: usize,
        max_channels: usize,
    ) -> Result<ChannelState, EngineError> {
        if channel_index >= max_channels {
            return Err(EngineError::ChannelOutOfRange);
        }
        self.records[channel_index].ok_or(EngineError::ChannelNotRegistered)
    }

    pub fn record_mut(
        &mut self,
        channel_index: usize,
        max_channels: usize,
    ) -> Result<&mut ChannelState, EngineError> {
        if channel_index >= max_channels {
            return Err(EngineError::ChannelOutOfRange);
        }
        self.records[channel_index]
            .as_mut()
            .ok_or(EngineError::ChannelNotRegistered)
    }

    pub fn registered_channel_mask(&self, max_channels: usize) -> Result<ChannelMask, EngineError> {
        let mut bits = 0u32;
        for (index, record) in self.records.iter().enumerate().take(max_channels) {
            if record.is_some() {
                bits |= Self::mask_bit(index)?.bits();
            }
        }
        Ok(ChannelMask::from_bits(bits))
    }

    pub fn mark_written(
        &self,
        channel_index: usize,
        max_channels: usize,
        dirty_mask: ChannelMask,
    ) -> Result<ChannelMask, EngineError> {
        self.record(channel_index, max_channels)?;
        let bit = Self::mask_bit(channel_index)?;
        Ok(ChannelMask::from_bits(dirty_mask.bits() | bit.bits()))
    }

    pub fn advance_phase(
        &mut self,
        channel_index: usize,
        max_channels: usize,
    ) -> Result<(), EngineError> {
        let record = self.record_mut(channel_index, max_channels)?;
        record.advance_phase();
        Ok(())
    }

    pub fn mask_bit(channel_index: usize) -> Result<ChannelMask, EngineError> {
        debug_assert!(
            channel_index < ChannelMask::CAPACITY_BITS,
            "channel index exceeds channel mask capacity"
        );
        if channel_index >= ChannelMask::CAPACITY_BITS {
            return Err(EngineError::ConfigurationLimitExceeded);
        }
        Ok(ChannelMask::from_bits(1u32 << channel_index))
    }
}
