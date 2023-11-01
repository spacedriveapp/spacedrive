// Suffixes
export const PROTOC_SUFFIX = {
	Linux: {
		i386: 'linux-x86_32',
		i686: 'linux-x86_32',
		x86_64: 'linux-x86_64',
		aarch64: 'linux-aarch_64',
	},
	Darwin: {
		x86_64: 'osx-x86_64',

		aarch64: 'osx-aarch_64',
	},
	Windows_NT: {
		i386: 'win32',
		i686: 'win32',
		x86_64: 'win64',
	},
}

export const PDFIUM_SUFFIX = {
	Linux: {
		x86_64: {
			musl: 'linux-musl-x64',
			glibc: 'linux-x64',
		},
		aarch64: 'linux-arm64',
	},
	Darwin: {
		x86_64: 'mac-x64',
		aarch64: 'mac-arm64',
	},
	Windows_NT: {
		x86_64: 'win-x64',
		aarch64: 'win-arm64',
	},
}

export const FFMPEG_SUFFFIX = {
	Linux: {
		x86_64: 'x86_64',
		aarch64: 'aarch64',
	},
	Darwin: {
		x86_64: 'x86_64',
		aarch64: 'arm64',
	},
	Windows_NT: {
		x86_64: 'x86_64',
		aarch64: 'aarch64',
	},
}

export const FFMPEG_WORKFLOW = {
	Linux: 'ffmpeg.yml',
	Darwin: 'ffmpeg-macos.yml',
	Windows_NT: 'ffmpeg.yml',
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
