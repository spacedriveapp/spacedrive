#!/usr/bin/env node

import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { env, exit, umask } from 'node:process'
import { fileURLToPath } from 'node:url'

import { extractTo } from 'archive-wasm/src/fs.mjs'
import * as _mustache from 'mustache'

import { getConst, NATIVE_DEPS_URL, NATIVE_DEPS_ASSETS } from './utils/consts.mjs'
import { get } from './utils/fetch.mjs'
import { getMachineId } from './utils/machineId.mjs'
import { symlinkSharedLibsMacOS, symlinkSharedLibsLinux } from './utils/shared.mjs'
import { which } from './utils/which.mjs'

if (/^(msys|mingw|cygwin)$/i.test(env.OSTYPE ?? '')) {
	console.error(
		'Bash for windows is not supported, please interact with this repo from Powershell or CMD'
	)
	exit(255)
}

// @ts-expect-error
const mustache = /** @type {import("mustache")}  */ (_mustache.default)

// Limit file permissions
umask(0o026)

const __debug = env.NODE_ENV === 'debug'
const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

// NOTE: Must point to package root path
const __root = path.resolve(path.join(__dirname, '..'))

const bugWarn =
	'This is probably a bug, please open a issue with you system info at: ' +
	'https://github.com/spacedriveapp/spacedrive/issues/new/choose'

// Current machine identifiers
const machineId = getMachineId()

// Basic dependeny check
if ((await Promise.all([which('cargo'), which('rustc'), which('pnpm')])).some(found => !found)) {
	console.error(`Basic dependencies missing.
Make sure you have rust and pnpm installed:
https://rustup.rs
https://pnpm.io/installation

Also that you have run the setup script:
packages/scripts/${machineId[0] === 'Windows_NT' ? 'setup.ps1' : 'setup.sh'}
`)
}

// Directory where the native deps will be downloaded
const nativeDeps = path.join(__root, 'apps', '.deps')
await fs.rm(nativeDeps, { force: true, recursive: true })
await fs.mkdir(nativeDeps, { mode: 0o750, recursive: true })

try {
	console.log('Downloading Native dependencies...')

	const assetName = getConst(NATIVE_DEPS_ASSETS, machineId)
	if (assetName == null) throw new Error('NO_ASSET')

	const archiveData = await get(`${NATIVE_DEPS_URL}/${assetName}`)

	await extractTo(archiveData, nativeDeps, {
		chmod: 0o600,
		recursive: true,
		overwrite: true,
	})
} catch (e) {
	console.error(`Failed to download native dependencies. ${bugWarn}`)
	if (__debug) console.error(e)
	exit(1)
}

// Extra OS specific setup
try {
	if (machineId[0] === 'Linux') {
		console.log(`Symlink shared libs...`)
		symlinkSharedLibsLinux(__root, nativeDeps).catch(e => {
			console.error(`Failed to symlink shared libs. ${bugWarn}`)
			throw e
		})
	} else if (machineId[0] === 'Darwin') {
		// This is still required due to how ffmpeg-sys-next builds script works
		console.log(`Symlink shared libs...`)
		await symlinkSharedLibsMacOS(__root, nativeDeps).catch(e => {
			console.error(`Failed to symlink shared libs. ${bugWarn}`)
			throw e
		})
	}
} catch (error) {
	if (__debug) console.error(error)
	exit(1)
}

// Generate .cargo/config.toml
console.log('Generating cargo config...')
try {
	let isWin = false
	let isMacOS = false
	let isLinux = false
	/** @type {boolean | { linker: string }} */
	let hasLLD = false
	switch (machineId[0]) {
		case 'Linux':
			isLinux = true
			if (await which('clang')) {
				if (await which('mold')) {
					hasLLD = { linker: 'mold' }
				} else if (await which('lld')) {
					hasLLD = { linker: 'lld' }
				}
			}
			break
		case 'Darwin':
			isMacOS = true
			break
		case 'Windows_NT':
			isWin = true
			hasLLD = await which('lld-link')
			break
	}

	await fs.writeFile(
		path.join(__root, '.cargo', 'config.toml'),
		mustache
			.render(
				await fs.readFile(path.join(__root, '.cargo', 'config.toml.mustache'), {
					encoding: 'utf8',
				}),
				{
					isWin,
					isMacOS,
					isLinux,
					// Escape windows path separator to be compatible with TOML parsing
					protoc: path
						.join(
							nativeDeps,
							'bin',
							machineId[0] === 'Windows_NT' ? 'protoc.exe' : 'protoc'
						)
						.replaceAll('\\', '\\\\'),
					nativeDeps: nativeDeps.replaceAll('\\', '\\\\'),
					hasLLD,
				}
			)
			.replace(/\n\n+/g, '\n'),
		{ mode: 0o751, flag: 'w+' }
	)
} catch (error) {
	console.error(`Failed to generate .cargo/config.toml. ${bugWarn}`)
	if (__debug) console.error(error)
	exit(1)
}
