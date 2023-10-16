import { type components } from '@octokit/openapi-types';
import { NextResponse } from 'next/server';
import { z } from 'zod';
import { env } from '~/env';

const version = z.union([z.literal('stable'), z.literal('alpha')]);
const tauriTarget = z.union([z.literal('linux'), z.literal('windows'), z.literal('darwin')]);
const tauriArch = z.union([z.literal('x86_64'), z.literal('aarch64')]);

const extensions = {
	linux: 'AppImage',
	windows: 'msi',
	darwin: 'dmg'
} as const satisfies Record<z.infer<typeof tauriTarget>, string>;

const paramsSchema = z.object({
	target: tauriTarget,
	arch: tauriArch,
	version: version.or(z.string())
});

export const runtime = 'edge';

export async function GET(
	_: Request,
	{
		params: rawParams
	}: {
		params: {
			version: string;
			target: string;
			arch: string;
		};
	}
) {
	const params = await paramsSchema.parseAsync(rawParams);

	const release = await (async () => {
		switch (params.version) {
			case 'alpha': {
				const data = await getRecentReleases();

				return data.find((d: any) => d.tag_name.includes('alpha'));
			}
			case 'stable':
				return await getLatestRelease();
			default:
				return getRelease(params.version);
		}
	})();

	if (!release) return NextResponse.json({ error: 'Release not found' }, { status: 404 });

	params.version = release.tag_name;

	const name = `Spacedrive-${params.target}-${params.arch}.${extensions[params.target]}` as const;

	const asset = release.assets?.find((asset: any) => asset.name === name);

	if (!asset) return NextResponse.json({ error: 'Asset not found' }, { status: 404 });

	return NextResponse.redirect(asset.browser_download_url);
}

type Release = components['schemas']['release'];

const FETCH_META = {
	headers: {
		Authorization: `Bearer ${env.GITHUB_PAT}`,
		Accept: 'application/vnd.github+json'
	}
} as RequestInit;

const RELEASES_PATH = `/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases`;

export function getRecentReleases(): Promise<Release[]> {
	return githubFetch(RELEASES_PATH);
}

export function getLatestRelease(): Promise<Release> {
	return githubFetch(`${RELEASES_PATH}/latest`);
}

export function getRelease(tag: string): Promise<Release> {
	return githubFetch(`${RELEASES_PATH}/tags/${tag}`);
}

async function githubFetch(path: string) {
	return fetch(`https://api.github.com${path}`, {
		...FETCH_META,
		next: {
			tags: [path]
		}
	}).then((r) => r.json());
}
