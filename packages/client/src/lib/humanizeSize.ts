// Inspired by: https://github.com/75lb/byte-size

const DECIMAL_UNITS = [
	{ short: 'B', long: 'byte', from: 0n },
	{ short: 'kB', long: 'kilobyte', from: 1000n },
	{ short: 'MB', long: 'megabyte', from: 1000n ** 2n },
	{ short: 'GB', long: 'gigabyte', from: 1000n ** 3n },
	{ short: 'TB', long: 'terabyte', from: 1000n ** 4n },
	{ short: 'PB', long: 'petabyte', from: 1000n ** 5n },
	{ short: 'EB', long: 'exabyte', from: 1000n ** 6n },
	{ short: 'ZB', long: 'zettabyte', from: 1000n ** 7n },
	{ short: 'YB', long: 'yottabyte', from: 1000n ** 8n },
	{ short: 'RB', long: 'ronnabyte', from: 1000n ** 9n },
	{ short: 'QB', long: 'quettabyte', from: 1000n ** 10n }
];

const BINARY_UNITS = [
	DECIMAL_UNITS[0],
	{ short: 'KiB', long: 'kibibyte', from: 1024n },
	{ short: 'MiB', long: 'mebibyte', from: 1024n ** 2n },
	{ short: 'GiB', long: 'gibibyte', from: 1024n ** 3n },
	{ short: 'TiB', long: 'tebibyte', from: 1024n ** 4n },
	{ short: 'PiB', long: 'pebibyte', from: 1024n ** 5n },
	{ short: 'EiB', long: 'exbibyte', from: 1024n ** 6n },
	{ short: 'ZiB', long: 'zebibyte', from: 1024n ** 7n },
	{ short: 'YiB', long: 'yobibyte', from: 1024n ** 8n }
];

const BYTE_TO_BIT = {
	B: 'b',
	byte: 'bit',
	kB: 'kb',
	KiB: 'Kib',
	kilobyte: 'kilobit',
	kibibyte: 'kibibit',
	MB: 'Mb',
	MiB: 'Mib',
	megabyte: 'megabit',
	mebibyte: 'mebibit',
	GB: 'Gb',
	GiB: 'Gib',
	gigabyte: 'gigabit',
	gibibyte: 'gibibit',
	TB: 'Tb',
	TiB: 'Tib',
	terabyte: 'terabit',
	tebibyte: 'tebibit',
	PB: 'Pb',
	PiB: 'Pib',
	petabyte: 'petabit',
	pebibyte: 'pebibit',
	EB: 'Eb',
	EiB: 'Eib',
	exabyte: 'exabit',
	exbibyte: 'exbibit',
	ZB: 'Zb',
	ZiB: 'Zib',
	zettabyte: 'zettabit',
	zebibyte: 'zebibit',
	YB: 'Yb',
	YiB: 'Yib',
	yottabyte: 'yottabit',
	yobibyte: 'yobibit',
	RB: 'Rb',
	ronnabyte: 'ronnabit',
	QB: 'Qb',
	quettabyte: 'quettabit'
};

const getBaseUnit = (n: bigint, map: typeof DECIMAL_UNITS | typeof BINARY_UNITS) => {
	const s = n.toString(10);
	const log10 = s.length + Math.log10(Number('0.' + s.substring(0, 15)));
	const index = (log10 / 3) | 0;
	return map[index] ?? (map[map.length - 1] as Exclude<(typeof map)[number], undefined>);
};

export function bytesToNumber(bytes: string[] | number[] | bigint[]) {
	return bytes
		.map((b) => (typeof b === 'bigint' ? b : BigInt(b)))
		.reduce((acc, curr, i) => acc + curr * 256n ** BigInt(bytes.length - i - 1));
}

export interface ByteSizeOpts {
	is_bit?: boolean;
	locales?: string | string[];
	precision?: number;
	base_unit?: 'decimal' | 'binary';
	use_plural?: boolean;
	no_thousands?: boolean;
}

/**
 * Returns an object with the spec `{ unit: string, long: string, bytes: bigint, value: number }`. The returned object defines a `toString` method meaning it can be used in any string context.
 *
 * @param value - The bytes value to convert.
 * @param options - Optional config.
 * @param options.is_bit - Use bit units names instead of byte units. Defaults to `false`.
 * @param options.locales - The locale to use for number formatting (e.g. `'de-DE'`). Defaults to your system locale. Passed directed into [Intl.NumberFormat()](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/NumberFormat/NumberFormat).
 * @param options.precision - Number of decimal places. Defaults to `1`.
 * @param options.base_unit - The base unit to use. Defaults to `'decimal'`.
 * @param options.use_plural - Use plural unit names when necessary. Defaults to `true`.
 * @param options.no_thousands - Do not convert TB to thousands. Defaults to `true`.
 */
export const humanizeSize = (
	value: null | string | number | bigint | string[] | number[] | bigint[] | undefined,
	{
		is_bit = false,
		precision = 1,
		locales,
		base_unit = 'decimal',
		use_plural = true,
		no_thousands = true
	}: ByteSizeOpts = {}
) => {
	if (value == null) value = 0n;
	if (Array.isArray(value)) value = bytesToNumber(value);
	else if (typeof value !== 'bigint') value = BigInt(value);
	const [isNegative, bytes] =
		typeof value === 'number'
			? value < 0
				? // Note: These magic shift operations internally convert value from f64 to u32
					[true, BigInt(-value >>> 0)]
				: [false, BigInt(value >>> 0)]
			: value < 0n
				? [true, -value]
				: [false, value];

	const unit = getBaseUnit(bytes, base_unit === 'decimal' ? DECIMAL_UNITS : BINARY_UNITS);
	const defaultFormat = new Intl.NumberFormat(locales, {
		style: 'decimal',
		minimumFractionDigits: precision,
		maximumFractionDigits: precision
	});
	const precisionFactor = 10 ** precision;
	value =
		unit.from === 0n
			? Number(bytes)
			: Number((bytes * BigInt(precisionFactor)) / unit.from) / precisionFactor;
	const plural = use_plural && value !== 1 ? 's' : '';

	//TODO: Improve this
	// Convert to thousands when short is TB to show correct progress value
	//i.e 2.5 TB = 2500
	if (unit.short === 'TB' && !no_thousands) {
		value = value * 1000;
	}

	return {
		unit: is_bit ? BYTE_TO_BIT[unit.short as keyof typeof BYTE_TO_BIT] : unit.short,
		long: is_bit ? BYTE_TO_BIT[unit.long as keyof typeof BYTE_TO_BIT] : unit.long,
		bytes,
		value: (isNegative ? -1 : 1) * value,
		toString() {
			return `${defaultFormat.format(this.value)} ${this.unit}${plural}`;
		}
	};
};
