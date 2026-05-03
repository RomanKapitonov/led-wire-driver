use crate::backend::BackendError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) enum BackendContractViolation {
    WrongChannelReturned,
    WrongTargetLength,
    TransferCompleteWhileIdle,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) enum EngineError {
    InvalidChannel,
    WriteBusy,
    Backend(BackendError),
    BackendContractViolation(BackendContractViolation),
    SourceLengthMismatch {
        expected_pixels: usize,
        actual_pixels: usize,
    },
}
