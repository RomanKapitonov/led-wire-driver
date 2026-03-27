#![cfg(test)]
#![allow(dead_code)]

use std::{array, cell::RefCell, rc::Rc, vec, vec::Vec};

use crate::{
    DRIVER_MAX_CHANNELS,
    api::backend::{
        AcquireWrite, BackendCapabilities, BackendChannelSpec, BackendError, BackendEvent,
        BackendSignal, BackendWriteLease, LedBackend, StartTransfer,
    },
    model::BackendChannelId,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FakeBackendLog {
    pub init_calls: usize,
    pub configured_specs: Vec<Vec<BackendChannelSpec>>,
    pub submit_masks: Vec<u32>,
    pub signals: Vec<BackendSignal>,
    pub events: Vec<BackendEvent>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AcquireBehavior {
    Ready,
    Busy,
    Error(BackendError),
    WrongChannel(BackendChannelId),
    WrongLength(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FakeBackendScript {
    next_init: Option<BackendError>,
    next_configure: Option<BackendError>,
    next_submit: Option<Result<StartTransfer, BackendError>>,
    acquire: [AcquireBehavior; DRIVER_MAX_CHANNELS],
    publish: [Option<BackendError>; DRIVER_MAX_CHANNELS],
}

impl Default for FakeBackendScript {
    fn default() -> Self {
        Self {
            next_init: None,
            next_configure: None,
            next_submit: None,
            acquire: [AcquireBehavior::Ready; DRIVER_MAX_CHANNELS],
            publish: [None; DRIVER_MAX_CHANNELS],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FakeBackendShared {
    script: FakeBackendScript,
    log: FakeBackendLog,
}

#[derive(Clone, Debug)]
pub(crate) struct FakeBackendHandle {
    shared: Rc<RefCell<FakeBackendShared>>,
}

impl FakeBackendHandle {
    pub(crate) fn script_init_error(&self, err: BackendError) {
        self.shared.borrow_mut().script.next_init = Some(err);
    }

    pub(crate) fn script_configure_error(&self, err: BackendError) {
        self.shared.borrow_mut().script.next_configure = Some(err);
    }

    pub(crate) fn script_submit_result(&self, result: Result<StartTransfer, BackendError>) {
        self.shared.borrow_mut().script.next_submit = Some(result);
    }

    pub(crate) fn script_acquire_busy(&self, channel: BackendChannelId) {
        self.shared.borrow_mut().script.acquire[channel.as_index()] = AcquireBehavior::Busy;
    }

    pub(crate) fn script_acquire_error(&self, channel: BackendChannelId, err: BackendError) {
        self.shared.borrow_mut().script.acquire[channel.as_index()] = AcquireBehavior::Error(err);
    }

    pub(crate) fn script_acquire_wrong_channel(
        &self,
        requested: BackendChannelId,
        returned: BackendChannelId,
    ) {
        self.shared.borrow_mut().script.acquire[requested.as_index()] =
            AcquireBehavior::WrongChannel(returned);
    }

    pub(crate) fn script_acquire_wrong_length(&self, channel: BackendChannelId, len: usize) {
        self.shared.borrow_mut().script.acquire[channel.as_index()] =
            AcquireBehavior::WrongLength(len);
    }

    pub(crate) fn script_publish_error(&self, channel: BackendChannelId, err: BackendError) {
        self.shared.borrow_mut().script.publish[channel.as_index()] = Some(err);
    }

    pub(crate) fn log(&self) -> FakeBackendLog {
        self.shared.borrow().log.clone()
    }
}

#[derive(Clone, Debug)]
struct FakeChannelSlot {
    configured: bool,
    channel: BackendChannelId,
    bytes: Vec<u8>,
    write_acquired: bool,
    write_published: bool,
}

impl Default for FakeChannelSlot {
    fn default() -> Self {
        Self {
            configured: false,
            channel: BackendChannelId::new(0),
            bytes: Vec::new(),
            write_acquired: false,
            write_published: false,
        }
    }
}

pub(crate) struct FakeBackend {
    capabilities: BackendCapabilities,
    slots: [FakeChannelSlot; DRIVER_MAX_CHANNELS],
    shared: Rc<RefCell<FakeBackendShared>>,
}

impl FakeBackend {
    pub(crate) fn new(capabilities: BackendCapabilities) -> (Self, FakeBackendHandle) {
        let shared = Rc::new(RefCell::new(FakeBackendShared::default()));
        (
            Self {
                capabilities,
                slots: array::from_fn(|_| FakeChannelSlot::default()),
                shared: Rc::clone(&shared),
            },
            FakeBackendHandle { shared },
        )
    }

    fn validate_specs(&self, specs: &[BackendChannelSpec]) -> Result<(), BackendError> {
        let mut seen = [false; DRIVER_MAX_CHANNELS];
        for spec in specs {
            let index = spec.channel.as_index();
            if index >= self.capabilities.max_channels || index >= DRIVER_MAX_CHANNELS {
                return Err(BackendError::InvalidBinding);
            }
            if seen[index] {
                return Err(BackendError::InvalidBinding);
            }
            seen[index] = true;

            if let Some(max_bytes) = self.capabilities.max_bytes_per_channel {
                let wire_bytes = u32::from(spec.pixels)
                    .checked_mul(3)
                    .ok_or(BackendError::InvalidBinding)?;
                if wire_bytes > max_bytes {
                    return Err(BackendError::InvalidBinding);
                }
            }
        }
        Ok(())
    }
}

pub(crate) struct FakeWriteLease<'a> {
    slot: &'a mut FakeChannelSlot,
    shared: Rc<RefCell<FakeBackendShared>>,
    returned_channel: BackendChannelId,
    exposed_len: usize,
}

impl BackendWriteLease for FakeWriteLease<'_> {
    fn channel(&self) -> BackendChannelId {
        self.returned_channel
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.slot.bytes[..self.exposed_len]
    }

    fn publish(&mut self) -> Result<(), BackendError> {
        let channel_index = self.slot.channel.as_index();
        if let Some(err) = self.shared.borrow_mut().script.publish[channel_index].take() {
            return Err(err);
        }

        self.slot.write_published = true;
        self.slot.write_acquired = false;
        Ok(())
    }
}

impl Drop for FakeWriteLease<'_> {
    fn drop(&mut self) {
        if !self.slot.write_published {
            self.slot.write_acquired = false;
        }
    }
}

impl LedBackend for FakeBackend {
    type WriteLease<'a>
        = FakeWriteLease<'a>
    where
        Self: 'a;

    fn init(&mut self) -> Result<(), BackendError> {
        let mut shared = self.shared.borrow_mut();
        shared.log.init_calls += 1;
        if let Some(err) = shared.script.next_init.take() {
            return Err(err);
        }
        Ok(())
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    fn configure_channels(&mut self, specs: &[BackendChannelSpec]) -> Result<(), BackendError> {
        self.validate_specs(specs)?;

        let mut shared = self.shared.borrow_mut();
        shared.log.configured_specs.push(specs.to_vec());
        if let Some(err) = shared.script.next_configure.take() {
            return Err(err);
        }
        drop(shared);

        self.slots = array::from_fn(|_| FakeChannelSlot::default());
        for spec in specs {
            let index = spec.channel.as_index();
            let wire_len = usize::from(spec.pixels) * 3;
            self.slots[index] = FakeChannelSlot {
                configured: true,
                channel: spec.channel,
                bytes: vec![0; wire_len],
                write_acquired: false,
                write_published: false,
            };
        }

        Ok(())
    }

    fn acquire_write_target(
        &mut self,
        channel: BackendChannelId,
    ) -> Result<AcquireWrite<Self::WriteLease<'_>>, BackendError> {
        let index = channel.as_index();
        if index >= self.capabilities.max_channels || index >= DRIVER_MAX_CHANNELS {
            return Err(BackendError::InvalidBinding);
        }

        let behavior = {
            let mut shared = self.shared.borrow_mut();
            let scripted = &mut shared.script.acquire[index];
            let current = *scripted;
            if !matches!(current, AcquireBehavior::Ready) {
                *scripted = AcquireBehavior::Ready;
            }
            current
        };

        let mut returned_channel = channel;
        let mut exposed_len_override = None;
        match behavior {
            AcquireBehavior::Busy => return Ok(AcquireWrite::Busy),
            AcquireBehavior::Error(err) => return Err(err),
            AcquireBehavior::WrongChannel(wrong) => returned_channel = wrong,
            AcquireBehavior::WrongLength(len) => exposed_len_override = Some(len),
            AcquireBehavior::Ready => {}
        }

        let slot_len = self.slots[index].bytes.len();
        let exposed_len = exposed_len_override.unwrap_or(slot_len).min(slot_len);
        let slot = &mut self.slots[index];
        if !slot.configured || slot.channel != channel || slot.write_acquired {
            return Ok(AcquireWrite::Busy);
        }

        slot.write_acquired = true;
        slot.write_published = false;

        Ok(AcquireWrite::Ready(FakeWriteLease {
            slot,
            shared: Rc::clone(&self.shared),
            returned_channel,
            exposed_len,
        }))
    }

    fn submit_channels(&mut self, mask_bits: u32) -> Result<StartTransfer, BackendError> {
        self.shared.borrow_mut().log.submit_masks.push(mask_bits);
        if let Some(result) = self.shared.borrow_mut().script.next_submit.take() {
            return result;
        }
        Ok(StartTransfer::Started)
    }

    fn on_signal(&mut self, signal: BackendSignal) {
        self.shared.borrow_mut().log.signals.push(signal);
    }

    fn on_event(&mut self, event: BackendEvent) {
        self.shared.borrow_mut().log.events.push(event);
    }
}
