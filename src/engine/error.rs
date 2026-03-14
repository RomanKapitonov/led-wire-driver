use crate::api::backend::BackendError;

#[derive(Copy, Clone, Debug, PartialEq, Eq, defmt::Format)]
pub enum EngineError {
    // Channel registration / addressing rejections.
    ChannelOutOfRange,
    ChannelNotRegistered,
    ChannelAlreadyRegistered,

    // Data-shape / registration validation failures.
    InvalidWireSpan,
    SourceLengthMismatch {
        expected_pixels: usize,
        actual_pixels: usize,
    },
    WriteBusy,

    // Backend/runtime transport failure.
    Backend(BackendError),
}
