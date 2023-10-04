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
 * Move headers and dylibs of external deps to our framework
 * @param {string} nativeDeps
 */
export async function setupMacOsFramework(nativeDeps) {
	// External deps
	const lib = path.join(nativeDeps, 'lib')
	const include = path.join(nativeDeps, 'include')

	// Framework
	const framework = path.join(nativeDeps, 'FFMpeg.framework')
	const headers = path.join(framework, 'Headers')
	const libraries = path.join(framework, 'Libraries')
	const documentation = path.join(framework, 'Resources', 'English.lproj', 'Documentation')

	// Move files
	await Promise.all([
		// Move pdfium license to framework
		fs.rename(
			path.join(nativeDeps, 'LICENSE.pdfium'),
			path.join(documentation, 'LICENSE.pdfium')
		),
		// Move dylibs to framework
		fs.readdir(lib, { recursive: true, withFileTypes: true }).then(file =>
			file
				.filter(
					entry =>
						(entry.isFile() || entry.isSymbolicLink()) && entry.name.endsWith('.dylib')
				)
				.map(entry => {
					const file = path.join(entry.path, entry.name)
					const newFile = path.resolve(libraries, path.relative(lib, file))
					return link(file, newFile, true)
				})
		),
		// Move headers to framework
		fs.readdir(include, { recursive: true, withFileTypes: true }).then(file =>
			file
				.filter(
					entry =>
						(entry.isFile() || entry.isSymbolicLink()) &&
						!entry.name.endsWith('.proto')
				)
				.map(entry => {
					const file = path.join(entry.path, entry.name)
					const newFile = path.resolve(headers, path.relative(include, file))
					return link(file, newFile, true)
				})
		),
	])
}

/**
 * Symlink shared libs paths for Linux
 * @param {string} root
 * @param {string} nativeDeps
 * @returns {Promise<{files: string[], toClean: string[]}>}
 */
export async function symlinkSharedLibsLinux(root, nativeDeps) {
	// rpath=${ORIGIN}/../lib/spacedrive
	const tauriSrc = path.join(root, 'apps', 'desktop', 'src-tauri')
	const targetRPath = path.join(root, 'target', 'lib', 'spacedrive')

	const [files] = await Promise.all([
		fs.readdir(path.join(nativeDeps, 'lib'), { withFileTypes: true }).then(files =>
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
		),
		link(path.join(nativeDeps, 'lib'), targetRPath),
	])

	return {
		files,
		toClean: [...files, targetRPath],
	}
}

/**
 * Symlink shared libs paths for macOS
 * @param {string} nativeDeps
 */
export async function symlinkSharedLibsMacOS(nativeDeps) {
	// External deps
	const lib = path.join(nativeDeps, 'lib')
	const include = path.join(nativeDeps, 'include')

	// Framework
	const framework = path.join(nativeDeps, 'FFMpeg.framework')
	const headers = path.join(framework, 'Headers')
	const libraries = path.join(framework, 'Libraries')

	// Link files
	await Promise.all([
		// Link header files
		fs.readdir(headers, { recursive: true, withFileTypes: true }).then(files =>
			Promise.all(
				files
					.filter(entry => entry.isFile() || entry.isSymbolicLink())
					.map(entry => {
						const file = path.join(entry.path, entry.name)
						return link(file, path.resolve(include, path.relative(headers, file)))
					})
			)
		),
		// Link dylibs
		fs.readdir(libraries, { recursive: true, withFileTypes: true }).then(files =>
			Promise.all(
				files
					.filter(
						entry =>
							(entry.isFile() || entry.isSymbolicLink()) &&
							entry.name.endsWith('.dylib')
					)
					.map(entry => {
						const file = path.join(entry.path, entry.name)
						/** @type {Promise<unknown>[]} */
						const actions = [
							link(file, path.resolve(lib, path.relative(libraries, file))),
						]

						// Sign dylib (Required for it to work on macOS 13+)
						if (entry.isFile())
							actions.push(exec(`codesign -s "${signId}" -f "${file}"`))

						return actions.length > 1 ? Promise.all(actions) : actions[0]
					})
			)
		),
	])
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
