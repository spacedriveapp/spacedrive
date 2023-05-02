const path = require('path');
const { spawn } = require('child_process');

process.env.BACKGROUND_FILE = path.join(__dirname, './dmg-background.png');
process.env.BACKGROUND_FILE_NAME = path.basename(process.env.BACKGROUND_FILE);
process.env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`;

const child = spawn('pnpm', ['tauri', 'build'], { stdio: 'inherit' });
process.on('SIGTERM', () => proc.kill('SIGTERM'));
process.on('SIGINT', () => proc.kill('SIGINT'));
process.on('SIGBREAK', () => proc.kill('SIGBREAK'));
process.on('SIGHUP', () => proc.kill('SIGHUP'));
proc.on('exit', (code, signal) => {
	let exitCode = code;
	// exit code could be null when OS kills the process(out of memory, etc) or due to node handling it
	// but if the signal is SIGINT the user exited the process so we want exit code 0
	if (exitCode === null) exitCode = signal === 'SIGINT' ? 0 : 1;
	process.exit(exitCode); //eslint-disable-line no-process-exit
});
