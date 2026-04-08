//! Public API projection of the internal engine error taxonomy.
//!
//! Mapping policy:
//! - each public boundary must explicitly enumerate the engine errors it
//!   expects to receive
//! - path-impossible engine errors are treated as maintenance backstops rather
//!   than broadened into misleading public categories
//! - [`unexpected_engine_error`] is therefore intentionally narrow and should
//!   only remain for impossible API-path cases
//!
//! For a worked example of why each impossible case is unreachable on its
//! specific call path, see the comments in [`map_runtime_write_prepare_error`].

use super::errors::{DriverInitError, RegisterError, RuntimeError};
use crate::{api::backend::BackendError, engine::EngineError};

pub(super) fn map_driver_init_error(err: EngineError) -> DriverInitError {
    match err {
        EngineError::Backend(_) => DriverInitError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelOutOfRange
        | EngineError::ChannelNotRegistered
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy
        | EngineError::BackendContractViolation(_) => {
            unexpected_engine_error("engine.init", err, DriverInitError::Backend)
        }
    }
}

pub(super) fn map_register_bind_error(err: EngineError) -> RegisterError {
    match err {
        EngineError::ChannelAlreadyRegistered => RegisterError::DuplicateChannel,
        EngineError::ChannelOutOfRange | EngineError::ConfigurationLimitExceeded => {
            RegisterError::InvalidBinding
        }
        EngineError::BackendContractViolation(_) => RegisterError::InvalidBinding,
        EngineError::Backend(BackendError::InvalidBinding) => RegisterError::InvalidBinding,
        EngineError::Backend(BackendError::TransportFault { .. }) => RegisterError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelNotRegistered
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy => {
            unexpected_engine_error("engine registration", err, RegisterError::Backend)
        }
    }
}

/// Canonical example of the impossible-case pattern used across all mapping
/// functions. Each `unexpected_engine_error` arm below notes why that variant
/// cannot reach this call site; the other mapping functions follow the same
/// reasoning without repeating it in full.
pub(super) fn map_runtime_write_prepare_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::WriteBusy => RuntimeError::Busy,
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        // Impossible on this path:
        // - InvalidState: lifecycle state is validated before any write attempt
        // - ChannelAlreadyRegistered: duplicate check happens at registration, not write time
        // - ConfigurationLimitExceeded: limit check happens at registration, not write time
        // - SourceLengthMismatch: length is checked during pack, not during prepare
        EngineError::InvalidState(_)
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. } => {
            unexpected_engine_error("engine.acquire_prepared_write", err, RuntimeError::Backend)
        }
    }
}

pub(super) fn map_runtime_write_pack_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::SourceLengthMismatch { .. } => RuntimeError::LengthMismatch,
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelOutOfRange
        | EngineError::ChannelNotRegistered
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::WriteBusy => unexpected_engine_error(
            "prepared_write.pack_rgb48_active",
            err,
            RuntimeError::Backend,
        ),
    }
}

pub(super) fn map_runtime_write_publish_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy => {
            unexpected_engine_error("prepared_write.publish", err, RuntimeError::Backend)
        }
    }
}

pub(super) fn map_runtime_mark_published_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy => {
            unexpected_engine_error("engine.mark_channel_published", err, RuntimeError::Backend)
        }
    }
}

pub(super) fn map_runtime_commit_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy => {
            unexpected_engine_error("engine.submit_dirty", err, RuntimeError::Backend)
        }
    }
}

pub(super) fn map_runtime_service_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        EngineError::InvalidState(_)
        | EngineError::ChannelOutOfRange
        | EngineError::ChannelNotRegistered
        | EngineError::ChannelAlreadyRegistered
        | EngineError::ConfigurationLimitExceeded
        | EngineError::SourceLengthMismatch { .. }
        | EngineError::WriteBusy => {
            unexpected_engine_error("engine.service", err, RuntimeError::Backend)
        }
    }
}

#[cold]
fn unexpected_engine_error<T>(operation: &'static str, err: EngineError, fallback: T) -> T {
    // Maintenance backstop for engine errors that should be unreachable on a
    // given public API path. Reachable cases must be matched explicitly above.
    debug_assert!(false, "unexpected engine error in {}: {:?}", operation, err);
    fallback
}
