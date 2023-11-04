import { env } from 'node:process'

import { extractTo } from 'archive-wasm/src/fs.mjs'

import { getConst, getSuffix, NATIVE_DEPS_SUFFIX, NATIVE_DEPS_WORKFLOW } from './consts.mjs'
import { getGhArtifactContent, getGhWorkflowRunArtifacts } from './github.mjs'
import { which } from './which.mjs'

const __debug = env.NODE_ENV === 'debug'

const sizeLimit = 256n * 1024n * 1024n

// Github repos
const SPACEDRIVE_REPO = 'spacedriveapp/spacedrive'

/**
 * Download and extract ffmpeg libs for video thumbnails
 * @param {string[]} machineId
 * @param {string} nativeDeps
 * @param {string[]} branches
 */
export async function downloadNativeDeps(machineId, nativeDeps, branches) {
	const workflow = getConst(NATIVE_DEPS_WORKFLOW, machineId)
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

	const ffmpegSuffix = getSuffix(NATIVE_DEPS_SUFFIX, machineId)
	if (ffmpegSuffix == null) throw new Error('NO_FFMPEG')

	let found = false
	for await (const artifact of getGhWorkflowRunArtifacts(SPACEDRIVE_REPO, workflow, branches)) {
		if (!ffmpegSuffix.test(artifact.name)) continue
		try {
			const data = await getGhArtifactContent(SPACEDRIVE_REPO, artifact.id)
			await extractTo(data, nativeDeps, {
				chmod: 0o600,
				sizeLimit,
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
