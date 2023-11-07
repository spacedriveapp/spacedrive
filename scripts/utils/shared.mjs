import { exec as execCb } from 'node:child_process'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { env } from 'node:process'
import { promisify } from 'node:util'

const exec = promisify(execCb)
const signId = env.APPLE_SIGNING_IDENTITY || '-'

/**
 * @param {string} origin
 * @param {string} target
 * @param {boolean} [rename]
 */
async function link(origin, target, rename) {
	const parent = path.dirname(target)
	await fs.mkdir(parent, { recursive: true, mode: 0o751 })
	await (rename ? fs.rename(origin, target) : fs.symlink(path.relative(parent, origin), target))
}

/**
 * Symlink shared libs paths for Linux
 * @param {string} root
 * @param {string} nativeDeps
 * @returns {Promise<void>}
 */
export async function symlinkSharedLibsLinux(root, nativeDeps) {
	// rpath=${ORIGIN}/../lib/spacedrive
	const targetLib = path.join(root, 'target', 'lib')
	const targetRPath = path.join(targetLib, 'spacedrive')
	await fs.unlink(targetRPath).catch(() => {})
	await fs.mkdir(targetLib, { recursive: true })
	await link(path.join(nativeDeps, 'lib'), targetRPath)
}

/**
 * Symlink shared libs paths for macOS
 * @param {string} root
 * @param {string} nativeDeps
 */
export async function symlinkSharedLibsMacOS(root, nativeDeps) {
	// rpath=@executable_path/../Frameworks/Spacedrive.framework
	const targetFrameworks = path.join(root, 'target', 'Frameworks')

	// Framework
	const framework = path.join(nativeDeps, 'Spacedrive.framework')

	// Link Spacedrive.framework to target folder so sd-server can work ootb
	await fs.rm(targetFrameworks, { recursive: true }).catch(() => {})
	await fs.mkdir(targetFrameworks, { recursive: true })
	await link(framework, path.join(targetFrameworks, 'Spacedrive.framework'))

	// Sign dylibs (Required for them to work on macOS 13+)
	await fs
		.readdir(path.join(framework, 'Libraries'), { recursive: true, withFileTypes: true })
		.then(files =>
			Promise.all(
				files
					.filter(entry => entry.isFile() && entry.name.endsWith('.dylib'))
					.map(entry =>
						exec(`codesign -s "${signId}" -f "${path.join(entry.path, entry.name)}"`)
					)
			)
		)
}

/**
 * Copy Windows DLLs for tauri build
 * @param {string} root
 * @param {string} nativeDeps
 * @returns {Promise<{files: string[], toClean: string[]}>}
 */
export async function copyWindowsDLLs(root, nativeDeps) {
	const tauriSrc = path.join(root, 'apps', 'desktop', 'src-tauri')
	const files = await Promise.all(
		await fs.readdir(path.join(nativeDeps, 'bin'), { withFileTypes: true }).then(files =>
			files
				.filter(entry => entry.isFile() && entry.name.endsWith(`.dll`))
				.map(async entry => {
					await fs.copyFile(
						path.join(entry.path, entry.name),
						path.join(tauriSrc, entry.name)
					)
					return entry.name
				})
		)
	)

	return { files, toClean: files.map(file => path.join(tauriSrc, file)) }
}

/**
 * Symlink shared libs paths for Linux
 * @param {string} root
 * @param {string} nativeDeps
 * @returns {Promise<{files: string[], toClean: string[]}>}
 */
export async function copyLinuxLibs(root, nativeDeps) {
	// rpath=${ORIGIN}/../lib/spacedrive
	const tauriSrc = path.join(root, 'apps', 'desktop', 'src-tauri')
	const files = await fs
		.readdir(path.join(nativeDeps, 'lib'), { withFileTypes: true })
		.then(files =>
			Promise.all(
				files
					.filter(
						entry =>
							(entry.isFile() || entry.isSymbolicLink()) &&
							(entry.name.endsWith('.so') || entry.name.includes('.so.'))
					)
					.map(async entry => {
						await fs.copyFile(
							path.join(entry.path, entry.name),
							path.join(tauriSrc, entry.name)
						)
						return entry.name
					})
			)
		)

	return {
		files,
		toClean: files.map(file => path.join(tauriSrc, file)),
	}
}
