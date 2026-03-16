//! Independent driver API.
//!
//! It does not expose any firmware-owned host/runtime integration path.
//!
//! The intended usage story is:
//! - setup through [`Driver::new`] and [`Driver::configure_prepared`]
//! - runtime channel writes through `driver.channel(channel).write_rgb48(...)`
//! - submission through [`Driver::commit`]
//! - host/backend ingress through [`crate::host::DriverHostIngress`]
//! - backend implementation contracts through [`backend`]
//!
//! Buffer provisioning is backend-owned and is intentionally outside this API.
//!
//! API invariants:
//! - `configure_prepared` is valid only before [`Driver::finalize`]
//! - `channel(...)/commit/service` are valid only after [`Driver::finalize`]
//! - channel tokens are driver-owned; cross-driver token usage is rejected
//! - lifecycle order is enforced by typestate (`Configuring` -> `Ready`)
//!
//! Minimal usage shape:
//! ```rust,ignore
//! use crate::api::{
//!     Driver, PreparedSetup, Rgb48,
//! };
//!
//! let setup: PreparedSetup = /* produced by preparation/bootstrap boundary */;
//! let mut configuring = Driver::new(MyBackend::new())?;
//! let handles = configuring.configure_prepared(&setup)?;
//! let mut driver = configuring.finalize()?;
//!
//! let channel = handles
//!     .get(crate::api::ChannelId::new(0))
//!     .unwrap();
//! driver
//!     .channel(channel)?
//!     .write_rgb48(&[Rgb48 { r: 65535, g: 0, b: 0 }])?;
//! driver.commit()?;
//! driver.service()?;
//! # Ok::<(), ()>(())
//! ```
//!
//! Typestate misuse (intended compile-time failures):
//! - calling `commit` on `Driver<_, Configuring>`
//! - calling `configure_prepared` on `Driver<_, Ready>`
pub mod backend;

mod driver;
mod error_map;
mod types;

pub use driver::{Configuring, Driver, Ready};
pub use types::{
    BackendChannelId, ChannelId, ConfiguredChannels, DriverInitError, FinalizeError, PixelLayout,
    PreparedSetup, RegisterError, Rgb48, RuntimeError,
};
