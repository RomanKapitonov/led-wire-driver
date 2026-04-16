#![no_std]

//! LED wire driver.
//!
//! Public surfaces:
//! - [`api`]: independent driver boundary.
//! - setup phase: validated [`api::PreparedSetup`] construction ->
//!   [`api::Driver::new`] -> one atomic `configure_prepared` -> `finalize`.
//! - runtime phase: channel writes via `driver.channel(channel)?.write_rgb48(...)`,
//!   then `commit`, periodic `service`, and direct backend ingress through
//!   semantic [`api::Driver::on_backend_event`] and backend-private
//!   [`api::Driver::on_backend_signal`].
//!
//! Internal layers:
//! - [`engine`]: authoritative driver state machine.
//! - [`api::backend`]: hardware-neutral backend contracts.
//! - [`pack`]: wire-format packing pipeline.
//!
//! Feature model:
//! - packing policy features are exclusive per axis,
//! - supported combinations are:
//!   - default features,
//!   - `pack-td-none + pack-sq-none`,
//!   - `pack-td-none + pack-sq-bayer`,
//!   - `pack-td-bayer + pack-sq-none`,
//!   - `pack-td-bayer + pack-sq-bayer`,
//! - `--all-features` is intentionally unsupported for this crate.
//! - revisit this only if external library/tooling requirements or new policy
//!   axes make additive feature behavior materially valuable.

#[cfg(test)]
extern crate std;

/// Compile-time driver logical-channel storage capacity.
///
/// Active channel count is still runtime-defined by registration and backend
/// capabilities.
pub const DRIVER_MAX_CHANNELS: usize = 8;

pub(crate) mod model;
pub mod backend;
pub mod setup;
pub mod error;
pub mod driver;

// Old modules kept until Tasks 1-9 complete
pub(crate) mod engine;
pub(crate) mod pack;
pub mod api;

#[cfg(test)]
pub(crate) mod test_support;
