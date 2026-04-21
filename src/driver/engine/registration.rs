use super::{
    EngineError,
    channel_state::ChannelState,
    mask::ChannelMask,
};
use crate::DRIVER_MAX_CHANNELS;

pub(in crate::driver) struct RegistrationTable {
    records: [Option<ChannelState>; DRIVER_MAX_CHANNELS],
    registered_mask: ChannelMask,
    count: usize,
}

impl RegistrationTable {
    pub(in crate::driver) fn new(
        records: [Option<ChannelState>; DRIVER_MAX_CHANNELS],
        count: usize,
    ) -> Self {
        let mut mask = ChannelMask::EMPTY;
        for (i, rec) in records.iter().enumerate() {
            if rec.is_some() {
                mask = mask.union(ChannelMask::single(i));
            }
        }
        Self { records, registered_mask: mask, count }
    }

    pub(in crate::driver) fn record(&self, index: usize) -> Result<&ChannelState, EngineError> {
        self.records
            .get(index)
            .and_then(|s| s.as_ref())
            .ok_or(EngineError::InvalidChannel)
    }

    pub(in crate::driver) fn record_mut(&mut self, index: usize) -> Result<&mut ChannelState, EngineError> {
        self.records
            .get_mut(index)
            .and_then(|s| s.as_mut())
            .ok_or(EngineError::InvalidChannel)
    }

    pub(in crate::driver) fn registered_mask(&self) -> ChannelMask {
        self.registered_mask
    }

    pub(in crate::driver) fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BackendChannelId, PixelLayout};

    fn make_state(backend: u8, pixels: u16) -> ChannelState {
        ChannelState::new(BackendChannelId::new(backend), pixels, PixelLayout::Grb)
    }

    #[test]
    fn lookup_registered_channel() {
        let mut records = [const { None }; crate::DRIVER_MAX_CHANNELS];
        records[2] = Some(make_state(2, 60));
        let table = RegistrationTable::new(records, 1);
        assert_eq!(table.record(2).unwrap().pixel_count, 60);
    }

    #[test]
    fn lookup_unregistered_returns_err() {
        let records = [const { None }; crate::DRIVER_MAX_CHANNELS];
        let table = RegistrationTable::new(records, 0);
        assert!(table.record(0).is_err());
    }

    #[test]
    fn registered_mask_covers_occupied_slots() {
        let mut records = [const { None }; crate::DRIVER_MAX_CHANNELS];
        records[0] = Some(make_state(0, 10));
        records[3] = Some(make_state(3, 10));
        let table = RegistrationTable::new(records, 2);
        let mask = table.registered_mask();
        assert!(mask.contains(0));
        assert!(mask.contains(3));
        assert!(!mask.contains(1));
    }
}
