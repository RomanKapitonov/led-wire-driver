use crate::model::{BackendChannelId, FrameEpoch, PixelLayout};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::driver) struct ChannelState {
    pub(in crate::driver) backend_channel: BackendChannelId,
    pub(in crate::driver) pixel_count: usize,
    pub(in crate::driver) layout: PixelLayout,
    /// Precomputed: pixel_count * 3 bytes (WS28xx always 24-bit/pixel).
    pub(in crate::driver) wire_byte_count: usize,
    pub(in crate::driver) frame_phase: FrameEpoch,
}

impl ChannelState {
    pub(in crate::driver) fn new(
        backend_channel: BackendChannelId,
        pixel_count: u16,
        layout: PixelLayout,
    ) -> Self {
        Self {
            backend_channel,
            pixel_count: pixel_count as usize,
            layout,
            wire_byte_count: (pixel_count as usize) * 3,
            frame_phase: FrameEpoch::ZERO,
        }
    }

    pub(in crate::driver) fn advance_phase(&mut self) {
        self.frame_phase = self.frame_phase.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_byte_count_precomputed() {
        let s = ChannelState::new(BackendChannelId::new(0), 10, PixelLayout::Grb);
        assert_eq!(s.wire_byte_count, 30);
    }

    #[test]
    fn advance_phase_increments() {
        let mut s = ChannelState::new(BackendChannelId::new(0), 5, PixelLayout::Rgb);
        assert_eq!(s.frame_phase, FrameEpoch::ZERO);
        s.advance_phase();
        assert_eq!(s.frame_phase, FrameEpoch::from_raw(1));
    }
}
