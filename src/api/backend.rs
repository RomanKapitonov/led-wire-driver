//! Public backend extension boundary for the driver.
//!
//! Backend implementations should depend on this module as the single backend
//! contract surface.
//!
//! Backend contract model:
//! - `register_channel` + `finalize_channels` define channel registration.
//! - `acquire_write_target(channel)` grants temporary mutable access to one
//!   backend-owned wire buffer for that channel.
//! - `publish_write(token)` publishes that write grant into backend-pending
//!   state for later transport submission.
//! - `submit_channels(mask)` asks backend transport to submit committed channels.
//!
//! Write target ownership:
//! - memory for write targets is backend-owned and must outlive grants.
//! - when `AcquireWrite::Ready(grant)` is returned, grant `(ptr,len)` must be
//!   writable for the duration of that write call path.
//! - `token` identity is backend-defined and must be accepted exactly once by
//!   `publish_write`.
//!
//! Busy/submit semantics:
//! - `AcquireWrite::Busy`: channel currently has no writable target available.
//! - `StartTransfer::Busy`: no transfer batch was accepted; caller retries from
//!   idle without dropping pending channel mask.
//! - `StartTransfer::Started`: transfer batch was accepted; completion is later
//!   indicated via `BackendEvent::TransferComplete`.
//!
//! Signal/event semantics:
//! - `on_signal` is for low-level backend signal ingress (e.g. IRQ lines).
//! - `on_event` is for logical backend events after host/runtime translation.
//! - event meaning stays backend-owned; driver reacts only to declared events.

use crate::model::PixelLayout;

#[derive(Copy, Clone, Debug, PartialEq, Eq, defmt::Format)]
pub enum BackendError {
    /// Submission shape/identity is invalid for this backend instance.
    InvalidBinding,
    /// Transport reported an unexpected fatal condition.
    TransportFault { raw_code: u8 },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendSignal {
    Line0,
    Line1,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    TransferComplete,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendChannelSpec {
    pub channel: u8,
    pub pixels: u16,
    pub layout: PixelLayout,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendWriteGrant {
    pub channel: u8,
    pub token: u16,
    pub ptr: *mut u8,
    pub len: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AcquireWrite {
    Ready(BackendWriteGrant),
    Busy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartTransfer {
    Started,
    Busy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub max_channels: usize,
    pub max_bytes_per_channel: Option<u32>,
    pub requires_dma_accessible_memory: bool,
}

pub trait LedBackend {
    fn init(&mut self) -> Result<(), BackendError>;
    fn capabilities(&self) -> BackendCapabilities;

    /// Registers one logical channel with semantic metadata.
    ///
    /// This is the preferred transport-agnostic registration path.
    fn register_channel(&mut self, spec: BackendChannelSpec) -> Result<(), BackendError> {
        let _ = spec;
        Ok(())
    }

    /// Finalizes channel registration for backends using `register_channel`.
    fn finalize_channels(&mut self) -> Result<(), BackendError> {
        Ok(())
    }

    /// Acquires a writable backend-owned target for the given channel.
    ///
    /// This is the preferred transport-agnostic write path.
    fn acquire_write_target(&mut self, channel: u8) -> Result<AcquireWrite, BackendError> {
        let _ = channel;
        Ok(AcquireWrite::Busy)
    }

    /// Publishes a previously acquired write grant token.
    fn publish_write(&mut self, token: u16) -> Result<(), BackendError> {
        let _ = token;
        Ok(())
    }

    /// Submits a committed logical channel mask.
    ///
    /// Completion ownership:
    /// - `StartTransfer::Started` means this exact submission batch was
    ///   accepted by transport.
    /// - `StartTransfer::Busy` means no batch was accepted and caller should
    ///   retry from idle without losing pending mask.
    fn submit_channels(&mut self, mask_bits: u32) -> Result<StartTransfer, BackendError>;

    fn on_signal(&mut self, signal: BackendSignal);

    /// Completion/event hook for backend-owned runtime state.
    fn on_event(&mut self, _event: BackendEvent) {}
}
