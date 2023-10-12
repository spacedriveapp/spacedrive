import artifact from '@actions/artifact';
import core from '@actions/core';
import io from '@actions/io';

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

const client = artifact.create();

for (const { ext, updaterExt, bundle } of OS_TARGETS[OS]) {
	const bundleDir = `${BUNDLE_DIR}/${bundle}`;

	const name = `Spacedrive-${OS}-${ARCH}.${ext}`;
	const artifactPath = `${ARTIFACTS_DIR}/${name}`;

	await io.mv(`${bundleDir}/Spacedrive*.${ext}`, artifactPath);
	await client.uploadArtifact(`Spacedrive-${OS}-${ARCH}`, [name], ARTIFACTS_DIR);

	if (updaterExt) {
		const buildName = `Spacedrive.${ext}.${updaterExt}`;
		const artifactName = `Spacedrive-Updater-${OS}-${ARCH}`;

		// https://tauri.app/v1/guides/distribution/updater#update-artifacts
		await io.mv(`${bundleDir}/${buildName}`, `${ARTIFACTS_DIR}/${artifactName}.${updaterExt}`);
		await io.mv(
			`${bundleDir}/${buildName}.sig`,
			`${ARTIFACTS_DIR}/${artifactName}.${updaterExt}.sig`
		);

		await client.uploadArtifact(
			artifactName,
			[`${artifactName}.${updaterExt}*`],
			ARTIFACTS_DIR
		);
	}
}
