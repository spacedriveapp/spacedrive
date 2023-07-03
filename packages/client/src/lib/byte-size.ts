// Inspired by: https://github.com/75lb/byte-size

const DECIMAL_UNITS = [
	{ short: 'B', long: 'bytes', from: 0n },
	{ short: 'kB', long: 'kilobytes', from: 1000n },
	{ short: 'MB', long: 'megabytes', from: 1000n ** 2n },
	{ short: 'GB', long: 'gigabytes', from: 1000n ** 3n },
	{ short: 'TB', long: 'terabytes', from: 1000n ** 4n },
	{ short: 'PB', long: 'petabytes', from: 1000n ** 5n },
	{ short: 'EB', long: 'exabytes', from: 1000n ** 6n },
	{ short: 'ZB', long: 'zettabytes', from: 1000n ** 7n },
	{ short: 'YB', long: 'yottabytes', from: 1000n ** 8n },
	{ short: 'RB', long: 'ronnabyte', from: 1000n ** 9n },
	{ short: 'QB', long: 'quettabyte', from: 1000n ** 10n }
];

const getDecimalUnit = (n: bigint) => {
	const s = n.toString(10);
	const log10 = s.length + Math.log10(Number('0.' + s.substring(0, 15)));
	const index = (log10 / 3) | 0;
	return (
		DECIMAL_UNITS[index] ??
		(DECIMAL_UNITS[DECIMAL_UNITS.length - 1] as Exclude<
			(typeof DECIMAL_UNITS)[number],
			undefined
		>)
	);
};

function bytesToNumber(bytes: string[] | number[] | bigint[]) {
	return bytes
		.map((b) => (typeof b === 'bigint' ? b : BigInt(b)))
		.reduce((acc, curr, i) => acc + curr * 256n ** BigInt(bytes.length - i - 1));
}

export interface ByteSizeOpts {
	locales?: string | string[];
	precision: number;
}

/**
 * Returns an object with the spec `{ value: string, unit: string, long: string }`. The returned object defines a `toString` method meaning it can be used in any string context.
 *
 * @param value - The bytes value to convert.
 * @param options - Optional config.
 * @param options.locales - *Node >=13 or modern browser only - on earlier platforms this option is ignored*. The locale to use for number formatting (e.g. `'de-DE'`). Defaults to your system locale. Passed directed into [Intl.NumberFormat()](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/NumberFormat/NumberFormat).
 * @param options.precision - Number of decimal places. Defaults to `1`.
 */
export const byteSize = (
	value: string | number | bigint | string[] | number[] | bigint[],
	{ precision, locales }: ByteSizeOpts = { precision: 1 }
) => {
	const defaultFormat = new Intl.NumberFormat(locales, {
		style: 'decimal',
		minimumFractionDigits: precision,
		maximumFractionDigits: precision
	});

	if (Array.isArray(value)) value = bytesToNumber(value);
	else if (typeof value !== 'bigint') value = BigInt(value);
	const [prefix, bytes] = value < 0n ? ['-', -value] : ['', value];

	const unit = getDecimalUnit(bytes);
	const precisionFactor = 10 ** precision;
	return {
		unit: unit.short,
		long: unit.long,
		value:
			prefix +
			defaultFormat.format(
				unit.from === 0n
					? bytes
					: Number((bytes * BigInt(precisionFactor)) / unit.from) / precisionFactor
			),
		toString() {
			return `${this.value} ${this.unit}`;
		}
	};
};
