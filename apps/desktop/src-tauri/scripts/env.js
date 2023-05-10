const fs = require('node:fs');
const path = require('node:path');

const toml = require('@iarna/toml');

const { platform, workspace, setupScript } = require('./const.js');

const cargoConfig = path.resolve(workspace, '.cargo/config');
const cargoConfigTempl = path.resolve(workspace, '.cargo/config.toml');

module.exports.setupFFMpegDlls = function setupDlls(FFMPEG_DIR, dev = false) {
	const ffmpegBinDir = path.join(FFMPEG_DIR, 'bin');
	const ffmpegDlls = fs.readdirSync(ffmpegBinDir).filter((file) => file.endsWith('.dll'));

	let targetDir = path.join(workspace, 'apps/desktop/src-tauri');
	if (dev) {
		targetDir = path.join(workspace, 'target/debug');
		// Ensure the target/debug directory exists
		fs.mkdirSync(targetDir, { recursive: true });
	}

	// Copy all DLLs from the $FFMPEG_DIR/bin to targetDir
	for (const dll of ffmpegDlls)
		fs.copyFileSync(path.join(ffmpegBinDir, dll), path.join(targetDir, dll));

	return ffmpegDlls;
};

module.exports.setupPlatformEnv = function setupEnv() {
	const env = {};

	if (platform === 'darwin' || platform === 'win32') {
		env.PROTOC = path.join(
			workspace,
			'target/Frameworks/bin',
			platform === 'win32' ? 'protoc.exe' : 'protoc'
		);
		env.FFMPEG_DIR = path.join(workspace, 'target/Frameworks');

		// Check if env.PROTOC is not empty and that the value is a valid path pointing to an existing file
		if (!(env.PROTOC && fs.existsSync(env.PROTOC) && fs.statSync(env.PROTOC).isFile())) {
			console.error(`The path to protoc is invalid: ${env.PROTOC}`);
			console.error(`Did you ran the setup script: ${setupScript}?`);
			process.exit(1);
		}

		// Check if env.FFMPEG_DIR is not empty and that the value is a valid path pointing to an existing directory
		if (
			!(
				env.FFMPEG_DIR &&
				fs.existsSync(env.FFMPEG_DIR) &&
				fs.statSync(env.FFMPEG_DIR).isDirectory()
			)
		) {
			console.error(`The path to ffmpeg is invalid: ${env.FFMPEG_DIR}`);
			console.error(`Did you ran the setup script: ${setupScript}?`);
			process.exit(1);
		}

		// Update cargo config with the new env variables
		const cargoConf = toml.parse(fs.readFileSync(cargoConfigTempl, { encoding: 'binary' }));
		cargoConf.env = {
			...(cargoConf.env ?? {}),
			...(env ?? {}),
			PROTOC: env.PROTOC,
			FFMPEG_DIR: env.FFMPEG_DIR
		};
		fs.writeFileSync(cargoConfig, toml.stringify(cargoConf));
	}

	return env;
};
