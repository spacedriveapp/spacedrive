import { env } from '~/env';

export const API = `https://api.github.com`;
export const REPO_API = `${API}/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}`;
export const HEADERS = {
	'Authorization': `Bearer ${env.GITHUB_PAT}`,
	'Accept': 'application/vnd.github+json',
	'Content-Type': 'application/json'
};
