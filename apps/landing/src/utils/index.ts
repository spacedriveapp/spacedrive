export function getWindow(): (Window & typeof globalThis) | null {
	return typeof window !== 'undefined' ? window : null;
}
