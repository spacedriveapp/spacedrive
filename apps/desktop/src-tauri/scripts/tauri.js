const fs = require('node:fs');
const path = require('node:path');

const toml = require('@iarna/toml');
const semver = require('semver');

const { spawn } = require('./spawn.js');

const workspace = path.resolve(__dirname, '../../../../')
const cargoConfig = toml.parse(
	fs.readFileSync(path.resolve(workspace, '.cargo/config.toml'), { encoding: 'binary' })
);
if (cargoConfig.env && typeof cargoConfig.env === 'object')
	for (const [name, value] of Object.entries(cargoConfig.env))
		if (!process.env[name]) process.env[name] = value;

const toRemove = [];
const [_, __, ...args] = process.argv;

if (args.length === 0) args.push('build');

const tauriConf = JSON.parse(
	fs.readFileSync(path.resolve(__dirname, '..', 'tauri.conf.json'), 'utf-8')
);

switch (args[0]) {
	case 'dev': {
		if (process.platform === 'win32') setupFFMpegDlls(true);
		break;
	}
	case 'build': {
		if (!process.env.NODE_OPTIONS || !process.env.NODE_OPTIONS.includes('--max_old_space_size')) {
			process.env.NODE_OPTIONS = `--max_old_space_size=4096 ${process.env.NODE_OPTIONS ?? ''}`;
		}

		if (args.findIndex((e) => e === '-c' || e === '--config') !== -1) {
			throw new Error('Custom tauri build config is not supported.');
		}

		const targets = args
			.filter((_, index, args) => {
				if (index === 0) return false;
				const previous = args[index - 1];
				return previous === '-t' || previous === '--target';
			})
			.flatMap((target) => target.split(','));

		const tauriPatch = {
			tauri: { bundle: { macOS: {} } }
		};

		switch (process.platform) {
			case 'darwin': {
				// Workaround while https://github.com/tauri-apps/tauri/pull/3934 is not merged
				const cliNode =
					process.arch === 'arm64' ? 'cli.darwin-arm64.node' : 'cli.darwin-x64.node';
				const tauriCliPatch = path.join(workspace, 'target/Frameworks/bin/', cliNode);
				if (!fs.existsSync(tauriCliPatch)) {
					throw new Error(
						`Tauri cli patch not found at ${path.relative(
							workspace,
							tauriCliPatch
						)}. Did you run \`pnpm i\`?`
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
				console.log(
					`WORKAROUND tauri-apps/tauri#3933: Replace ${path.relative(
						workspace,
						tauriBin
					)} -> ${path.relative(workspace, tauriCliPatch)}`
				);
				fs.copyFileSync(tauriCliPatch, tauriBin);

				// ARM64 support was added in macOS 11, but we need at least 11.2 due to our ffmpeg build
				let macOSMinimumVersion = tauriConf?.tauri?.bundle?.macOS?.minimumSystemVersion;
				let macOSArm64MinimumVersion = '11.2';
				if (
					(targets.includes('aarch64-apple-darwin') ||
						(targets.length === 0 && process.arch === 'arm64')) &&
					(macOSMinimumVersion == null ||
						semver.lt(
							semver.coerce(macOSMinimumVersion),
							semver.coerce(macOSArm64MinimumVersion)
						))
				) {
					macOSMinimumVersion = macOSArm64MinimumVersion;
					console.log(
						`aarch64-apple-darwin target detected, setting minimum system version to ${macOSMinimumVersion}`
					);
				}

				if (macOSMinimumVersion) {
					process.env.MACOSX_DEPLOYMENT_TARGET = macOSMinimumVersion;
					tauriPatch.tauri.bundle.macOS.minimumSystemVersion = macOSMinimumVersion;
				}

				// Point tauri to our ffmpeg framework
				tauriPatch.tauri.bundle.macOS.frameworks = [
					path.join(workspace, 'target/Frameworks/FFMpeg.framework')
				];

				// Configure DMG background
				process.env.BACKGROUND_FILE = path.resolve(__dirname, '..', 'dmg-background.png');
				process.env.BACKGROUND_FILE_NAME = path.basename(process.env.BACKGROUND_FILE);
				process.env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`;

				if (!fs.existsSync(process.env.BACKGROUND_FILE))
					console.warn(
						`WARNING: DMG background file not found at ${process.env.BACKGROUND_FILE}`
					);

				break;
			}
			case 'win32':
				// Point tauri to the ffmpeg DLLs
				tauriPatch.tauri.bundle.resources = setupFFMpegDlls();
				toRemove.push(
					...tauriPatch.tauri.bundle.resources.map((file) =>
						path.join(workspace, 'apps/desktop/src-tauri', file)
					)
				);
				break;
		}

		const tauriPatchConf = path.resolve(__dirname, '..', 'tauri.conf.patch.json');
		fs.writeFileSync(tauriPatchConf, JSON.stringify(tauriPatch, null, 2));

		toRemove.push(tauriPatchConf);
		args.splice(1, 0, '-c', tauriPatchConf);
	}
}

let code = 0;
spawn('pnpm', ['tauri', ...args])
	.catch((exitCode) => {
		code = exitCode;
		console.error(`tauri ${args[0]} failed with exit code ${exitCode}`);
		console.error(
			`If you got an error related to FFMpeg or Protoc/Protobuf you may need to re-run \`pnpm i\``
		);
	})
	.finally(() => {
		for (const file of toRemove)
			try {
				fs.unlinkSync(file);
			} catch (e) {}

		process.exit(code);
	});

function setupFFMpegDlls(dev = false) {
	if (!process.env.FFMPEG_DIR) throw new Error('Missing envvar FFMPEG_DIR');
	const ffmpegBinDir = path.join(process.env.FFMPEG_DIR, 'bin');
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
}
