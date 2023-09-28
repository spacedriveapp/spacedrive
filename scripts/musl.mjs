import { exec as execCb } from 'node:child_process';
import { promisify } from 'node:util';

const exec = promisify(execCb);

/** @returns {Promise<boolean>} */
export async function isMusl() {
	try {
		return (await exec('ldd /bin/ls')).stdout.includes('musl');
	} catch {
		return false;
	}
}
