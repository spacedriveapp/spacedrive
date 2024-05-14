use exif::Tag;

/// Used for converting DMS to decimal coordinates, and is the amount to divide by.
///
/// # Examples:
///
/// ```
/// use sd_media_metadata::image::DMS_DIVISION;
///
/// let latitude = [53.0, 19.0, 35.11]; // in DMS
/// latitude.iter().zip(DMS_DIVISION.iter());
/// ```
pub const DMS_DIVISION: [f64; 3] = [1.0, 60.0, 3600.0];

/// The amount of significant figures we wish to retain after the decimal point.
///
/// This is currently 8 digits (after the integer) as that is precise enough for most
/// applications.
///
/// This is calculated with `10^n`, where `n` is the desired amount of SFs.
pub const DECIMAL_SF: f64 = 100_000_000.0;

/// All possible time tags, to be zipped with [`OFFSET_TAGS`]
pub const TIME_TAGS: [Tag; 3] = [Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized];

/// All possible time offset tags, to be zipped with [`TIME_TAGS`]
pub const OFFSET_TAGS: [Tag; 3] = [
	Tag::OffsetTime,
	Tag::OffsetTimeOriginal,
	Tag::OffsetTimeDigitized,
];

/// The Earth's maximum latitude (can also be negative, depending on if you're North or South of the Equator).
pub const LAT_MAX_POS: f64 = 90.0;

/// The Earth's maximum longitude (can also be negative depending on if you're East or West of the Prime meridian).
///
/// The negative value of this is known as the anti-meridian, and when combined they make a 360 degree circle around the Earth.
pub const LONG_MAX_POS: f64 = 180.0;

/// 125km. This is the Kármán line + a 25km additional padding just to be safe.
pub const ALT_MAX_HEIGHT: i32 = 125_000;

/// -1km. This should be adequate for even the Dead Sea on the Israeli border,
/// the lowest point on land (and much deeper).
pub const ALT_MIN_HEIGHT: i32 = -1000;

/// The maximum degrees that a direction can be (as a bearing, starting from 0 degrees)
pub const DIRECTION_MAX: i32 = 360;

pub const PLUSCODE_DIGITS: [char; 20] = [
	'2', '3', '4', '5', '6', '7', '8', '9', 'C', 'F', 'G', 'H', 'J', 'M', 'P', 'Q', 'R', 'V', 'W',
	'X',
];

pub const PLUSCODE_GRID_SIZE: f64 = 20.0;
