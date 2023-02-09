export function arraysEqual<T>(a: T[], b: T[]) {
	if (a === b) return true;
	if (a == null || b == null) return false;
	if (a.length !== b.length) return false;

	return a.every((n, i) => b[i] === n);
}

export * from './objectKind';
