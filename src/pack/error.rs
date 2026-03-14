#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    SourceLengthMismatch {
        source_pixels: usize,
        target_pixels: usize,
    },
}
