//! Engine-owned channel registration semantics and storage.

use heapless::Vec;

use super::{EngineError, mask::ChannelMask};
use crate::{
    DRIVER_MAX_CHANNELS,
    api::{
        backend::BackendChannelSpec,
        types::{Channel, PreparedSetup},
    },
    model::{BackendChannelId, FrameEpoch, PixelLayout},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelMeta {
    pub len_pixels: usize,
    pub layout: PixelLayout,
    pub backend_channel: BackendChannelId,
}

impl ChannelMeta {
    pub const fn new(
        backend_channel: BackendChannelId,
        len_pixels: usize,
        layout: PixelLayout,
    ) -> Self {
        Self {
            len_pixels,
            layout,
            backend_channel,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelRuntime {
    pub frame_phase: FrameEpoch,
}

impl ChannelRuntime {
    pub const fn new() -> Self {
        Self {
            frame_phase: FrameEpoch::ZERO,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelState {
    pub meta: ChannelMeta,
    pub runtime: ChannelRuntime,
}

impl ChannelState {
    pub const fn new(
        backend_channel: BackendChannelId,
        len_pixels: usize,
        layout: PixelLayout,
    ) -> Self {
        Self {
            meta: ChannelMeta::new(backend_channel, len_pixels, layout),
            runtime: ChannelRuntime::new(),
        }
    }

    pub const fn len_pixels(self) -> usize {
        self.meta.len_pixels
    }

    pub const fn layout(self) -> PixelLayout {
        self.meta.layout
    }

    pub const fn backend_channel(self) -> BackendChannelId {
        self.meta.backend_channel
    }

    pub const fn frame_phase(self) -> FrameEpoch {
        self.runtime.frame_phase
    }

    pub fn advance_phase_if_dirty(&mut self, dirty: bool) {
        if dirty {
            self.runtime.frame_phase = self.runtime.frame_phase.wrapping_add(1);
        }
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
    pub(crate) fn from_prepared_setup(
        setup: &PreparedSetup,
        driver_id: u32,
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
            if let Some(max_bytes) = max_bytes_per_channel {
                if wire_bytes > max_bytes {
                    return Err(EngineError::ConfigurationLimitExceeded);
                }
            }

            let record = ChannelState::new(
                binding.backend_channel,
                binding.pixels as usize,
                binding.layout,
            );
            let spec = BackendChannelSpec {
                channel: record.backend_channel(),
                pixels: u16::try_from(record.len_pixels())
                    .map_err(|_| EngineError::ChannelOutOfRange)?,
                layout: record.layout(),
            };

            debug_assert!(
                DRIVER_MAX_CHANNELS <= u8::MAX as usize,
                "Channel handle index storage assumes DRIVER_MAX_CHANNELS fits in u8"
            );
            handles[channel_index] = Some(Channel::new(driver_id, channel_index as u8));
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

    pub fn advance_phase_if_dirty(
        &mut self,
        channel_index: usize,
        max_channels: usize,
        dirty: bool,
    ) -> Result<(), EngineError> {
        let record = self.record_mut(channel_index, max_channels)?;
        record.advance_phase_if_dirty(dirty);
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
