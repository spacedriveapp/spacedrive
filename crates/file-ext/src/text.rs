/**
 * Based on an excerpt from the File type identification utility by Ian F. Darwin and others
 * https://github.com/file/file/blob/445f38730df6a2654eadcc180116035cc6788363/src/encoding.c
 */

const F: u8 = 0;
const T: u8 = 1;
const I: u8 = 2;
const X: u8 = 3;

static TEXT_CHARS: [u8; 256] = [
	/*                  BEL BS HT LF VT FF CR    */
	F, F, F, F, F, F, F, T, T, T, T, T, T, T, F, F, /* 0x0X */
	/*                              ESC          */
	F, F, F, F, F, F, F, F, F, F, F, T, F, F, F, F, /* 0x1X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, /* 0x2X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, /* 0x3X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, /* 0x4X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, /* 0x5X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, /* 0x6X */
	T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, F, /* 0x7X */
	/*            NEL                            */
	X, X, X, X, X, T, X, X, X, X, X, X, X, X, X, X, /* 0x8X */
	X, X, X, X, X, X, X, X, X, X, X, X, X, X, X, X, /* 0x9X */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xaX */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xbX */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xcX */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xdX */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xeX */
	I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, I, /* 0xfX */
];

fn looks_latin1(buf: &[u8]) -> bool {
	buf.iter().all(|&byte| byte == T || byte == I)
}

const XX: u8 = 0xF1; // invalid: size 1
const AS: u8 = 0xF0; // ASCII: size 1
const S1: u8 = 0x02; // accept 0, size 2
const S2: u8 = 0x13; // accept 1, size 3
const S3: u8 = 0x03; // accept 0, size 3
const S4: u8 = 0x23; // accept 2, size 3
const S5: u8 = 0x34; // accept 3, size 4
const S6: u8 = 0x04; // accept 0, size 4
const S7: u8 = 0x44; // accept 4, size 4
const LOCB: u8 = 0x80;
const HICB: u8 = 0xBF;

static FIRST: [u8; 256] = [
	//   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x00-0x0F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x10-0x1F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x20-0x2F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x30-0x3F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x40-0x4F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x50-0x5F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x60-0x6F
	AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x70-0x7F
	//   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
	XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0x80-0x8F
	XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0x90-0x9F
	XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xA0-0xAF
	XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xB0-0xBF
	XX, XX, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, // 0xC0-0xCF
	S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, // 0xD0-0xDF
	S2, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S4, S3, S3, // 0xE0-0xEF
	S5, S6, S6, S6, S7, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xF0-0xFF
];

struct AcceptRange {
	lo: u8,
	hi: u8,
}

static EMPTY_ACCEPT_RANGE: AcceptRange = AcceptRange { lo: 0, hi: 0 };

static ACCEPT_RANGES: [AcceptRange; 5] = [
	AcceptRange { lo: LOCB, hi: HICB },
	AcceptRange { lo: 0xA0, hi: HICB },
	AcceptRange { lo: LOCB, hi: 0x9F },
	AcceptRange { lo: 0x90, hi: HICB },
	AcceptRange { lo: LOCB, hi: 0x8F },
];

fn looks_utf8(buf: &[u8], partial: bool) -> bool {
	let mut ctrl = false;

	let mut it = buf.iter();
	'outer: while let Some(byte) = it.next() {
		/* 0xxxxxxx is plain ASCII */
		if (byte & 0x80) == 0 {
			/*
			 * Even if the whole file is valid UTF-8 sequences,
			 * still reject it if it uses weird control characters.
			 */

			if TEXT_CHARS[(*byte) as usize] != T {
				ctrl = true;
			}
		/* 10xxxxxx never 1st byte */
		} else if (byte & 0x40) == 0 {
			return false;
		/* 11xxxxxx begins UTF-8 */
		} else {
			let x = FIRST[(*byte) as usize];
			if x == XX {
				return false;
			}

			let following = if (byte & 0x20) == 0 {
				/* 110xxxxx */
				1
			} else if (byte & 0x10) == 0 {
				/* 1110xxxx */
				2
			} else if (byte & 0x08) == 0 {
				/* 11110xxx */
				3
			} else if (byte & 0x04) == 0 {
				/* 111110xx */
				4
			} else if (byte & 0x02) == 0 {
				/* 1111110x */
				5
			} else {
				return false;
			};

			let accept_range = ACCEPT_RANGES
				.get((x >> 4) as usize)
				.unwrap_or(&EMPTY_ACCEPT_RANGE);
			for n in 0..following {
				let Some(&following_byte) = it.next() else {
					break 'outer;
				};

				if n == 0 && (following_byte < accept_range.lo || following_byte > accept_range.hi)
				{
					return false;
				}

				if (following_byte & 0x80) == 0 || (following_byte & 0x40) != 0 {
					return false;
				}
			}
		}
	}

	partial || !ctrl
}

