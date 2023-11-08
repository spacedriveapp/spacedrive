import { exec as execCb } from 'node:child_process'
import { setTimeout } from 'node:timers/promises'
import { promisify } from 'node:util'

const exec = promisify(execCb)

/**
 * @param {string} file
 * @returns {Promise<void>}
 */
export async function waitLockUnlock(file) {
	let locked = false
	while (!locked) {
		try {
			await exec(`flock -ns "${file}" -c true`)
			await setTimeout(100)
		} catch {
			locked = true
		}
	}

	while (locked) {
		try {
			await exec(`flock -ns "${file}" -c true`)
		} catch {
			await setTimeout(100)
			continue
		}
		locked = false
	}
}
