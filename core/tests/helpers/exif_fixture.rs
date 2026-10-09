//! Hand-built JPEGs carrying only an EXIF segment, for tests that need a
//! capture time or a place without a real photo in the tree.

/// A JPEG that is only a SOI, an APP1 EXIF segment and a trailing payload:
/// enough for the EXIF reader, which walks markers and never decodes.
/// `date` is `YYYY:MM:DD HH:MM:SS`; `gps` is signed decimal degrees.
pub fn exif_jpeg(seed: u8, date: Option<&str>, gps: Option<(f64, f64)>) -> Vec<u8> {
	fn entry(tiff: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: [u8; 4]) {
		tiff.extend_from_slice(&tag.to_be_bytes());
		tiff.extend_from_slice(&kind.to_be_bytes());
		tiff.extend_from_slice(&count.to_be_bytes());
		tiff.extend_from_slice(&value);
	}
	fn dms(degrees: f64) -> [u8; 24] {
		let abs = degrees.abs();
		let d = abs.floor();
		let m = ((abs - d) * 60.0).floor();
		let s = ((abs - d) * 60.0 - m) * 60.0;
		let mut out = [0u8; 24];
		for (i, (num, den)) in [(d as u32, 1u32), (m as u32, 1), ((s * 1000.0) as u32, 1000)]
			.into_iter()
			.enumerate()
		{
			out[i * 8..i * 8 + 4].copy_from_slice(&num.to_be_bytes());
			out[i * 8 + 4..i * 8 + 8].copy_from_slice(&den.to_be_bytes());
		}
		out
	}

	let entries = date.is_some() as u32 + gps.is_some() as u32;
	let ifd0_len = 2 + 12 * entries + 4;
	let mut tiff = b"MM\x00\x2a".to_vec();
	tiff.extend_from_slice(&8u32.to_be_bytes());
	tiff.extend_from_slice(&(entries as u16).to_be_bytes());
	let date_offset = 8 + ifd0_len;
	let gps_offset = date_offset + if date.is_some() { 20 } else { 0 };
	if date.is_some() {
		entry(&mut tiff, 0x0132, 2, 20, date_offset.to_be_bytes());
	}
	if gps.is_some() {
		entry(&mut tiff, 0x8825, 4, 1, gps_offset.to_be_bytes());
	}
	tiff.extend_from_slice(&0u32.to_be_bytes());
	if let Some(date) = date {
		assert_eq!(date.len(), 19);
		tiff.extend_from_slice(date.as_bytes());
		tiff.push(0);
	}
	if let Some((lat, lon)) = gps {
		let rationals = gps_offset + 2 + 12 * 4 + 4;
		tiff.extend_from_slice(&4u16.to_be_bytes());
		let lat_ref = if lat < 0.0 { b"S\0\0\0" } else { b"N\0\0\0" };
		let lon_ref = if lon < 0.0 { b"W\0\0\0" } else { b"E\0\0\0" };
		entry(&mut tiff, 0x0001, 2, 2, *lat_ref);
		entry(&mut tiff, 0x0002, 5, 3, rationals.to_be_bytes());
		entry(&mut tiff, 0x0003, 2, 2, *lon_ref);
		entry(&mut tiff, 0x0004, 5, 3, (rationals + 24).to_be_bytes());
		tiff.extend_from_slice(&0u32.to_be_bytes());
		tiff.extend_from_slice(&dms(lat));
		tiff.extend_from_slice(&dms(lon));
	}

	let mut payload = b"Exif\x00\x00".to_vec();
	payload.extend_from_slice(&tiff);
	let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
	out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
	out.extend_from_slice(&payload);
	out.extend(std::iter::repeat_n(seed, 1000 + seed as usize * 7));
	out
}
