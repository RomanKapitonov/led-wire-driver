use crate::api::backend::BackendError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EngineStateExpectation {
    MustBeRegistering,
    MustBeReady,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendContractViolation {
    WrongChannelReturned,
    WrongTargetLength,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    // Engine lifecycle / phase misuse.
    InvalidState(EngineStateExpectation),

    // Channel registration / addressing rejections.
    ChannelOutOfRange,
    ChannelNotRegistered,
    ChannelAlreadyRegistered,
    ConfigurationLimitExceeded,

    // Data-shape / registration validation failures.
    SourceLengthMismatch {
        expected_pixels: usize,
        actual_pixels: usize,
    },
    WriteBusy,
    BackendContractViolation(BackendContractViolation),

    // Backend/runtime transport failure.
    Backend(BackendError),
}
