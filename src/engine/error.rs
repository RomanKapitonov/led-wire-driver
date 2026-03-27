use crate::api::backend::BackendError;

/// Internal lifecycle expectation violated by an engine entry point.
///
/// These are maintenance/backstop errors: public driver typestate should
/// normally prevent callers from ever observing them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EngineStateExpectation {
    MustBeRegistering,
    MustBeReady,
}

/// Backend contract mismatch detected by the engine.
///
/// These indicate that the backend returned an invalid shape or reported an
/// event at an invalid time. They are intentionally distinct from genuine
/// backend transport faults.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendContractViolation {
    WrongChannelReturned,
    WrongTargetLength,
    TransferCompleteWhileIdle,
}

/// Internal engine operational error vocabulary.
///
/// Policy:
/// - `InvalidState(...)` means an engine entry point was reached in the wrong
///   lifecycle phase despite higher-level typestate/API guards
/// - `ConfigurationLimitExceeded` means an engine-owned structural limit was
///   exceeded during planning or mask accounting
/// - `BackendContractViolation(...)` means the backend violated a declared
///   driver/backend contract
/// - `Backend(...)` means a genuine backend-owned failure
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
