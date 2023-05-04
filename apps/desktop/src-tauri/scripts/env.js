const fs = require('node:fs');
const path = require('node:path');
const toml = require('@iarna/toml');

const { workspace, platform } = require('./const.js');

const cargoConfigTempl = path.resolve(workspace, '.cargo/config.toml');
const cargoConfig = path.resolve(workspace, '.cargo/config');

module.exports.setupPlatformEnv = function setupEnv(env, dev = false) {
	if (env !== null && typeof env === 'object') {
		process.env = Object.assign(process.env, env);
	} else {
		env = null;
	}

	if (platform === 'darwin' || platform === 'win32') {
		process.env.PROTOC = path.join(workspace, 'target/Frameworks/bin/protoc');
		process.env.FFMPEG_DIR = path.join(workspace, 'target/Frameworks');

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

		// Update cargo config with the new env variables
		const cargoConf = toml.parse(fs.readFileSync(cargoConfigTempl, { encoding: 'binary' }));
		cargoConf.env = {
			...(cargoConf.env ?? {}),
			...(env ?? {}),
			PROTOC: process.env.PROTOC,
			FFMPEG_DIR: process.env.FFMPEG_DIR
		};
		fs.writeFileSync(cargoConfig, toml.stringify(cargoConf));
	}

	if (dev && platform === 'win32') {
		// Ensure the target/debug directory exists
		const debugTargetDir = path.join(workspace, 'target/debug');
		fs.mkdirSync(debugTargetDir, { recursive: true });

		// Copy all DLLs from the $FFMPEG_DIR/bin to target/debug
		for (const dll of fs
			.readdirSync(path.join(process.env.FFMPEG_DIR, 'bin'))
			.filter((file) => file.endsWith('.dll'))) {
			fs.copyFileSync(path.join(ffmpegBinDir, dll), path.join(debugTargetDir, dll));
		}
	}
};