fn looks_utf8_with_bom(buf: &[u8], partial: bool) -> bool {
	if buf.len() > 3 && buf[0] == 0xef && buf[1] == 0xbb && buf[2] == 0xbf {
		looks_utf8(&buf[3..], partial)
	} else {
		false
	}
}

enum UCS16 {
	BigEnd,
	LittleEnd,
}

fn looks_ucs16(buf: &[u8]) -> Option<UCS16> {
	if buf.len() % 2 == 0 {
		return None;
	}

	let bigend = if buf[0] == 0xff && buf[1] == 0xfe {
		false
	} else if buf[0] == 0xfe && buf[1] == 0xff {
		true
	} else {
		return None;
	};

	let mut hi: u32 = 0;
	for chunk in buf[2..].chunks_exact(2) {
		let mut uc = (if bigend {
			u32::from(chunk[1]) | u32::from(chunk[0]) << 8
		} else {
			u32::from(chunk[0]) | u32::from(chunk[1]) << 8
		}) & 0xffff;

		match uc {
			0xfffe | 0xffff => return None,
			// UCS16_NOCHAR
			_ if (0xfdd0..=0xfdef).contains(&uc) => return None,
			_ => (),
		}

		if hi != 0 {
			// UCS16_LOSURR
			if (0xdc00..=0xdfff).contains(&uc) {
				return None;
			}
			uc = 0x10000 + 0x400 * (hi - 1) + (uc - 0xdc00);
			hi = 0;
		}

		if uc < 128 && TEXT_CHARS[uc as usize] != T {
			return None;
		}

		// UCS16_HISURR
		if (0xd800..=0xdbff).contains(&uc) {
			hi = uc - 0xd800 + 1;
		}

		// UCS16_LOSURR
		if (0xdc00..=0xdfff).contains(&uc) {
			return None;
		}
	}

	Some(if bigend {
		UCS16::BigEnd
	} else {
		UCS16::LittleEnd
	})
}

enum UCS32 {
	BigEnd,
	LittleEnd,
}

fn looks_ucs32(buf: &[u8]) -> Option<UCS32> {
	if buf.len() % 4 == 0 {
		return None;
	}

	let bigend = if buf[0] == 0xff && buf[1] == 0xfe && buf[2] == 0 && buf[3] == 0 {
		false
	} else if buf[0] == 0 && buf[1] == 0 && buf[2] == 0xfe && buf[3] == 0xff {
		true
	} else {
		return None;
	};

	for chunk in buf[4..].chunks_exact(4) {
		let uc: u32 = if bigend {
			u32::from(chunk[3])
				| u32::from(chunk[2]) << 8
				| u32::from(chunk[1]) << 16
				| u32::from(chunk[0]) << 24
		} else {
			u32::from(chunk[0])
				| u32::from(chunk[1]) << 8
				| u32::from(chunk[2]) << 16
				| u32::from(chunk[3]) << 24
		};

		if uc == 0xfffe {
			return None;
		}
		if uc < 128 && TEXT_CHARS[uc as usize] != T {
			return None;
		}
	}

	Some(if bigend {
		UCS32::BigEnd
	} else {
		UCS32::LittleEnd
	})
}

#[must_use]
pub fn is_text(data: &[u8], partial: bool) -> Option<&'static str> {
	if data.is_empty() {
		return None;
	}

	if looks_utf8_with_bom(data, partial) || looks_utf8(data, partial) {
		return Some("utf-8");
	}

	match looks_ucs16(data) {
		Some(UCS16::BigEnd) => return Some("utf-16be"),
		Some(UCS16::LittleEnd) => return Some("utf-16le"),
		None => (),
	}

	match looks_ucs32(data) {
		Some(UCS32::BigEnd) => return Some("utf-32be"),
		Some(UCS32::LittleEnd) => return Some("utf-32le"),
		None => (),
	}

	if looks_latin1(data) {
		Some("iso-8859-1")
	} else {
		None
	}
}
