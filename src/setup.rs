use crate::model::{BackendChannelId, PixelLayout};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelSetup {
    pub backend_channel: BackendChannelId,
    pub pixel_count: u16,
    pub layout: PixelLayout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_setup_fields() {
        let s = ChannelSetup {
            backend_channel: BackendChannelId::new(3),
            pixel_count: 60,
            layout: PixelLayout::Grb,
        };
        assert_eq!(s.pixel_count, 60);
    }
}
