//! Independent driver API.
//!
//! It does not expose any firmware-owned host/runtime integration path.
//!
//! The intended usage story is:
//! - setup through validated [`PreparedSetup`] values and
//!   one single-shot call to [`Driver::configure_prepared`]
//! - runtime channel writes through `driver.channel(channel).write_rgb48(...)`
//! - submission through [`Driver::commit`]
//! - backend ingress through [`Driver::on_backend_signal`] for backend-private
//!   low-level signals and [`Driver::on_backend_event`] for semantic backend
//!   events
//! - backend implementation contracts through [`backend`]
//!
//! Buffer provisioning is backend-owned and is intentionally outside this API.
//! Backend-specific acceptance checks still happen later during registration.
//!
//! API invariants:
//! - `configure_prepared` is valid only before [`Driver::finalize`]
//! - `configure_prepared` is single-shot and atomically commits configuration
//! - `channel(...)/commit/service` are valid only after [`Driver::finalize`]
//! - channel handles are driver-owned; cross-driver handle usage is rejected
//! - lifecycle order is enforced by typestate (`Configuring` -> `Ready`)
//!
//! Minimal usage shape:
//! ```rust,ignore
//! use crate::api::{
//!     Driver, PreparedBinding, PreparedSetup, Rgb48,
//! };
//!
//! let setup = PreparedSetup::from_bindings([
//!     PreparedBinding::new(
//!         crate::api::ChannelId::new(0),
//!         crate::api::BackendChannelId::new(0),
//!         60,
//!         crate::api::PixelLayout::Grb,
//!     ),
//! ])?;
//! let mut configuring = Driver::new(MyBackend::new())?;
//! let handles = configuring.configure_prepared(&setup)?;
//! let mut driver = configuring.finalize();
//!
//! let channel = handles
//!     .get(crate::api::ChannelId::new(0))
//!     .unwrap();
//! driver
//!     .channel(channel)?
//!     .write_rgb48(&[Rgb48 { r: 65535, g: 0, b: 0 }])?;
//! driver.commit()?;
//! driver.service()?;
//! # Ok::<(), crate::api::SetupBuildError>(())
//! ```
//!
//! Typestate misuse (intended compile-time failures):
//! - calling `commit` on `Driver<_, Configuring>`
//! - calling `configure_prepared` on `Driver<_, Ready>`
//!
//! Runtime misuse rejected explicitly:
//! - calling `configure_prepared` with an empty setup
//! - calling `configure_prepared` more than once
//!
//! Runtime error categories:
//! - [`RuntimeError::InvalidChannel`] means the handle does not belong to this
//!   driver instance or no longer resolves to a registered channel
//! - [`RuntimeError::LengthMismatch`] means the source pixel slice length does
//!   not match the configured channel length
//! - [`RuntimeError::BackendContract`] means the backend violated the driver's
//!   expected runtime contract
//! - [`RuntimeError::Backend`] means an actual backend-owned failure such as a
//!   transport fault
pub mod backend;

pub(crate) mod channel;
mod driver;
mod error_map;
pub(crate) mod errors;
pub(crate) mod setup;
#[cfg(test)]
mod tests;

pub use crate::model::{BackendChannelId, ChannelId, PixelLayout, Rgb48};
pub use channel::ConfiguredChannels;
pub use driver::{Configuring, Driver, Ready};
pub use errors::{DriverInitError, RegisterError, RuntimeError};
pub use setup::{PreparedBinding, PreparedSetup, SetupBuildError};
