use super::types::{DriverInitError, RegisterError, RuntimeError};
use crate::{api::backend::BackendError, engine::EngineError};

pub(super) fn map_driver_init_error(err: EngineError) -> DriverInitError {
    match err {
        EngineError::Backend(_) => DriverInitError::Backend,
        _ => unexpected_driver_init_error(err, "engine.init"),
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
        _ => unexpected_register_error(err, "engine registration"),
    }
}

pub(super) fn map_runtime_write_prepare_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::WriteBusy => RuntimeError::Busy,
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.acquire_prepared_write"),
    }
}

pub(super) fn map_runtime_write_pack_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::SourceLengthMismatch { .. } => RuntimeError::LengthMismatch,
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "prepared_write.pack_rgb48_active"),
    }
}

pub(super) fn map_runtime_write_publish_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "prepared_write.publish"),
    }
}

pub(super) fn map_runtime_mark_published_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.mark_channel_published"),
    }
}

pub(super) fn map_runtime_commit_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::ChannelOutOfRange | EngineError::ChannelNotRegistered => {
            RuntimeError::InvalidChannel
        }
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.submit_dirty"),
    }
}

pub(super) fn map_runtime_service_error(err: EngineError) -> RuntimeError {
    match err {
        EngineError::BackendContractViolation(_) => RuntimeError::BackendContract,
        EngineError::Backend(_) => RuntimeError::Backend,
        _ => unexpected_runtime_error(err, "engine.service"),
    }
}

fn unexpected_register_error(err: EngineError, operation: &'static str) -> RegisterError {
    debug_assert!(
        false,
        "unexpected register-phase engine error in {}: {:?}",
        operation, err
    );
    RegisterError::Backend
}

fn unexpected_driver_init_error(err: EngineError, operation: &'static str) -> DriverInitError {
    debug_assert!(
        false,
        "unexpected init-phase engine error in {}: {:?}",
        operation, err
    );
    DriverInitError::Backend
}

fn unexpected_runtime_error(err: EngineError, operation: &'static str) -> RuntimeError {
    debug_assert!(
        false,
        "unexpected runtime-phase engine error in {}: {:?}",
        operation, err
    );
    RuntimeError::Backend
}
