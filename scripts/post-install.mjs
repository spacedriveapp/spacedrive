import { exec as _exec } from 'node:child_process';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { env, umask } from 'node:process';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import mustache from 'mustache';

import { downloadFFMpeg, downloadPDFium, downloadProtc } from './deps.mjs';
import { getGitBranches } from './git.mjs';
import { isMusl } from './musl.mjs';
import { which } from './which.mjs';

umask(0o026);

if (env.IGNORE_POSTINSTALL === 'true') process.exit(0);

if (/^(msys|mingw|cygwin)$/i.test(env.OSTYPE ?? '')) {
	console.error('Bash for windows is not supported, please execute this from Powershell or CMD');
	process.exit(255);
}

const exec = promisify(_exec);

const __debug = env.NODE_ENV === 'debug';
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// NOTE: Must point to package root path
const __root = path.resolve(path.join(__dirname, '..'));

// Current machine identifiers
const machineId = [os.type(), os.machine()];
if (machineId[0] === 'Linux') machineId.push((await isMusl()) ? 'musl' : 'glibc');

// Basic dependeny check
if (
	(await Promise.all([which('cargo'), which('rustc'), which('pnpm'), which('node')])).some(
		(found) => !found
	)
) {
	console.error(`Basic dependencies missing.
Make sure you have rust, node.js and pnpm installed:
https://rustup.rs
https://nodejs.org/en/download
https://pnpm.io/installation

Also that you have run the setup script:
packages/scripts/${machineId[0] === 'Windows_NT' ? 'setup.ps1' : 'setup.sh'}
`);
}

// Accepted git branches for querying for artifacts (current, main, master)
const branches = await getGitBranches(__root);

// Create the basic target directory hierarchy
const nativeDeps = path.join(__root, 'native-deps');
await fs.rm(nativeDeps, { force: true, recursive: true });
await Promise.all(
	['bin', 'lib', 'include'].map((dir) =>
		fs.mkdir(path.join(nativeDeps, dir), { mode: 0o750, recursive: true })
	)
);

// Download all necessary external dependencies
const deps = [
	downloadProtc(machineId, nativeDeps).catch((e) => {
		console.error(
			'Failed to download protoc, this is required for Spacedrive to compile. ' +
				'Please install it with your system package manager'
		);
		throw e;
	}),
	downloadPDFium(machineId, nativeDeps).catch((e) => {
		console.warn(
			'Failed to download pdfium lib. ' +
				"This is optional, but if one isn't configured Spacedrive won't be able to generate thumbnails for PDF files"
		);
		if (__debug) console.error(e);
	}),
	downloadFFMpeg(machineId, nativeDeps, branches).catch((e) => {
		console.error(
			'Failed to download ffmpeg. This is probably a bug, please open a issue with you system info at: ' +
				'https://github.com/spacedriveapp/spacedrive/issues/new/choose'
		);
		throw e;
	})
];

await Promise.all(deps).catch((e) => {
	if (__debug) console.error(e);
	process.exit(1);
});

// Generate .cargo/config.toml
console.log('Generating cargo config...');
try {
	await fs.writeFile(
		path.join(__root, '.cargo', 'config.toml'),
		mustache
			.render(
				await fs.readFile(path.join(__root, '.cargo', 'config.toml.mustache'), {
					encoding: 'utf8'
				}),
				{
					ffmpeg: machineId[0] === 'Linux' ? false : nativeDeps.replaceAll('\\', '\\\\'),
					protoc: path
						.join(
							nativeDeps,
							'bin',
							machineId[0] === 'Windows_NT' ? 'protoc.exe' : 'protoc'
						)
						.replaceAll('\\', '\\\\'),
					projectRoot: __root.replaceAll('\\', '\\\\'),
					isWin: machineId[0] === 'Windows_NT',
					isMacOS: machineId[0] === 'Darwin',
					isLinux: machineId[0] === 'Linux'
				}
			)
			.replace(/\n\n+/g, '\n'),
		{ mode: 0o751, flag: 'w+' }
	);
} catch (error) {
	console.error(
		'Failed to generate .cargo/config.toml, please open an issue on: ' +
			'https://github.com/spacedriveapp/spacedrive/issues/new/choose'
	);
	if (__debug) console.error(error);
	process.exit(1);
}

