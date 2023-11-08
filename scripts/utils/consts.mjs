export const NATIVE_DEPS_URL =
	'https://github.com/spacedriveapp/native-deps/releases/latest/download'

export const NATIVE_DEPS_ASSETS = {
	Linux: {
		x86_64: {
			musl: 'native-deps-x86_64-linux-musl.tar.xz',
			glibc: 'native-deps-x86_64-linux-gnu.tar.xz',
		},
		aarch64: {
			musl: 'native-deps-aarch64-linux-musl.tar.xz',
			glibc: 'native-deps-aarch64-linux-gnu.tar.xz',
		},
	},
	Darwin: {
		x86_64: 'native-deps-x86_64-darwin-apple.tar.xz',
		aarch64: 'native-deps-aarch64-darwin-apple.tar.xz',
	},
	Windows_NT: {
		x86_64: 'native-deps-x86_64-windows-gnu.tar.xz ',
		aarch64: 'native-deps-aarch64-windows-gnu.tar.xz',
	},
}

/**
 * @param {Record<string, unknown>} constants
 * @param {string[]} identifiers
 * @returns {string?}
 */
export function getConst(constants, identifiers) {
	/** @type {string | Record<string, unknown>} */
	let constant = constants

	for (const id of identifiers) {
		constant = /** @type {string | Record<string, unknown>} */ (constant[id])
		if (!constant) return null
		if (typeof constant !== 'object') break
	}

	return typeof constant === 'string' ? constant : null
}
