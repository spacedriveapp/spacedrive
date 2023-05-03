const path = require('path');
const { spawn } = require('child_process');

process.env.BACKGROUND_FILE = path.join(__dirname, './dmg-background.png');
process.env.BACKGROUND_FILE_NAME = path.basename(process.env.BACKGROUND_FILE);
process.env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`;

// TODO: Save a backup of tauri.conf.json to tauri.conf.json.bak
// TODO: Modify tauri.conf.json
// 	- macOSPrivateApi -> false
// 	- populate .tauri.bundle.macOS.frameworks with FFMpeg.framework path

const child = spawn('pnpm', ['tauri', 'build'], { stdio: 'inherit' });
process.on('SIGTERM', () => child.kill('SIGTERM'));
process.on('SIGINT', () => child.kill('SIGINT'));
process.on('SIGBREAK', () => child.kill('SIGBREAK'));
process.on('SIGHUP', () => child.kill('SIGHUP'));
child.on('exit', (code, signal) => {
	// TODO: restore tauri.conf.json.bak to tauri.conf.json
	let exitCode = code;
	// exit code could be null when OS kills the process(out of memory, etc) or due to node handling it
	// but if the signal is SIGINT the user exited the process so we want exit code 0
	if (exitCode === null) exitCode = signal === 'SIGINT' ? 0 : 1;
	process.exit(exitCode); //eslint-disable-line no-process-exit
});
