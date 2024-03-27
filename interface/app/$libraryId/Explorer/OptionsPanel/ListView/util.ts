export function getSizes<T extends { [key: string]: number }>(sizes: T) {
	return (Object.entries(sizes) as [keyof T, T[keyof T]][]).sort((a, b) => a[1] - b[1]);
}
