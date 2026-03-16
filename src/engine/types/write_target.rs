use core::ptr::NonNull;

use crate::{engine::EngineError, model::PixelLayout};

use super::FrameEpoch;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WireSpan {
    pub addr: usize,
    pub size_bytes: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WireTarget {
    ptr: NonNull<u8>,
    len_bytes: u32,
}

impl WireTarget {
    pub fn from_span(span: WireSpan) -> Result<Self, EngineError> {
        if span.size_bytes != 0 && span.addr == 0 {
            return Err(EngineError::InvalidWireSpan);
        }

        let ptr = if span.size_bytes == 0 {
            NonNull::dangling()
        } else {
            NonNull::new(span.addr as *mut u8).ok_or(EngineError::InvalidWireSpan)?
        };

        Ok(Self {
            ptr,
            len_bytes: span.size_bytes,
        })
    }

    /// Executes `f` with a mutable byte view of this write target.
    ///
    /// Safety contract:
    /// - target validity/lifetime is guaranteed by backend-owned registration
    ///   and write-grant lifecycle;
    /// - the mutable slice must be used only for immediate in-call mutation.
    ///
    /// This method keeps the raw-pointer-to-slice conversion as the single
    /// unsafe boundary for driver write-target access.
    pub fn with_mut_bytes<R>(self, f: impl FnOnce(&mut [u8]) -> R) -> R {
        // SAFETY: pointer/length validity is guaranteed by backend write-grant
        // ownership and `WireTarget::from_span` construction checks.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len_bytes as usize) };
        f(bytes)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WritePlan {
    pub(crate) layout: PixelLayout,
    pub(crate) frame_phase: FrameEpoch,
    pub(crate) target: WireTarget,
}
