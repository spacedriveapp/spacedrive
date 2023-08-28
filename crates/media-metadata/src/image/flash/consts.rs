use super::FlashMode;

pub const FLASH_AUTO: [u32; 8] = [0x18, 0x19, 0x1d, 0x1f, 0x58, 0x59, 0x5d, 0x5f];
pub const FLASH_ENABLED: [u32; 7] = [0x08, 0x09, 0x0d, 0x0f, 0x49, 0x4d, 0x4f];
pub const FLASH_DISABLED: [u32; 4] = [0x10, 0x14, 0x30, 0x50];
pub const FLASH_FORCED: [u32; 3] = [0x41, 0x45, 0x47];

pub const FLASH_MODES: [(FlashMode, &[u32]); 4] = [
	(FlashMode::Auto, &FLASH_AUTO),
	(FlashMode::On, &FLASH_ENABLED),
	(FlashMode::Off, &FLASH_DISABLED),
	(FlashMode::Forced, &FLASH_FORCED),
];
