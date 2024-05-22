export function getSizes<T extends { [key: string]: number }>(sizes: T) {
	const sizesArr = (Object.entries(sizes) as [keyof T, T[keyof T]][]).sort((a, b) => a[1] - b[1]);

	// Map fo size to index
	const indexMap = new Map<keyof T, number>();

	// Map of index to size
	const sizeMap = new Map<number, keyof T>();

	for (let i = 0; i < sizesArr.length; i++) {
		const size = sizesArr[i];
		if (!size) continue;
		indexMap.set(size[0], i);
		sizeMap.set(i, size[0]);
	}

	return { indexMap, sizeMap };
}
