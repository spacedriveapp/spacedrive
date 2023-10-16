import { type components } from '@octokit/openapi-types';
import { env } from '~/env';

type Release = components['schemas']['release'];

const FETCH_META = {
	headers: {
		Authorization: `Bearer ${env.GITHUB_PAT}`,
		Accept: 'application/vnd.github+json'
	}
} as RequestInit;

export const RELEASES_PATH = `/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases`;

interface FetchConfig<T> {
	path: string;
	_type?: T;
}

export const getRecentReleases = {
	path: RELEASES_PATH
} as FetchConfig<Release[]>;

export const getLatestRelease = {
	path: `${RELEASES_PATH}/latest`
} as FetchConfig<Release>;

export const getRelease = (tag: string) =>
	({
		path: `${RELEASES_PATH}/tags/${tag}`
	}) as FetchConfig<Release>;

export async function githubFetch<T>({ path }: FetchConfig<T>): Promise<T> {
	return fetch(`https://api.github.com${path}`, {
		...FETCH_META,
		next: {
			tags: [path]
		}
	}).then((r) => r.json());
}

export function getReleaseFrontmatter({ body }: Release): {
	frontmatter: object;
	body: string;
} {
	if (!body) return { frontmatter: {}, body: '' };

	let tagline: string | undefined;

	const lines = body.split('\n');

	if (lines[0].startsWith('<!-- frontmatter')) {
		const endIndex = lines.findIndex((l) => l.startsWith('-->'));
		if (!endIndex) return { frontmatter: {}, body: '' };

		const [_, ...frontmatter] = lines.splice(0, endIndex + 1);

		for (const line of frontmatter) {
			const [name, value] = line.split(': ');

			if (name === 'tagline') tagline = value.trim();
		}
	}

	return { frontmatter: { tagline }, body: lines.join('\n') };
}
