import * as fs from 'node:fs/promises'
import * as os from 'node:os'
import * as path from 'node:path'
import { env } from 'node:process'

import * as semver from 'semver'

import { symlinkSharedLibsLinux, copyWindowsDLLs } from './shared.mjs'

/**
 * @param {string} root
 * @param {string} nativeDeps
 * @param {string[]} args
 * @returns {Promise<string[]>}
 */
export async function patchTauri(root, nativeDeps, args) {
	if (args.findIndex(e => e === '-c' || e === '--config') !== -1) {
		throw new Error('Custom tauri build config is not supported.')
	}

	// Location for desktop app tauri code
	const tauriRoot = path.join(root, 'apps', 'desktop', 'src-tauri')

	const osType = os.type()
	const tauriPatch = {
		tauri: {
			bundle: {
				macOS: {
					minimumSystemVersion: '',
				},
				resources:
					osType === 'Windows_NT'
						? await copyWindowsDLLs(root, nativeDeps)
						: osType === 'Linux'
						? await symlinkSharedLibsLinux(root, nativeDeps)
						: [],
			},
		},
	}

	if (osType === 'Darwin') {
		// ARM64 support was added in macOS 11, but we need at least 11.2 due to our ffmpeg build
		const macOSArm64MinimumVersion = '11.2'

		let macOSMinimumVersion = (
			await fs.readFile(path.join(tauriRoot, 'tauri.conf.json'), 'utf-8').then(JSON.parse)
		)?.tauri?.bundle?.macOS?.minimumSystemVersion

		const targets = args
			.filter((_, index, args) => {
				if (index === 0) return false
				const previous = args[index - 1]
				return previous === '-t' || previous === '--target'
			})
			.flatMap(target => target.split(','))

		if (
			(targets.includes('aarch64-apple-darwin') ||
				(targets.length === 0 && process.arch === 'arm64')) &&
			(macOSMinimumVersion == null ||
				semver.lt(
					semver.coerce(macOSMinimumVersion) ?? macOSArm64MinimumVersion,
					macOSArm64MinimumVersion
				))
		) {
			macOSMinimumVersion = macOSArm64MinimumVersion
			console.log(
				`aarch64-apple-darwin target detected, setting minimum system version to ${macOSMinimumVersion}`
			)
		}

		if (macOSMinimumVersion) {
			env.MACOSX_DEPLOYMENT_TARGET = macOSMinimumVersion
			tauriPatch.tauri.bundle.macOS.minimumSystemVersion = macOSMinimumVersion
		} else {
			throw new Error('No minimum macOS version detected, please review tauri.conf.json')
		}
	}

	const tauriPatchConf = path.join(tauriRoot, 'tauri.conf.patch.json')
	await fs.writeFile(tauriPatchConf, JSON.stringify(tauriPatch, null, 2))

	// Modify args to load patched tauri config
	args.splice(1, 0, '-c', tauriPatchConf)

	// Files to be removed
	return [
		...tauriPatch.tauri.bundle.resources.map(file => path.resolve(tauriRoot, file)),
		tauriPatchConf,
	]
}
