//! Public backend extension boundary for the driver.
//!
//! Backend implementations should depend on this module as the single backend
//! contract surface.
//!
//! Backend contract model:
//! - `configure_channels(&[BackendChannelSpec])` defines atomic channel
//!   registration.
//! - `acquire_write_target(channel)` grants temporary mutable access to one
//!   backend-owned wire buffer for that channel.
//! - `publish()` on the acquired lease publishes that write into backend-pending
//!   state for later transport submission.
//! - `submit_channels(mask)` asks backend transport to submit committed channels.
//!
//! Write target ownership:
//! - memory for write targets is backend-owned.
//! - when `AcquireWrite::Ready(lease)` is returned, `lease.bytes_mut()` exposes
//!   the exact writable target for that acquired write.
//! - dropping an unpublished lease must abort it so the channel becomes
//!   retryable.
//!
//! Busy/submit semantics:
//! - `AcquireWrite::Busy`: channel currently has no writable target available.
//! - `StartTransfer::Busy`: no transfer batch was accepted; caller retries from
//!   idle without dropping pending channel mask.
//! - `StartTransfer::Started`: transfer batch was accepted; completion is later
//!   indicated via `BackendEvent::TransferComplete`.
//!
//! Signal/event semantics:
//! - `on_signal` is for backend-private low-level ingress before translation
//!   into semantic backend events.
//! - `on_event` is for logical backend events after host/runtime translation.
//! - only `BackendEvent` carries engine-meaningful semantics.
//! - `BackendSignal` is intentionally opaque and transport/backend-local.
//! - `BackendEvent::TransferComplete` is only valid after the backend has
//!   previously accepted `StartTransfer::Started` and before that in-flight
//!   batch has been completed by the driver runtime.
//! - reporting `BackendEvent::TransferComplete` while no transfer is in flight
//!   is a backend contract violation; the engine may latch that violation and
//!   surface it on a later runtime call as a `ServiceError::BackendContract`.

use crate::model::{BackendChannelId, PixelLayout};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    /// Submission shape/identity is invalid for this backend instance.
    InvalidBinding,
    /// Transport reported an unexpected fatal condition.
    TransportFault { raw_code: u8 },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendSignal {
    /// Backend-private signal token.
    ///
    /// The driver does not attach semantic meaning to this value. It exists so
    /// backends with transport-specific ingress can keep a pre-translation hook
    /// without forcing those transport details into [`BackendEvent`].
    Opaque(u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    /// One previously accepted submission batch has completed.
    TransferComplete,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendChannelSpec {
    pub channel: BackendChannelId,
    pub pixels: u16,
    pub layout: PixelLayout,
}

/// Safe lease contract selected for the long-term backend write boundary.
pub trait BackendWriteLease {
    /// Returns the backend channel identity this live lease was acquired for.
    fn channel(&self) -> BackendChannelId;

    /// Exposes the exact writable backend-owned target for this acquired write.
    fn bytes_mut(&mut self) -> &mut [u8];

    /// Publishes this acquired write into backend-pending state.
    fn publish(&mut self) -> Result<(), BackendError>;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AcquireWrite<G> {
    Ready(G),
    Busy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartTransfer {
    Started,
    Busy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendCapabilities {
    /// Maximum number of backend channels the driver may register.
    pub max_channels: usize,
    /// Structural per-channel byte limit enforced during registration staging.
    pub max_bytes_per_channel: Option<u32>,
}

pub trait LedBackend {
    type WriteLease<'a>: BackendWriteLease + 'a
    where
        Self: 'a;

    /// Performs backend-local initialization before registration begins.
    fn init(&mut self) -> Result<(), BackendError>;

    /// Returns structural limits that shape what the driver may register.
    fn capabilities(&self) -> BackendCapabilities;

    /// Applies the full logical-channel configuration for this backend.
    fn configure_channels(&mut self, specs: &[BackendChannelSpec]) -> Result<(), BackendError> {
        let _ = specs;
        Ok(())
    }

    /// Acquires a writable backend-owned target for the given channel.
    fn acquire_write_target(
        &mut self,
        channel: BackendChannelId,
    ) -> Result<AcquireWrite<Self::WriteLease<'_>>, BackendError> {
        let _ = channel;
        Ok(AcquireWrite::Busy)
    }

    /// Submits a committed logical channel mask.
    fn submit_channels(&mut self, mask_bits: u32) -> Result<StartTransfer, BackendError>;

    /// Backend-private low-level ingress hook.
    fn on_signal(&mut self, signal: BackendSignal);

    /// Completion/event hook for backend-owned runtime state.
    fn on_event(&mut self, _event: BackendEvent) {}
}
