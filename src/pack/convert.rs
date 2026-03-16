use crate::model::Rgb48;

/// Converts a driver-native sample into 16-bit linear wire channels.
pub trait WireColor {
    fn to_wire(&self) -> [u16; 3];
    fn is_off(&self) -> bool;
}

impl WireColor for Rgb48 {
    #[inline(always)]
    fn to_wire(&self) -> [u16; 3] {
        [self.r, self.g, self.b]
    }

    #[inline(always)]
    fn is_off(&self) -> bool {
        (self.r | self.g | self.b) == 0
    }
}
