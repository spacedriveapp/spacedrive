const fs = require('node:fs');
const util = require('node:util');
const path = require('node:path');

const { spawn } = require('./spawn.js');
const { platform, workspace, setupScript } = require('./const.js');
const { setupFFMpegDlls, setupPlatformEnv } = require('./env.js');

const BACKGROUND_FILE = path.resolve(__dirname, '..', 'dmg-background.png');
const BACKGROUND_FILE_NAME = path.basename(BACKGROUND_FILE);

const toRemove = [];
const [_, __, ...args] = process.argv;

if (args.length === 0) args.push('build');

switch (args[0]) {
	case 'dev': {
		console.log('Tauri DEV');
		const env = setupPlatformEnv(null, true);
		if (platform === 'win32') setupFFMpegDlls(env.FFMPEG_DIR, true);
		break;
	}
	case 'build': {
		console.log('Tauri Build');
		if (args.findIndex((e) => e === '-c' || e === '--config') !== -1) {
			throw new Error('Custom tauri build config is not supported.');
		}

		const env = setupPlatformEnv({
			BACKGROUND_FILE,
			BACKGROUND_CLAUSE: `set background picture of opts to file ".background:${BACKGROUND_FILE_NAME}"`,
			BACKGROUND_FILE_NAME
		});

		console.log(util.inspect(env, { depth: null, colors: true }));

		const tauriPatch = {
			build: { features: [] },
			tauri: { bundle: { macOS: {} }, updater: {} }
		};

		if (process.env.TAURI_PRIVATE_KEY) {
			console.log('Tauri Private Key detected, enable updater');
			tauriPatch.build.features.push('updater');
			tauriPatch.tauri.updater.active = true;
		}

		switch (platform) {
			case 'darwin': {
				// Workaround while https://github.com/tauri-apps/tauri/pull/3934 is not merged
				const cliNode =
					process.arch === 'arm64' ? 'cli.darwin-arm64.node' : 'cli.darwin-x64.node';

				const tauriCliPatch = path.join(workspace, 'target/Frameworks/bin/', cliNode);
				if (!fs.existsSync(tauriCliPatch)) {
					throw new Error(
						`tauri patch not found at ${tauriCliPatch}. Did you run the setup script: ${setupScript}?`
					);
				}

				const tauriBin = path.join(
					workspace,
					'node_modules/@tauri-apps',
					cliNode.replace(/\.[^.]+$/, '').replace(/\./g, '-'),
					cliNode
				);
				if (!fs.existsSync(tauriBin)) {
					throw new Error('tauri bin not found at ${tauriBin}. Did you run `pnpm i`?');
				}

				console.log(`Replace ${tauriBin} with ${tauriCliPatch}`);
				fs.copyFileSync(tauriCliPatch, tauriBin);

				if (process.arch === 'arm64') {
					console.log('Tauri ARM64 detected, set minimum system version to 11.2');
					tauriPatch.tauri.bundle.macOS.minimumSystemVersion = '11.2';
				}

				// Point tauri to the ffmpeg framework
				tauriPatch.tauri.bundle.macOS.frameworks = [
					path.join(workspace, 'target/Frameworks/FFMpeg.framework')
				];
				break;
			}
			case 'win32':
				// Point tauri to the ffmpeg DLLs
				tauriPatch.tauri.bundle.resources = setupFFMpegDlls(env.FFMPEG_DIR);
				toRemove.push(
					...tauriPatch.tauri.bundle.resources.map((file) =>
						path.join(workspace, 'apps/desktop/src-tauri', file)
					)
				);
				break;
		}

		const tauriConf = path.resolve(__dirname, '..', 'tauri.conf.patch.json');
		fs.writeFileSync(tauriConf, JSON.stringify(tauriPatch, null, 2));

		toRemove.push(tauriConf);
		args.splice(1, 0, '-c', tauriConf);

		console.log(util.inspect(tauriPatch, { depth: null, colors: true }));
	}
}

let code = 0;
spawn('pnpm', ['tauri', ...args])
	.catch((exitCode) => {
		code = exitCode;
		console.error(`tauri ${args[0]} failed with exit code ${exitCode}`);
		console.error(
			`If you got an error related to FFMpeg or Protoc/Protobuf you may need to run ${setupScript}`
		);
	})
	.finally(() => {
		for (const file of toRemove)
			try {
				fs.unlinkSync(file);
			} catch (e) {}

		process.exit(code);
	});
