const fs = require('node:fs');
const path = require('node:path');

const merge = require('lodash.merge');

const { spawn } = require('./spawn.js');
const { platform, workspace } = require('./const.js');
const { setupFFMpegDlls, setupPlatformEnv } = require('./env.js');

const BACKGROUND_FILE = path.resolve(__dirname, '..', 'dmg-background.png');
const BACKGROUND_FILE_NAME = path.basename(BACKGROUND_FILE);

const env = setupPlatformEnv({
	BACKGROUND_FILE,
	BACKGROUND_CLAUSE: `set background picture of opts to file ".background:${BACKGROUND_FILE_NAME}"`,
	BACKGROUND_FILE_NAME
});

const tauriPatch = {
	tauri: {
		bundle: {
			macOS: { frameworks: [path.join(workspace, 'target/Frameworks/FFMpeg.framework')] }
		}
	}
};

if (platform === 'win32') {
	// Point tauri to the ffmpeg DLLs
	tauriPatch.tauri.bundle.resources = setupFFMpegDlls(env.FFMPEG_DIR);
}

if (process.env.CI === 'true') {
	// In CI, backup original tauri config and replace it with our patched version merged with the original
	const tauriConf = path.resolve(__dirname, '..', 'tauri.conf.json');
	fs.copyFileSync(tauriConf, `${tauriConf}.bak`);
	fs.writeFileSync(
		tauriConf,
		JSON.stringify(
			merge(JSON.parse(fs.readFileSync(tauriConf, { encoding: 'utf-8' })), tauriPatch),
			null,
			2
		)
	);
} else {
	const tauriConf = path.resolve(__dirname, '..', 'tauri.conf.patch.json');
	fs.writeFileSync(tauriConf, JSON.stringify(tauriPatch, null, 2));

	let code = 0;
	spawn('pnpm', ['tauri', 'build', '-c', tauriConf])
		.catch((exitCode) => {
			code = exitCode;
			console.error(`tauri build failed with exit code ${exitCode}`);
		})
		.finally(() => {
			try {
				fs.unlinkSync(tauriConf);
			} catch (e) {}
			process.exit(code);
		});
}
