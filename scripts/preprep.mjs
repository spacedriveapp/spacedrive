import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { env, exit, umask } from 'node:process'
import { fileURLToPath } from 'node:url'

import * as _mustache from 'mustache'

import { downloadFFMpeg, downloadPDFium, downloadProtc } from './utils/deps.mjs'
import { getGitBranches } from './utils/git.mjs'
import { getMachineId } from './utils/machineId.mjs'
import {
	setupMacOsFramework,
	symlinkSharedLibsMacOS,
	symlinkSharedLibsLinux,
} from './utils/shared.mjs'
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
await Promise.all(
	['bin', 'lib', 'include'].map(dir =>
		fs.mkdir(path.join(nativeDeps, dir), { mode: 0o750, recursive: true })
	)
)

// Accepted git branches for querying for artifacts (current, main, master)
const branches = await getGitBranches(__root)

// Download all necessary external dependencies
await Promise.all([
	downloadProtc(machineId, nativeDeps).catch(e => {
		console.error(
			'Failed to download protobuf compiler, this is required to build Spacedrive. ' +
				'Please install it with your system package manager'
		)
		throw e
	}),
	downloadPDFium(machineId, nativeDeps).catch(e => {
		console.warn(
			'Failed to download pdfium lib. ' +
				"This is optional, but if one isn't present Spacedrive won't be able to generate thumbnails for PDF files"
		)
		if (__debug) console.error(e)
	}),
	downloadFFMpeg(machineId, nativeDeps, branches).catch(e => {
		console.error(`Failed to download ffmpeg. ${bugWarn}`)
		throw e
	}),
]).catch(e => {
	if (__debug) console.error(e)
	exit(1)
})

// Extra OS specific setup
try {
	if (machineId[0] === 'Linux') {
		console.log(`Symlink shared libs...`)
		symlinkSharedLibsLinux(__root, nativeDeps).catch(e => {
			console.error(`Failed to symlink shared libs. ${bugWarn}`)
			throw e
		})
	} else if (machineId[0] === 'Darwin') {
		console.log(`Setup Framework...`)
		await setupMacOsFramework(nativeDeps).catch(e => {
			console.error(`Failed to setup Framework. ${bugWarn}`)
			throw e
		})
		// This is still required due to how ffmpeg-sys-next builds script works
		console.log(`Symlink shared libs...`)
		await symlinkSharedLibsMacOS(nativeDeps).catch(e => {
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
	await fs.writeFile(
		path.join(__root, '.cargo', 'config.toml'),
		mustache
			.render(
				await fs.readFile(path.join(__root, '.cargo', 'config.toml.mustache'), {
					encoding: 'utf8',
				}),
				{
					isWin: machineId[0] === 'Windows_NT',
					isMacOS: machineId[0] === 'Darwin',
					isLinux: machineId[0] === 'Linux',
					// Escape windows path separator to be compatible with TOML parsing
					protoc: path
						.join(
							nativeDeps,
							'bin',
							machineId[0] === 'Windows_NT' ? 'protoc.exe' : 'protoc'
						)
						.replaceAll('\\', '\\\\'),
					nativeDeps: nativeDeps.replaceAll('\\', '\\\\'),
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
