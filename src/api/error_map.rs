use crate::{
    api::backend::BackendError,
    engine::EngineError,
};

use super::types::{DriverInitError, FinalizeError, RegisterError, RuntimeError};

pub(super) fn map_driver_init_error(err: EngineError) -> DriverInitError {
    match err {
        EngineError::Backend(_) => DriverInitError::Backend,
        _ => unexpected_driver_init_error(err, "engine.init"),
    }
}

pub(super) fn map_register_bind_error(err: EngineError) -> RegisterError {
    match err {
        EngineError::ChannelAlreadyRegistered => RegisterError::DuplicateChannel,
        EngineError::ChannelOutOfRange | EngineError::InvalidWireSpan => RegisterError::InvalidBinding,
        EngineError::Backend(BackendError::InvalidBinding) => RegisterError::InvalidBinding,
        EngineError::Backend(BackendError::TransportFault { .. }) => RegisterError::Backend,
        _ => unexpected_register_error(err, "engine.register_channel"),
    }
}

pub(super) fn map_finalize_error(err: EngineError) -> FinalizeError {
    match err {
        EngineError::Backend(BackendError::InvalidBinding) => FinalizeError::InvalidBinding,
        EngineError::Backend(BackendError::TransportFault { .. }) => FinalizeError::Backend,
        _ => unexpected_finalize_error(err, "engine.finalize_configuration"),
    }
}

pub(super) fn map_runtime_write_prepare_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::WriteBusy => RuntimeError::Busy,
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => RuntimeError::InvalidChannel,
        EngineError::InvalidWireSpan => RuntimeError::Backend,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.prepare_channel_write"),
    }
}

pub(super) fn map_runtime_write_pack_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::SourceLengthMismatch { .. } => RuntimeError::LengthMismatch,
        EngineError::InvalidWireSpan | EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.write_slice_to_plan"),
    }
}

pub(super) fn map_runtime_mark_written_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => RuntimeError::InvalidChannel,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.mark_channel_written"),
    }
}

pub(super) fn map_runtime_commit_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => RuntimeError::InvalidChannel,
        _ => unexpected_runtime_error(err, "engine.submit_dirty"),
    }
}

pub(super) fn map_runtime_service_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.service"),
    }
}

fn unexpected_register_error(err: EngineError, operation: &'static str) -> RegisterError {
    debug_assert!(
        false,
        "unexpected register-phase engine error in {}: {:?}",
        operation,
        err
    );
    RegisterError::Backend
}

fn unexpected_driver_init_error(err: EngineError, operation: &'static str) -> DriverInitError {
    debug_assert!(
        false,
        "unexpected init-phase engine error in {}: {:?}",
        operation,
        err
    );
    DriverInitError::Backend
}

fn unexpected_finalize_error(err: EngineError, operation: &'static str) -> FinalizeError {
    debug_assert!(
        false,
        "unexpected finalize-phase engine error in {}: {:?}",
        operation,
        err
    );
    FinalizeError::Backend
}

fn unexpected_runtime_error(err: EngineError, operation: &'static str) -> RuntimeError {
    debug_assert!(
        false,
        "unexpected runtime-phase engine error in {}: {:?}",
        operation,
        err
    );
    RuntimeError::Backend
}
