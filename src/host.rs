//! Integration-facing host ingress surface.
//!
//! This module is intentionally separate from `driver::api`:
//! - `driver::api` is the application-facing write/commit/service boundary.
//! - `driver::host` is the runtime/integration-facing backend signal/event ingress.

use crate::api::backend::{BackendEvent, BackendSignal};

pub trait DriverHostIngress {
    fn on_backend_signal(&mut self, signal: BackendSignal);
    fn on_backend_event(&mut self, event: BackendEvent);
}
