import { exec as execCb } from 'node:child_process'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { env } from 'node:process'
import { promisify } from 'node:util'

const __debug = env.NODE_ENV === 'debug'

const exec = promisify(execCb)

/**
 * @param {string} repoPath
 * @returns {Promise<string?>}
 */
async function getRemoteBranchName(repoPath) {
	let branchName
	try {
		branchName = (await exec('git symbolic-ref --short HEAD', { cwd: repoPath })).stdout.trim()
		if (!branchName) throw new Error('Empty local branch name')
	} catch (error) {
		if (__debug) {
			console.warn(`Failed to read git local branch name`)
			console.error(error)
		}
		return null
	}

	let remoteBranchName
	try {
		remoteBranchName = (
			await exec(`git for-each-ref --format="%(upstream:short)" refs/heads/${branchName}`, {
				cwd: repoPath,
			})
		).stdout.trim()
		const [_, branch] = remoteBranchName.split('/')
		if (!branch) throw new Error('Empty remote branch name')
		remoteBranchName = branch
	} catch (error) {
		if (__debug) {
			console.warn(`Failed to read git remote branch name`)
			console.error(error)
		}
		return null
	}

	return remoteBranchName
}

// https://stackoverflow.com/q/3651860#answer-67151923
// eslint-disable-next-line no-control-regex
const REF_REGEX = /ref:\s+refs\/heads\/(?<branch>[^\s\x00-\x1F:?[\\^~]+)/
const GITHUB_REF_REGEX = /^refs\/heads\//

/**
 * @param {string} repoPath
 * @returns {Promise<string[]>}
 */
export async function getGitBranches(repoPath) {
	const branches = ['main', 'master']

	if (env.GITHUB_HEAD_REF) {
		branches.unshift(env.GITHUB_HEAD_REF)
	} else if (env.GITHUB_REF) {
		branches.unshift(env.GITHUB_REF.replace(GITHUB_REF_REGEX, ''))
	}

	const remoteBranchName = await getRemoteBranchName(repoPath)
	if (remoteBranchName) {
		branches.unshift(remoteBranchName)
	} else {
		let head
		try {
			head = await fs.readFile(path.join(repoPath, '.git', 'HEAD'), { encoding: 'utf8' })
		} catch (error) {
			if (__debug) {
				console.warn(`Failed to read git HEAD file`)
				console.error(error)
			}
			return branches
		}

		const match = REF_REGEX.exec(head)
		if (match?.groups?.branch) branches.unshift(match.groups.branch)
	}

	return branches
}
