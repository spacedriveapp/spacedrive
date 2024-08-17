import client from '@actions/artifact';
import * as core from '@actions/core';
import * as glob from '@actions/glob';
import * as io from '@actions/io';
import { exists } from '@actions/io/lib/io-util';

type OS = 'darwin' | 'windows' | 'linux';
type Arch = 'x64' | 'arm64';

interface TargetConfig {
	ext: string;
	bundle: string;
}

interface BuildTarget {
	updater: false | { bundle: string; bundleExt: string; archiveExt: string };
	standalone: TargetConfig[];
}

const OS_TARGETS = {
	darwin: {
		updater: {
			bundle: 'macos',
			bundleExt: 'app',
			archiveExt: 'tar.gz'
		},
		standalone: [{ ext: 'dmg', bundle: 'dmg' }]
	},
	windows: {
		updater: {
			bundle: 'msi',
			bundleExt: 'msi',
			archiveExt: 'zip'
		},
		standalone: [{ ext: 'msi', bundle: 'msi' }]
	},
	linux: {
		updater: false,
		standalone: [{ ext: 'deb', bundle: 'deb' }]
	}
} satisfies Record<OS, BuildTarget>;

// Workflow inputs
const OS = core.getInput('os') as OS;
const ARCH = core.getInput('arch') as Arch;
const TARGET = core.getInput('target');
const PROFILE = core.getInput('profile');

const BUNDLE_DIR = `target/${TARGET}/${PROFILE}/bundle`;
const ARTIFACTS_DIR = '.artifacts';
const ARTIFACT_BASE = `Spacedrive-${OS}-${ARCH}`;
const FRONT_END_BUNDLE = 'apps/desktop/dist.tar.xz';
const UPDATER_ARTIFACT_NAME = `Spacedrive-Updater-${OS}-${ARCH}`;
const FRONTEND_ARCHIVE_NAME = `Spacedrive-frontend-${OS}-${ARCH}`;

async function globFiles(pattern: string) {
	const globber = await glob.create(pattern);
	return await globber.glob();
}

async function uploadFrontend() {
	if (!(await exists(FRONT_END_BUNDLE))) {
		console.error(`Frontend archive not found`);
		return;
	}

	const artifactName = `${FRONTEND_ARCHIVE_NAME}.tar.xz`;
	const artifactPath = `${ARTIFACTS_DIR}/${artifactName}`;

	await io.cp(FRONT_END_BUNDLE, artifactPath);
	await client.uploadArtifact(artifactName, [artifactPath], ARTIFACTS_DIR);
}

async function uploadUpdater(updater: BuildTarget['updater']) {
	if (!updater) return;
	const { bundle, bundleExt, archiveExt } = updater;
	const fullExt = `${bundleExt}.${archiveExt}`;
	const files = await globFiles(`${BUNDLE_DIR}/${bundle}/*.${fullExt}*`);

	const updaterPath = files.find((file) => file.endsWith(fullExt));
	if (!updaterPath) throw new Error(`Updater path not found. Files: ${files.join(',')}`);

	const artifactPath = `${ARTIFACTS_DIR}/${UPDATER_ARTIFACT_NAME}.${archiveExt}`;

	// https://tauri.app/v1/guides/distribution/updater#update-artifacts
	await io.cp(updaterPath, artifactPath);
	await io.cp(`${updaterPath}.sig`, `${artifactPath}.sig`);

	await client.uploadArtifact(
		UPDATER_ARTIFACT_NAME,
		[artifactPath, `${artifactPath}.sig`],
		ARTIFACTS_DIR
	);
}

async function uploadStandalone({ bundle, ext }: TargetConfig) {
	const files = await globFiles(`${BUNDLE_DIR}/${bundle}/*.${ext}*`);

	const standalonePath = files.find((file) => file.endsWith(ext));
	if (!standalonePath) throw new Error(`Standalone path not found. Files: ${files.join(',')}`);

	const artifactName = `${ARTIFACT_BASE}.${ext}`;
	const artifactPath = `${ARTIFACTS_DIR}/${artifactName}`;

	await io.cp(standalonePath, artifactPath, { recursive: true });
	await client.uploadArtifact(artifactName, [artifactPath], ARTIFACTS_DIR);
}

async function run() {
	await io.mkdirP(ARTIFACTS_DIR);

	const { updater, standalone } = OS_TARGETS[OS];

	await Promise.all([
		uploadUpdater(updater),
		uploadFrontend(),
		...standalone.map((config) => uploadStandalone(config))
	]);
}

run().catch((error: unknown) => {
	console.error(error);
	process.exit(1);
});
