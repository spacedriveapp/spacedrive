export function bytesToNumber(bytes: number[]) {
	return bytes.reduce((acc, curr, i) => acc + curr * Math.pow(256, bytes.length - i - 1), 0);
}
