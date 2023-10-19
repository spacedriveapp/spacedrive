import { NextResponse } from 'next/server';
import { env } from '~/env';

export const runtime = 'edge';

export async function GET() {
	const latestRelease = await getReleaseVersion();

	if (!latestRelease) return NextResponse.json({ error: 'Release not found' }, { status: 404 });

	return NextResponse.json(
		{
			name: latestRelease.name,
			version: latestRelease.tag_name,
			pub_date: latestRelease.published_at,
			url: latestRelease.html_url,
			notes: latestRelease.body
		},
		{
			headers: {
				'Cache-Control': 'public, max-age=60'
			}
		}
	);
}

async function getReleaseVersion(): Promise<any> {
	return await githubFetch(`/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases/latest`);
}

const FETCH_META = {
	headers: {
		Authorization: `Bearer ${env.GITHUB_PAT}`,
		Accept: 'application/vnd.github+json'
	},
	next: {
		revalidate: 60
	}
} as RequestInit;

async function githubFetch(path: string) {
	return fetch(`https://api.github.com${path}`, FETCH_META).then((r) => r.json());
}