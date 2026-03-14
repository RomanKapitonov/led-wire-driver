use crate::model::Rgb24;

/// Converts a driver-native sample into 16-bit linear wire channels.
pub trait ToWire {
    fn to_wire(&self) -> [u16; 3];
}

impl ToWire for Rgb24 {
    fn to_wire(&self) -> [u16; 3] {
        let expand = |v: u8| ((v as u16) << 8) | (v as u16);
        [expand(self.r), expand(self.g), expand(self.b)]
    }
}

/// Fast off-path detection used to short-circuit black writes.
pub trait IsOff {
    fn is_off(&self) -> bool;
}

impl IsOff for Rgb24 {
    fn is_off(&self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }
}
