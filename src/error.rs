use crate::{backend::BackendError, model::BackendChannelId};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConfigureError {
    NoChannels,
    TooManyChannels,
    DuplicateBackendChannel(BackendChannelId),
    ChannelTooLarge,
    Backend(BackendError),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    InvalidChannel,
    Busy,
    Backend(BackendError),
    BackendContract,
    LengthMismatch { expected: usize, actual: usize },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    Backend(BackendError),
    BackendContract,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;

    #[test]
    fn configure_error_debug() {
        let _ = format!("{:?}", ConfigureError::DuplicateBackendChannel(BackendChannelId::new(0)));
    }

    #[test]
    fn write_error_variants() {
        let _ = format!("{:?}", WriteError::Busy);
        let _ = format!("{:?}", WriteError::InvalidChannel);
        let _ = format!("{:?}", WriteError::BackendContract);
    }

    #[test]
    fn service_error_debug() {
        let _ = format!("{:?}", ServiceError::BackendContract);
    }
}
