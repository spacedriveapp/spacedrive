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
	const targetShare = path.join(root, 'target', 'share', 'spacedrive')
	const targetRPath = path.join(targetLib, 'spacedrive')
	const targetModelShare = path.join(targetShare, 'models')
	await Promise.all([
		...[targetRPath, targetModelShare].map(path => fs.unlink(path).catch(() => {})),
		...[targetLib, targetShare].map(path => fs.mkdir(path, { recursive: true })),
	])
	await link(path.join(nativeDeps, 'lib'), targetRPath)
	await link(path.join(nativeDeps, 'models'), targetModelShare)
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
 * @param {string} nativeDeps
 * @returns {Promise<Record<string, string>>}
 */
export async function windowsDLLs(nativeDeps) {
	return Object.fromEntries(
		await fs
			.readdir(path.join(nativeDeps, 'bin'), { withFileTypes: true })
			.then(files =>
				files
					.filter(entry => entry.isFile() && entry.name.endsWith(`.dll`))
					.map(entry => [path.join(entry.path, entry.name), '.'])
			)
	)
}

/**
 * Symlink shared libs paths for Linux
 * @param {string} nativeDeps
 * @returns {Promise<Record<string, string>>}
 */
export async function linuxLibs(nativeDeps) {
	return Object.fromEntries(
		await fs
			.readdir(path.join(nativeDeps, 'lib'), { withFileTypes: true })
			.then(files =>
				Promise.all(
					files
						.filter(
							entry =>
								(entry.isFile() || entry.isSymbolicLink()) &&
								(entry.name.endsWith('.so') || entry.name.includes('.so.'))
						)
						.map(entry => [path.join(entry.path, entry.name), '.'])
				)
			)
	)
}

/**
 * Create universal framework for iOS Simulator
 * @param {string} x86x64
 * @param {string} aarch64
 * @param {string} universal
 */
export async function lipoSimulatorFramework(x86x64, aarch64, universal) {
	const frameworks = await fs
		.readdir(x86x64, { withFileTypes: true, recursive: false })
		.then(files =>
			files
				.filter(entry => entry.isDirectory() && entry.name.endsWith(`.framework`))
				.map(entry => path.basename(entry.name, '.framework'))
		)

	await Promise.all(
		frameworks.map(name =>
			fs.cp(
				path.join(x86x64, `${name}.framework`),
				path.join(universal, `${name}.framework`),
				{
					mode: fs.constants.COPYFILE_FICLONE | fs.constants.COPYFILE_EXCL,
					force: false,
					recursive: true,
					dereference: true,
					errorOnExist: true,
					preserveTimestamps: true,
				}
			)
		)
	)

	await Promise.all(
		frameworks.map(name => {
			const x86x64Lib = path.join(x86x64, `${name}.framework`, `${name}`)
			const aarch64Lib = path.join(aarch64, `${name}.framework`, `${name}`)
			const universalLib = path.join(universal, `${name}.framework`, `${name}`)
			return exec(`lipo -create '${x86x64Lib}' '${aarch64Lib}' -output '${universalLib}'`)
		})
	)
}

/**
 * Create XCFramework for iOS
 * @param {string} mobileNativeDeps
 * @param {string[]} mobileTargets
 * @param {string} output
 */
export async function createXCFramework(mobileNativeDeps, mobileTargets, output) {
	const firstMobileTarget = mobileTargets[0]
	if (firstMobileTarget == null) throw new Error('No mobile targets specified')

	const frameworks = await fs
		.readdir(path.join(mobileNativeDeps, firstMobileTarget), {
			withFileTypes: true,
			recursive: false,
		})
		.then(files =>
			files
				.filter(entry => entry.isDirectory() && entry.name.endsWith(`.framework`))
				.map(entry => path.basename(entry.name, '.framework'))
		)

	// await Promise.all(
	// 	mobileTargets.flatMap(target =>
	// 		frameworks.map(frameworks => {
	// 			if (frameworks.startsWith('lib')) return null
	// 			return fs.rename(
	// 				path.join(mobileNativeDeps, target, `${frameworks}.framework`),
	// 				path.join(mobileNativeDeps, target, `lib${frameworks}.framework`)
	// 			)
	// 		})
	// 	)
	// )

	await Promise.all(
		frameworks.map(name => {
			const frameworks = mobileTargets
				.flatMap(target => [
					'-framework',
					`'${path.join(mobileNativeDeps, target, `${name}.framework`)}'`,
				])
				.join(' ')
			const xcframework = path.join(output, `${name}.xcframework`)
			return exec(`xcodebuild -create-xcframework ${frameworks} -output '${xcframework}'`)
		})
	)
}
