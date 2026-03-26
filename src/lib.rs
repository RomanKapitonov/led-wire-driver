#![no_std]
#![no_main]

//! LED wire driver.
//!
//! Public surfaces:
//! - [`api`]: independent driver boundary.
//! - setup phase: validated [`api::PreparedSetup`] construction ->
//!   [`api::Driver::new`] -> one atomic `configure_prepared` -> `finalize`.
//! - runtime phase: channel writes via `driver.channel(channel)?.write_rgb48(...)`,
//!   then `commit`, periodic `service`, and direct backend ingress through
//!   [`api::Driver::on_backend_event`] / [`api::Driver::on_backend_signal`].
//!
//! Internal layers:
//! - [`engine`]: authoritative driver state machine.
//! - [`api::backend`]: hardware-neutral backend contracts.
//! - [`pack`]: wire-format packing pipeline.

/// Compile-time driver logical-channel storage capacity.
///
/// Active channel count is still runtime-defined by registration and backend
/// capabilities.
pub const DRIVER_MAX_CHANNELS: usize = 8;

pub(crate) mod engine;
pub(crate) mod model;
pub(crate) mod pack;

pub mod api;
