import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { env } from 'node:process';

// https://stackoverflow.com/q/3651860#answer-67151923
const REF_REGEX = /ref:\s+refs\/heads\/(?<branch>[^\s\x00-\x1F\:\?\[\\\^\~]+)/;

/**
 * @param {string} repoPath
 * @returns {Promise<string[]>}
 */
export async function getGitBranches(repoPath) {
	const branches = ['main', 'master'];

	if (env.GITHUB_HEAD_REF)
		branches.unshift(env.GITHUB_HEAD_REF);
	else if (env.GITHUB_REF)
		branches.unshift(env.GITHUB_REF.replace(/^refs\/heads\//, ''));

	let head;
	try {
		head = await fs.readFile(path.join(repoPath, '.git', 'HEAD'), { encoding: 'utf8' });
	} catch {
		return branches;
	}

	const match = REF_REGEX.exec(head);
	if (match?.groups?.branch) branches.unshift(match.groups.branch);

	return branches;
}
