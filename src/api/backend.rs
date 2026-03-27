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
//!   surface it on a later runtime call as `RuntimeError::BackendContract`.

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
///
/// Contract:
/// - `bytes_mut()` exposes the exact writable target for this acquired write.
/// - the returned borrow must not outlive the lease object.
/// - `publish()` transfers the acquired write into backend-pending state.
/// - if the lease is dropped without successful publish, backend-local cleanup
///   must abort the write so the channel becomes retryable.
pub trait BackendWriteLease {
    /// Returns the backend channel identity this live lease was acquired for.
    fn channel(&self) -> BackendChannelId;

    /// Exposes the exact writable backend-owned target for this acquired write.
    ///
    /// The returned slice borrow must not outlive the lease object.
    fn bytes_mut(&mut self) -> &mut [u8];

    /// Publishes this acquired write into backend-pending state.
    ///
    /// After successful publish, dropping the lease must no longer abort the
    /// acquired write.
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
    ///
    /// The driver computes `pixels * 3` for each prepared binding and rejects
    /// configurations that exceed this limit before backend mutation.
    pub max_bytes_per_channel: Option<u32>,
}

pub trait LedBackend {
    type WriteLease<'a>: BackendWriteLease + 'a
    where
        Self: 'a;

    /// Performs backend-local initialization before registration begins.
    ///
    /// This is the only failure point during `Driver::new()`: callers should
    /// see `DriverInitError::Backend` if initialization does not succeed.
    fn init(&mut self) -> Result<(), BackendError>;

    /// Returns structural limits that shape what the driver may register.
    ///
    /// These capabilities are read during engine construction/registration
    /// planning and are expected to remain stable for the lifetime of the
    /// backend instance.
    fn capabilities(&self) -> BackendCapabilities;

    /// Applies the full logical-channel configuration for this backend.
    ///
    /// The call must behave as one configuration unit from the driver's
    /// perspective:
    /// - on success the backend is fully configured for the given set,
    /// - on failure the backend must not require driver recreation just to
    ///   clear partial registration state,
    /// - retry after failure must remain possible unless the backend reports a
    ///   hard fatal condition.
    fn configure_channels(&mut self, specs: &[BackendChannelSpec]) -> Result<(), BackendError> {
        let _ = specs;
        Ok(())
    }

    /// Acquires a writable backend-owned target for the given channel.
    ///
    /// This is the preferred transport-agnostic write path.
    fn acquire_write_target(
        &mut self,
        channel: BackendChannelId,
    ) -> Result<AcquireWrite<Self::WriteLease<'_>>, BackendError> {
        let _ = channel;
        Ok(AcquireWrite::Busy)
    }

    /// Submits a committed logical channel mask.
    ///
    /// Completion ownership:
    /// - `StartTransfer::Started` means this exact submission batch was
    ///   accepted by transport.
    /// - `StartTransfer::Busy` means no batch was accepted and caller should
    ///   retry from idle without losing pending mask.
    fn submit_channels(&mut self, mask_bits: u32) -> Result<StartTransfer, BackendError>;

    /// Backend-private low-level ingress hook.
    ///
    /// This is intended for transport/backend-local notifications before they
    /// are translated into logical backend events. The driver does not attach
    /// any meaning to the signal token beyond forwarding it to the backend.
    fn on_signal(&mut self, signal: BackendSignal);

    /// Completion/event hook for backend-owned runtime state.
    ///
    /// Contract:
    /// - `BackendEvent::TransferComplete` must only be emitted for a transfer
    ///   batch previously accepted via `StartTransfer::Started`
    /// - emitting completion while the driver has no in-flight transfer is a
    ///   backend contract violation
    fn on_event(&mut self, _event: BackendEvent) {}
}
