import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { env, exit, umask, platform } from 'node:process'
import { fileURLToPath } from 'node:url'

import * as toml from '@iarna/toml'

import { patchTauri } from './utils/patchTauri.mjs'
import spawn from './utils/spawn.mjs'

if (/^(msys|mingw|cygwin)$/i.test(env.OSTYPE ?? '')) {
	console.error(
		'Bash for windows is not supported, please interact with this repo from Powershell or CMD'
	)
	exit(255)
}

// Limit file permissions
umask(0o026)

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const [_, __, ...args] = process.argv

// NOTE: Must point to package root path
const __root = path.resolve(path.join(__dirname, '..'))

// Location for desktop app
const desktopApp = path.join(__root, 'apps', 'desktop')

// Location of the native dependencies
const nativeDeps = path.join(__root, 'apps', '.deps')

// Files to be removed when script finish executing
const __cleanup = /** @type {string[]} */ ([])
const cleanUp = () => Promise.all(__cleanup.map(file => fs.unlink(file).catch(() => {})))
process.on('SIGINT', cleanUp)

// Check if file/dir exists
const exists = (/** @type {string} */ path) =>
	fs
		.access(path, fs.constants.R_OK)
		.then(() => true)
		.catch(() => false)

// Export environment variables defined in cargo.toml
const cargoConfig = await fs
	.readFile(path.resolve(__root, '.cargo', 'config.toml'), { encoding: 'binary' })
	.then(toml.parse)
if (cargoConfig.env && typeof cargoConfig.env === 'object')
	for (const [name, value] of Object.entries(cargoConfig.env)) if (!env[name]) env[name] = value

// Default command
if (args.length === 0) args.push('build')

let code = 0
try {
	switch (args[0]) {
		case 'dev': {
			__cleanup.push(...(await patchTauri(__root, nativeDeps, args)))
			break
		}
		case 'build': {
			if (!env.NODE_OPTIONS || !env.NODE_OPTIONS.includes('--max_old_space_size')) {
				env.NODE_OPTIONS = `--max_old_space_size=4096 ${env.NODE_OPTIONS ?? ''}`
			}

			__cleanup.push(...(await patchTauri(__root, nativeDeps, args)))

			switch (process.platform) {
				case 'darwin': {
					// Configure DMG background
					env.BACKGROUND_FILE = path.resolve(
						desktopApp,
						'src-tauri',
						'dmg-background.png'
					)
					env.BACKGROUND_FILE_NAME = path.basename(env.BACKGROUND_FILE)
					env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${env.BACKGROUND_FILE_NAME}"`

					if (!(await exists(env.BACKGROUND_FILE)))
						console.warn(
							`WARNING: DMG background file not found at ${env.BACKGROUND_FILE}`
						)

					break
				}
				case 'linux':
					// Cleanup appimage bundle to avoid build_appimage.sh failing
					await fs.rm(path.join(__root, 'target', 'release', 'bundle', 'appimage'), {
						recursive: true,
						force: true,
					})
					break
			}
		}
	}

	await spawn('pnpm', ['exec', 'tauri', ...args], desktopApp).catch(async error => {
		if (args[0] === 'build' || platform === 'linux') {
			// Work around appimage buindling not working sometimes
			const appimageDir = path.join(__root, 'target', 'release', 'bundle', 'appimage')
			if (
				(await exists(path.join(appimageDir, 'build_appimage.sh'))) &&
				(await fs.readdir(appimageDir).then(f => f.every(f => !f.endsWith('.AppImage'))))
			) {
				// Remove AppDir to allow build_appimage to rebuild it
				await fs.rm(path.join(appimageDir, 'spacedrive.AppDir'), {
					recursive: true,
					force: true,
				})
				return spawn('bash', ['build_appimage.sh'], appimageDir).catch(exitCode => {
					code = exitCode
					console.error(`tauri ${args[0]} failed with exit code ${exitCode}`)
				})
			}
		}

		console.error(
			`tauri ${args[0]} failed with exit code ${typeof error === 'number' ? error : 1}`
		)

		console.warn(
			`If you got an error related to libav*/FFMpeg or Protoc/Protobuf you may need to re-run \`pnpm prep\``,
			`If you got an error related to missing nasm you need to run ${
				platform === 'win32' ? './scripts/setup.ps1' : './scripts/setup.sh'
			}`
		)

		throw error
	})
} catch (error) {
	if (typeof error === 'number') {
		code = error
	} else {
		if (error instanceof Error) console.error(error)
		code = 1
	}
} finally {
	cleanUp()
	exit(code)
}
