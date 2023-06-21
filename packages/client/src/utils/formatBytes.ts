export function bytesToNumber(bytes: number[]) {
	return bytes.reduce((acc, curr, i) => acc + curr * Math.pow(256, bytes.length - i - 1), 0);
}

export function formatBytes(bytes: number[], decimals = 2) {
	const bytesNum = bytesToNumber(bytes);

	if (bytesNum === 0) return '0 Bytes';

	const k = 1024;
	const dm = decimals < 0 ? 0 : decimals;
	const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];

	const i = Math.floor(Math.log(bytesNum) / Math.log(k));

	return parseFloat((bytesNum / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}
