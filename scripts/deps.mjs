import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { env } from 'node:process';
import { extractTo } from 'archive-wasm/src/fs.mjs';

import {
	getGh,
	getGhArtifactContent,
	getGhReleasesAssets,
	getGhWorkflowRunArtifacts
} from './github.mjs';
import {
	FFMPEG_SUFFFIX,
	FFMPEG_WORKFLOW,
	getConst,
	getSuffix,
	LIBHEIF_SUFFIX,
	LIBHEIF_WORKFLOW,
	PDFIUM_SUFFIX,
	PROTOC_SUFFIX,
	TAURI_CLI_SUFFIX
} from './suffix.mjs';
import { which } from './which.mjs';

const noop = () => {};

const __debug = env.NODE_ENV === 'debug';
const __osType = os.type();

// Github repos
const PDFIUM_REPO = 'bblanchon/pdfium-binaries';
const PROTOBUF_REPO = 'protocolbuffers/protobuf';
const SPACEDRIVE_REPO = 'spacedriveapp/spacedrive';

/**
 * Download and extract protobuff compiler binary
 * @param {string[]} machineId
 * @param {string} framework
 */
export async function downloadProtc(machineId, framework) {
	if (await which('protoc')) return;

	console.log('Downloading protoc...');

	const protocSuffix = getSuffix(PROTOC_SUFFIX, machineId);
	if (protocSuffix == null) throw new Error('NO_PROTOC');

	let found = false;
	for await (const release of getGhReleasesAssets(PROTOBUF_REPO)) {
		if (!protocSuffix.test(release.name)) continue;
		try {
			await extractTo(await getGh(release.downloadUrl), framework, {
				chmod: 0o600,
				overwrite: true
			});
			found = true;
			break;
		} catch (error) {
			console.warn('Failed to download protoc, re-trying...');
			if (__debug) console.error(error);
		}
	}

	if (!found) throw new Error('NO_PROTOC');

	// cleanup
	await fs.unlink(path.join(framework, 'readme.txt')).catch(__debug ? console.error : noop);
}

/**
 * Download and extract pdfium library for generating PDFs thumbnails
 * @param {string[]} machineId
 * @param {string} framework
 */
export async function downloadPDFium(machineId, framework) {
	console.log('Downloading pdfium...');

	const pdfiumSuffix = getSuffix(PDFIUM_SUFFIX, machineId);
	if (pdfiumSuffix == null) throw new Error('NO_PDFIUM');

	let found = false;
	for await (const release of getGhReleasesAssets(PDFIUM_REPO)) {
		if (!pdfiumSuffix.test(release.name)) continue;
		try {
			await extractTo(await getGh(release.downloadUrl), framework, {
				chmod: 0o600,
				overwrite: true
			});
			found = true;
			break;
		} catch (error) {
			console.warn('Failed to download pdfium, re-trying...');
			if (__debug) console.error(error);
		}
	}

	if (!found) throw new Error('NO_PDFIUM');

	// cleanup
	const cleanup = [
		fs.rename(path.join(framework, 'LICENSE'), path.join(framework, 'LICENSE.pdfium')),
		...['args.gn', 'PDFiumConfig.cmake', 'VERSION'].map((file) =>
			fs.unlink(path.join(framework, file)).catch(__debug ? console.error : noop)
		)
	];

	switch (__osType) {
		case 'Linux':
			cleanup.push(fs.chmod(path.join(framework, 'lib', 'libpdfium.so'), 0o750));
			break;
		case 'Darwin':
			cleanup.push(fs.chmod(path.join(framework, 'lib', 'libpdfium.dylib'), 0o750));
			break;
	}

	await Promise.all(cleanup);
}

/**
 * Download and extract ffmpeg libs for video thumbnails
 * @param {string[]} machineId
 * @param {string} framework
 * @param {string[]} branches
 */
export async function downloadFFMpeg(machineId, framework, branches) {
	const workflow = getConst(FFMPEG_WORKFLOW, machineId);
	if (workflow == null) {
		console.log('Checking FFMPeg...');
		if (await which('ffmpeg')) {
			// TODO: check ffmpeg version match what we need
			return;
		} else {
			throw new Error('NO_FFMPEG');
		}
	}

	console.log('Downloading FFMPeg...');

	const ffmpegSuffix = getSuffix(FFMPEG_SUFFFIX, machineId);
	if (ffmpegSuffix == null) throw new Error('NO_FFMPEG');

	let found = false;
	for await (const artifact of getGhWorkflowRunArtifacts(SPACEDRIVE_REPO, workflow, branches)) {
		if (!ffmpegSuffix.test(artifact.name)) continue;
		try {
			const data = await getGhArtifactContent(SPACEDRIVE_REPO, artifact.id);
			await extractTo(data, framework, {
				chmod: 0o600,
				recursive: true,
				overwrite: true
			});
			found = true;
			break;
		} catch (error) {
			console.warn('Failed to download FFMpeg, re-trying...');
			if (__debug) console.error(error);
		}
	}

	if (!found) throw new Error('NO_FFMPEG');
}

/**
 * Download and extract libheif libs for heif thumbnails
 * @param {string[]} machineId
 * @param {string} framework
 * @param {string[]} branches
 */
export async function downloadLibHeif(machineId, framework, branches) {
	const workflow = getConst(LIBHEIF_WORKFLOW, machineId);
	if (workflow == null) return;

	console.log('Downloading LibHeif...');

	const ffmpegSuffix = getSuffix(LIBHEIF_SUFFIX, machineId);
	if (ffmpegSuffix == null) throw new Error('NO_LIBHEIF');

	let found = false;
	for await (const artifact of getGhWorkflowRunArtifacts(SPACEDRIVE_REPO, workflow, branches)) {
		if (!ffmpegSuffix.test(artifact.name)) continue;
		try {
			const data = await getGhArtifactContent(SPACEDRIVE_REPO, artifact.id);
			await extractTo(data, framework, {
				chmod: 0o600,
				recursive: true,
				overwrite: true
			});
			found = true;
			break;
		} catch (error) {
			console.warn('Failed to download LibHeif, re-trying...');
			if (__debug) console.error(error);
		}
	}

	if (!found) throw new Error('NO_LIBHEIF');
}

/**
 * Workaround while https://github.com/tauri-apps/tauri/pull/3934 is not available in a Tauri stable release
 * @param {string[]} machineId
 * @param {string} framework
 * @param {string[]} branches
 */
export async function downloadPatchedTauriCLI(machineId, framework, branches) {
	console.log('Dowloading patched tauri CLI...');

	const tauriCliSuffix = getSuffix(TAURI_CLI_SUFFIX, machineId);
	if (tauriCliSuffix == null) return;

	let found = false;
	for await (const artifact of getGhWorkflowRunArtifacts(
		SPACEDRIVE_REPO,
		'tauri-patched-cli-js.yml',
		branches
	)) {
		if (!tauriCliSuffix.test(artifact.name)) continue;
		try {
			await extractTo(
				await getGhArtifactContent(SPACEDRIVE_REPO, artifact.id),
				path.join(framework, 'bin'),
				{
					chmod: 0o700,
					overwrite: true
				}
			);
			found = true;
			break;
		} catch (error) {
			console.warn('Failed to download patched tauri cli.js, re-trying...');
			if (__debug) console.error(error);
		}
	}

	if (!found) throw new Error('NO_TAURI_CLI');
}
