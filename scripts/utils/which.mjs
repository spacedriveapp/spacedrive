import { exec as execCb } from 'node:child_process'
import * as fs from 'node:fs/promises'
import * as os from 'node:os'
import * as path from 'node:path'
import { env } from 'node:process'
import { promisify } from 'node:util'

const exec = promisify(execCb)

/**
 * @param {string} progName
 * @returns {Promise<boolean>}
 */
async function where(progName) {
	// Reject paths
	if (/[\\]/.test(progName)) return false
	try {
		await exec(`where "${progName}"`)
	} catch {
		return false
	}

	return true
}

/**
 * @param {string} progName
 * @returns {Promise<boolean>}
 */
export async function which(progName) {
	return os.type() === 'Windows_NT'
		? where(progName)
		: Promise.any(
				Array.from(new Set(env.PATH?.split(':'))).map(dir =>
					fs.access(path.join(dir, progName), fs.constants.X_OK)
				)
			).then(
				() => true,
				() => false
			)
}
