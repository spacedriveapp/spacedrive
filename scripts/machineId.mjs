import { exec as execCb } from 'node:child_process';
import * as os from 'node:os';
import { env } from 'node:process';
import { promisify } from 'node:util';

const __debug = env.NODE_ENV === 'debug';

let libc = 'glibc';
if (os.type() === 'Linux') {
	try {
		const exec = promisify(execCb);
		if ((await exec('ldd /bin/ls')).stdout.includes('musl')) {
			libc = 'musl';
		}
	} catch (error) {
		if (__debug) {
			console.warn(`Failed to check libc type`);
			console.error(error);
		}
	}
}

const OS_TYPE = {
	darwin: 'Darwin',
	windows: 'Windows_NT',
	linux: 'Linux'
};

export function getMachineId() {
	let machineId;

	/**
	 * Possible TARGET_TRIPLE:
	 * x86_64-apple-darwin
	 * aarch64-apple-darwin
	 * x86_64-pc-windows-msvc
	 * aarch64-pc-windows-msvc
	 * x86_64-unknown-linux-gnu
	 * x86_64-unknown-linux-musl
	 * aarch64-unknown-linux-gnu
	 * aarch64-unknown-linux-musl
	 * armv7-unknown-linux-gnueabihf
	 */
	if (env.TARGET_TRIPLE) {
		const target = env.TARGET_TRIPLE.split('-');
		const osType = OS_TYPE[target[2]];

		if (!osType) throw new Error(`Unknown OS type: ${target[2]}`);
		if (!target[0]) throw new Error(`Unknown machine type: ${target[0]}`);

		machineId = [osType, target[0]];
		if (machineId[0] === 'Linux') machineId.push(target[3].includes('musl') ? 'musl' : 'glibc');
	} else {
		// Current machine identifiers
		machineId = [os.type(), os.machine()];
		if (machineId[0] === 'Linux') machineId.push(libc);
	}

	return machineId;
}
