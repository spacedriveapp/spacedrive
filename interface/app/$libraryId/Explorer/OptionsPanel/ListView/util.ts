export function getSizeOptions<T extends string>(options: T[]) {
	return options.reduce(
		(acc, option, index) => {
			acc[option] = index;
			return acc;
		},
		{} as Record<T, number>
	);
}
