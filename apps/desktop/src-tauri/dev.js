const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

isWindows = () => process.platform === 'win32' || /^(msys|cygwin)$/.test(process.env.OSTYPE);

const script = `.github/scripts/${isWindows ? 'setup-system.ps1' : 'setup-system.sh'}`;

// Root pnpm workspace directory
const rootDir = path.join(__dirname, '../../../');

if (process.platform === 'darwin') {
	process.env.PROTOC = path.join(rootDir, 'target/Frameworks/bin/protoc');
	process.env.FFMPEG_DIR = path.join(rootDir, 'target/Frameworks');
}

if (!process.platform === 'linux') {
	// Check if process.env.PROTOC is not empty and that the value is a valid path pointing to an existing file
	if (
		!(
			process.env.PROTOC &&
			fs.existsSync(process.env.PROTOC) &&
			fs.statSync(process.env.PROTOC).isFile()
		)
	) {
		console.error(`The path to protoc is invalid: ${process.env.PROTOC}`);
		console.error(`Did you ran the setup script: ${script}?`);
		process.exit(1);
	}

	// Check if process.env.FFMPEG_DIR is not empty and that the value is a valid path pointing to an existing directory
	if (
		!(
			process.env.FFMPEG_DIR &&
			fs.existsSync(process.env.FFMPEG_DIR) &&
			fs.statSync(process.env.FFMPEG_DIR).isDirectory()
		)
	) {
		console.error(`The path to ffmpeg is invalid: ${process.env.FFMPEG_DIR}`);
		console.error(`Did you ran the setup script: ${script}?`);
		process.exit(1);
	}
}

if (isWindows) {
	// Ensure the target/debug directory exists
	const debugTargetDir = path.join(rootDir, 'target/debug');
	fs.mkdirSync(debugTargetDir, { recursive: true });

	// Copy all DLLs from the $FFMPEG_DIR/bin to target/debug
	for (const dll of fs
		.readdirSync(path.join(process.env.FFMPEG_DIR, 'bin'))
		.filter((file) => file.endsWith('.dll'))) {
		fs.copyFileSync(
			path.join(ffmpegBinDir, dll),
			path.join(debugTargetDir, dll)
		);
	}
}

const child = spawn('pnpm', ['tauri', 'dev'], { stdio: 'inherit' });
process.on('SIGTERM', () => child.kill('SIGTERM'));
process.on('SIGINT', () => child.kill('SIGINT'));
process.on('SIGBREAK', () => child.kill('SIGBREAK'));
process.on('SIGHUP', () => child.kill('SIGHUP'));
child.on('exit', (code, signal) => {
	let exitCode = code;
	// exit code could be null when OS kills the process(out of memory, etc) or due to node handling it
	// but if the signal is SIGINT the user exited the process so we want exit code 0
	if (exitCode === null) exitCode = signal === 'SIGINT' ? 0 : 1;
	process.exit(exitCode); //eslint-disable-line no-process-exit
});
