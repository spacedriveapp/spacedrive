// Suffixes

export const NATIVE_DEPS_SUFFIX = {
	Linux: {
		x86_64: 'x86_64-linux-gnu',
		aarch64: 'aarch64-linux-gnu',
	},
	Darwin: {
		x86_64: 'x86_64-darwin-apple',
		aarch64: 'aarch64-darwin-apple',
	},
	Windows_NT: {
		x86_64: 'x86_64-windows-gnu',
		aarch64: 'aarch64-windows-gnu',
	},
}

export const NATIVE_DEPS_WORKFLOW = {
	Linux: 'native-deps.yml',
	Darwin: 'native-deps.yml',
	Windows_NT: 'native-deps.yml',
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

/**
 * @param {Record<string, unknown>} suffixes
 * @param {string[]} identifiers
 * @returns {RegExp?}
 */
export function getSuffix(suffixes, identifiers) {
	const suffix = getConst(suffixes, identifiers)
	return suffix ? new RegExp(`${suffix}(\\.[^\\.]+)*$`) : null
}
