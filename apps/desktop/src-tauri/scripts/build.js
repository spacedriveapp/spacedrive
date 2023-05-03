const fs = require('node:fs');
const path = require('node:path');

const { spawn } = require('./spawn.js');
const { setupPlatformEnv } = require('./env.js');
const { workspace, platform } = require('./const.js');

setupPlatformEnv();

process.env.BACKGROUND_FILE = path.resolve(__dirname, '..', 'dmg-background.png');
process.env.BACKGROUND_FILE_NAME = path.basename(process.env.BACKGROUND_FILE);
process.env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`;

const tauriConfPath = path.resolve(__dirname, '..', 'tauri.conf.json');
const tauriConf = fs.readFileSync(tauriConfPath, { encoding: 'utf-8' });
const tauri = JSON.parse(tauriConf);

if (platform === 'darwin') {
	tauri.macOSPrivateApi = false;
	tauri.tauri.bundle.macOS.frameworks = [
		...(tauri.tauri.bundle.macOS.frameworks ?? []),
		path.join(workspace, 'target/Frameworks/FFMpeg.framework')
	];
}

fs.writeFileSync(tauriConfPath, JSON.stringify(tauri, null, 2));

spawn('pnpm', ['tauri', 'build']).then(
	() => {
		fs.writeFileSync(tauriConfPath, tauriConf);
	},
	(code) => {
		fs.writeFileSync(tauriConfPath, tauriConf);
		process.exit(code);
	}
);