// Setup macOS Frameworks
if (machineId[0] === 'Darwin') {
	try {
		console.log('Setup Frameworks & Sign libraries...');
		const ffmpegFramework = path.join(nativeDeps, 'FFMpeg.framework');
		// Move pdfium License to FFMpeg.framework
		await fs.rename(
			path.join(nativeDeps, 'LICENSE.pdfium'),
			path.join(
				ffmpegFramework,
				'Resources',
				'English.lproj',
				'Documentation',
				'LICENSE.pdfium'
			)
		);
		// Move include files to FFMpeg.framework
		const include = path.join(nativeDeps, 'include');
		const headers = path.join(ffmpegFramework, 'Headers');
		const includeFiles = await fs.readdir(include, { recursive: true, withFileTypes: true });
		const moveIncludes = includeFiles
			.filter(
				(entry) =>
					(entry.isFile() || entry.isSymbolicLink()) && !entry.name.endsWith('.proto')
			)
			.map(async (entry) => {
				const file = path.join(entry.path, entry.name);
				const newFile = path.resolve(headers, path.relative(include, file));
				await fs.mkdir(path.dirname(newFile), { mode: 0o751, recursive: true });
				await fs.rename(file, newFile);
			});
		// Move libs to FFMpeg.framework
		const lib = path.join(nativeDeps, 'lib');
		const libraries = path.join(ffmpegFramework, 'Libraries');
		const libFiles = await fs.readdir(lib, { recursive: true, withFileTypes: true });
		const moveLibs = libFiles
			.filter(
				(entry) =>
					(entry.isFile() || entry.isSymbolicLink()) && entry.name.endsWith('.dylib')
			)
			.map(async (entry) => {
				const file = path.join(entry.path, entry.name);
				const newFile = path.resolve(libraries, path.relative(lib, file));
				await fs.mkdir(path.dirname(newFile), { mode: 0o751, recursive: true });
				await fs.rename(file, newFile);
			});

		await Promise.all([...moveIncludes, ...moveLibs]);

		// Symlink headers
		const headerFiles = await fs.readdir(headers, { recursive: true, withFileTypes: true });
		const linkHeaders = headerFiles
			.filter((entry) => entry.isFile() || entry.isSymbolicLink())
			.map(async (entry) => {
				const file = path.join(entry.path, entry.name);
				const link = path.resolve(include, path.relative(headers, file));
				const linkDir = path.dirname(link);
				await fs.mkdir(linkDir, { mode: 0o751, recursive: true });
				await fs.symlink(path.relative(linkDir, file), link);
			});
		// Symlink libraries
		const libraryFiles = await fs.readdir(libraries, { recursive: true, withFileTypes: true });
		const linkLibs = libraryFiles
			.filter(
				(entry) =>
					(entry.isFile() || entry.isSymbolicLink()) && entry.name.endsWith('.dylib')
			)
			.map(async (entry) => {
				const file = path.join(entry.path, entry.name);
				const link = path.resolve(lib, path.relative(libraries, file));
				const linkDir = path.dirname(link);
				await fs.mkdir(linkDir, { mode: 0o751, recursive: true });
				await fs.symlink(path.relative(linkDir, file), link);
				if (entry.isFile()) {
					// Sign the lib with the local machine certificate (Required for it to work on macOS 13+)
					await exec(`codesign -s "${env.APPLE_SIGNING_IDENTITY || '-'}" -f "${file}"`);
				}
			});

		await Promise.all([...linkHeaders, ...linkLibs]);
	} catch (error) {
		console.error(
			'Failed to configure required Frameworks.This is probably a bug, please open a issue with you system info at: ' +
				'https://github.com/spacedriveapp/spacedrive/issues/new/choose'
		);
		if (__debug) console.error(error);
		process.exit(1);
	}
}
