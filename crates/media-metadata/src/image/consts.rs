use exif::Tag;

/// Used for converting DMS to decimal coordinates, and is the amount to divide by.
///
/// # Examples:
///
/// ```
/// use sd_media_metadata::image::DMS_DIVISION;
///
/// let latitude = [53_f64, 19_f64, 35.11_f64]; // in DMS
/// latitude.iter().zip(DMS_DIVISION.iter());
/// ```
pub const DMS_DIVISION: [f64; 3] = [1_f64, 60_f64, 3600_f64];

/// The amount of significant figures we wish to retain after the decimal point.
///
/// This is currrently 8 digits (after the integer) as that is precise enough for most
/// applications.
///
/// This is calculated with `10^n`, where `n` is the desired amount of SFs.
pub const DECIMAL_SF: f64 = 100_000_000_f64;

/// All possible time tags, to be zipped with [`OFFSET_TAGS`]
pub const TIME_TAGS: [Tag; 3] = [Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized];

/// All possible time offset tags, to be zipped with [`TIME_TAGS`]
pub const OFFSET_TAGS: [Tag; 3] = [
	Tag::OffsetTime,
	Tag::OffsetTimeOriginal,
	Tag::OffsetTimeDigitized,
];
