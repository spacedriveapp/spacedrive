import * as fs from 'node:fs/promises'
import * as os from 'node:os'
import * as path from 'node:path'
import { env } from 'node:process'

import { extractTo } from 'archive-wasm/src/fs.mjs'

import {
	FFMPEG_SUFFFIX,
	FFMPEG_WORKFLOW,
	getConst,
	getSuffix,
	LIBHEIF_SUFFIX,
	LIBHEIF_WORKFLOW,
	PDFIUM_SUFFIX,
	PROTOC_SUFFIX,
} from './consts.mjs'
import {
	getGh,
	getGhArtifactContent,
	getGhReleasesAssets,
	getGhWorkflowRunArtifacts,
} from './github.mjs'
import { which } from './which.mjs'

const noop = () => {}

const __debug = env.NODE_ENV === 'debug'
const __osType = os.type()

// Github repos
const PDFIUM_REPO = 'bblanchon/pdfium-binaries'
const PROTOBUF_REPO = 'protocolbuffers/protobuf'
const SPACEDRIVE_REPO = 'spacedriveapp/spacedrive'

/**
 * Download and extract protobuff compiler binary
 * @param {string[]} machineId
 * @param {string} nativeDeps
 */
export async function downloadProtc(machineId, nativeDeps) {
	if (await which('protoc')) return

	console.log('Downloading protoc...')

	const protocSuffix = getSuffix(PROTOC_SUFFIX, machineId)
	if (protocSuffix == null) throw new Error('NO_PROTOC')

	let found = false
	for await (const release of getGhReleasesAssets(PROTOBUF_REPO)) {
		if (!protocSuffix.test(release.name)) continue
		try {
			await extractTo(await getGh(release.downloadUrl), nativeDeps, {
				chmod: 0o600,
				overwrite: true,
			})
			found = true
			break
		} catch (error) {
			console.warn('Failed to download protoc, re-trying...')
			if (__debug) console.error(error)
		}
	}

	if (!found) throw new Error('NO_PROTOC')

	// cleanup
	await fs.unlink(path.join(nativeDeps, 'readme.txt')).catch(__debug ? console.error : noop)
}

/**
 * Download and extract pdfium library for generating PDFs thumbnails
 * @param {string[]} machineId
 * @param {string} nativeDeps
 */
export async function downloadPDFium(machineId, nativeDeps) {
	console.log('Downloading pdfium...')

	const pdfiumSuffix = getSuffix(PDFIUM_SUFFIX, machineId)
	if (pdfiumSuffix == null) throw new Error('NO_PDFIUM')

	let found = false
	for await (const release of getGhReleasesAssets(PDFIUM_REPO)) {
		if (!pdfiumSuffix.test(release.name)) continue
		try {
			await extractTo(await getGh(release.downloadUrl), nativeDeps, {
				chmod: 0o600,
				overwrite: true,
			})
			found = true
			break
		} catch (error) {
			console.warn('Failed to download pdfium, re-trying...')
			if (__debug) console.error(error)
		}
	}

	if (!found) throw new Error('NO_PDFIUM')

	// cleanup
	const cleanup = [
		fs.rename(path.join(nativeDeps, 'LICENSE'), path.join(nativeDeps, 'LICENSE.pdfium')),
		...['args.gn', 'PDFiumConfig.cmake', 'VERSION'].map(file =>
			fs.unlink(path.join(nativeDeps, file)).catch(__debug ? console.error : noop)
		),
	]

	switch (__osType) {
		case 'Linux':
			cleanup.push(fs.chmod(path.join(nativeDeps, 'lib', 'libpdfium.so'), 0o750))
			break
		case 'Darwin':
			cleanup.push(fs.chmod(path.join(nativeDeps, 'lib', 'libpdfium.dylib'), 0o750))
			break
	}

	await Promise.all(cleanup)
}

/**
 * Download and extract ffmpeg libs for video thumbnails
 * @param {string[]} machineId
 * @param {string} nativeDeps
 * @param {string[]} branches
 */
export async function downloadFFMpeg(machineId, nativeDeps, branches) {
	const workflow = getConst(FFMPEG_WORKFLOW, machineId)
	if (workflow == null) {
		console.log('Checking FFMPeg...')
		if (await which('ffmpeg')) {
			// TODO: check ffmpeg version match what we need
			return
		} else {
			throw new Error('NO_FFMPEG')
		}
	}

	console.log('Downloading FFMPeg...')

	const ffmpegSuffix = getSuffix(FFMPEG_SUFFFIX, machineId)
	if (ffmpegSuffix == null) throw new Error('NO_FFMPEG')

	let found = false
	for await (const artifact of getGhWorkflowRunArtifacts(SPACEDRIVE_REPO, workflow, branches)) {
		if (!ffmpegSuffix.test(artifact.name)) continue
		try {
			const data = await getGhArtifactContent(SPACEDRIVE_REPO, artifact.id)
			await extractTo(data, nativeDeps, {
				chmod: 0o600,
				recursive: true,
				overwrite: true,
			})
			found = true
			break
		} catch (error) {
			console.warn('Failed to download FFMpeg, re-trying...')
			if (__debug) console.error(error)
		}
	}

	if (!found) throw new Error('NO_FFMPEG')
}
