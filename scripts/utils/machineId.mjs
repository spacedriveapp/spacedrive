import { exec as execCb } from 'node:child_process'
import * as os from 'node:os'
import { env } from 'node:process'
import { promisify } from 'node:util'

const __debug = env.NODE_ENV === 'debug'

/** @type {'musl' | 'glibc'} */
let libc = 'glibc'
if (os.type() === 'Linux') {
	try {
		const exec = promisify(execCb)
		if ((await exec('ldd /bin/ls')).stdout.includes('musl')) {
			libc = 'musl'
		}
	} catch (error) {
		if (__debug) {
			console.warn(`Failed to check libc type`)
			console.error(error)
		}
	}
}

/** @type {Record<string, string>} */
const OS_TYPE = {
	darwin: 'Darwin',
	windows: 'Windows_NT',
	linux: 'Linux',
}

/** @returns {['Darwin' | 'Windows_NT', 'x86_64' | 'aarch64'] | ['Linux', 'x86_64' | 'aarch64', 'musl' | 'glibc']} */
export function getMachineId() {
	let _os, _arch
	let _libc = libc

	/**
	 * Supported TARGET_TRIPLE:
	 * x86_64-apple-darwin
	 * aarch64-apple-darwin
	 * x86_64-pc-windows-msvc
	 * aarch64-pc-windows-msvc
	 * x86_64-unknown-linux-gnu
	 * x86_64-unknown-linux-musl
	 * aarch64-unknown-linux-gnu
	 * aarch64-unknown-linux-musl
	 */
	if (env.TARGET_TRIPLE) {
		const target = env.TARGET_TRIPLE.split('-')
		_os = OS_TYPE[target[2] ?? '']
		_arch = target[0]
		if (_os === 'Linux') _libc = target[3]?.includes('musl') ? 'musl' : 'glibc'
	} else {
		// Current machine identifiers
		_os = os.type()
		_arch = os.machine()
		if (_arch === 'arm64') _arch = 'aarch64'
	}

	if (_arch !== 'x86_64' && _arch !== 'aarch64') throw new Error(`Unsuported architecture`)

	if (_os === 'Linux') {
		return [_os, _arch, _libc]
	} else if (_os !== 'Darwin' && _os !== 'Windows_NT') {
		throw new Error(`Unsuported OS`)
	}

	return [_os, _arch]
}
