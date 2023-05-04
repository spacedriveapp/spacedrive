const fs = require('node:fs');
const path = require('node:path');

const { spawn } = require('./spawn.js');
const { setupPlatformEnv } = require('./env.js');
const { workspace, platform } = require('./const.js');

setupPlatformEnv({
	BACKGROUND_FILE: (BACKGROUND_FILE = path.resolve(__dirname, '..', 'dmg-background.png')),
	BACKGROUND_FILE_NAME: (BACKGROUND_FILE_NAME = path.basename(process.env.BACKGROUND_FILE)),
	BACKGROUND_CLAUSE:
		(BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`)
});

const tauriConfPath = path.resolve(__dirname, '..', 'tauri.conf.json');
const tauriConf = fs.readFileSync(tauriConfPath, { encoding: 'utf-8' });
const tauri = JSON.parse(tauriConf);

if (platform === 'darwin') {
	tauri.tauri.macOSPrivateApi = false;
	tauri.tauri.bundle.macOS.frameworks = [
		...(tauri.tauri.bundle.macOS.frameworks ?? []),
		path.join(workspace, 'target/Frameworks/FFMpeg.framework')
	];
}

fs.writeFileSync(tauriConfPath, JSON.stringify(tauri, null, 2));

if (process.env.CI === 'true') {
	fs.writeFileSync(`${tauriConfPath}.bak`, tauriConf);
} else {
	const args = ['tauri', 'build'];

	if (platform === 'darwin') {
		// Disable updater bundle due to: https://github.com/tauri-apps/tauri/issues/3933
		args.concat(['--bundle', 'dmg,app']);
	}

	spawn('pnpm', args).then(
		() => {
			fs.writeFileSync(tauriConfPath, tauriConf);
		},
		(code) => {
			fs.writeFileSync(tauriConfPath, tauriConf);
			process.exit(code);
		}
	);
}
