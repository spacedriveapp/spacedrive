import * as artifact from '@actions/artifact';
import * as core from '@actions/core';
import * as glob from '@actions/glob';
import * as io from '@actions/io';

type OS = 'darwin' | 'windows' | 'linux';
type Arch = 'x64' | 'arm64';
type BuildTarget = { ext: string; updaterExt?: string; bundle: string };

const OS_TARGETS = {
	darwin: [
		{ ext: 'dmg', bundle: 'dmg' },
		{ ext: 'app', updaterExt: '.tar.gz', bundle: 'macos' }
	],
	windows: [{ ext: 'msi', updaterExt: '.zip', bundle: 'msi' }],
	linux: [
		{ ext: 'deb', bundle: 'deb' },
		{ ext: 'AppImage', updaterExt: '.tar.gz', bundle: 'appimage' }
	]
} satisfies Record<OS, Array<BuildTarget>>;

// Workflow inputs
const OS: OS = core.getInput('os') as any;
const ARCH: Arch = core.getInput('arch') as any;
const TARGET = core.getInput('target');
const PROFILE = core.getInput('profile');

const BUNDLE_DIR = `target/${TARGET}/${PROFILE}/bundle`;
const ARTIFACTS_DIR = '.artifacts';
const ARTIFACT_NAME = `Spacedrive-${OS}-${ARCH}`;

const client = artifact.create();

async function run() {
	for (const { ext, updaterExt, bundle } of OS_TARGETS[OS]) {
		const bundlePath = `${BUNDLE_DIR}/${bundle}`;

		const name = `${ARTIFACT_NAME}.${ext}`;
		const artifactPath = `${ARTIFACTS_DIR}/${name}`;

		const globber = await glob.create(`${bundlePath}/*.${ext}*`);
		const files = await globber.glob();

		const standalonePath = files.find((file) => file.endsWith(ext));
		if (!standalonePath) throw `Standalone path not found. Files: ${files}`;

		await io.cp(standalonePath, artifactPath);
		await client.uploadArtifact(ARTIFACT_NAME, [artifactPath], './');

		if (updaterExt) {
			const artifactName = `Spacedrive-Updater-${OS}-${ARCH}.${updaterExt}`;
			const artifactPath = `${ARTIFACTS_DIR}/${artifactName}`;

			const updaterPath = files.find((file) => file.endsWith(updaterExt));
			if (!updaterPath) throw `Updater path not found. Files: ${files}`;

			// https://tauri.app/v1/guides/distribution/updater#update-artifacts
			await io.cp(updaterPath, artifactPath);
			await io.cp(`${updaterPath}.sig`, `${artifactPath}.sig`);

			await client.uploadArtifact(artifactName, [artifactPath, `${artifactPath}.sig`], './');
		}
	}
}

run();
