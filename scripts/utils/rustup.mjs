import { exec as execCb } from 'node:child_process'
import { promisify } from 'node:util'

const exec = promisify(execCb)

/**
 * Get the list of rust targets
 * @returns {Promise<Set<string>>}
 */
export async function getRustTargetList() {
	return new Set((await exec('rustup target list --installed')).stdout.split('\n'))
}
