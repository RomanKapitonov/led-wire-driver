pub trait SpatialQuantizer {
    fn quantize(&mut self, value: u16, index: usize) -> u8;
}

#[derive(Default)]
pub struct NoSpatialQuantizer;

impl SpatialQuantizer for NoSpatialQuantizer {
    fn quantize(&mut self, value: u16, _index: usize) -> u8 {
        (value >> 8) as u8
    }
}

#[cfg(feature = "pack-sq-bayer")]
#[derive(Default)]
pub struct SpatialBayerQuantizer;

#[cfg(feature = "pack-sq-bayer")]
impl SpatialQuantizer for SpatialBayerQuantizer {
    fn quantize(&mut self, value: u16, index: usize) -> u8 {
        const THRESHOLD_8X8: [u16; 64] = [
            0, 32768, 8192, 40960, 2048, 34816, 10240, 43008, 49152, 16384, 57344, 24576, 51200,
            18432, 59392, 26624, 12288, 45056, 4096, 36864, 14336, 47104, 6144, 38912, 61440,
            28672, 53248, 20480, 63488, 30720, 55296, 22528, 3072, 35840, 11264, 44032, 1024,
            33792, 9216, 41984, 52224, 19456, 60416, 27648, 50176, 17408, 58368, 25600, 15360,
            48128, 7168, 39936, 13312, 46080, 5120, 37888, 64512, 31744, 56320, 23552, 62464,
            29696, 54272, 21504,
        ];

        let threshold = THRESHOLD_8X8[index % 64];
        let base = (value >> 8) as u8;
        let fraction = (value & 0x00FF) << 8;
        if fraction > threshold && base < 255 {
            base + 1
        } else {
            base
        }
    }
}
